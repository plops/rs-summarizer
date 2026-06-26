use std::time::Duration;

use chrono::Utc;
use sqlx::SqlitePool;
use tokio::time::sleep;
use tracing;

use crate::db;
use crate::errors::{ProcessError, TranscriptError, SummaryError, EmbeddingError};
use crate::models::Summary;
use crate::services::embedding::{embedding_to_bytes, EmbeddingService};
use crate::services::summary::SummaryService;
use crate::services::transcript::TranscriptService;
use crate::state::{AppState, ModelOption};
use crate::utils::markdown_converter::convert_markdown_to_youtube_format;
use crate::utils::url_validator::split_urls;

/// Core background task that orchestrates the full summarization pipeline.
/// Spawned by tokio after a new summary row is inserted.
pub async fn process_summary(db_pool: SqlitePool, identifier: i64, app: AppState) {
    if let Err(e) = process_summary_inner(&db_pool, identifier, &app).await {
        tracing::error!(identifier = identifier, error = %e, "Processing failed");
        let formatted = format_process_error(&e);
        mark_error(&db_pool, identifier, &formatted).await;
    }
}

/// Helper to estimate transcript duration in seconds.
/// Falls back to word count approximation if no timestamps are present.
fn get_transcript_duration_secs(transcript: &str) -> u32 {
    let mut max_secs = 0;
    for line in transcript.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if let Some(first) = parts.first() {
            let ts_parts: Vec<&str> = first.split(':').collect();
            if ts_parts.len() == 3 {
                let h: u32 = ts_parts[0].parse().unwrap_or(0);
                let m: u32 = ts_parts[1].parse().unwrap_or(0);
                let s: u32 = ts_parts[2].parse().unwrap_or(0);
                let secs = h * 3600 + m * 60 + s;
                if secs > max_secs {
                    max_secs = secs;
                }
            } else if ts_parts.len() == 2 {
                let m: u32 = ts_parts[0].parse().unwrap_or(0);
                let s: u32 = ts_parts[1].parse().unwrap_or(0);
                let secs = m * 60 + s;
                if secs > max_secs {
                    max_secs = secs;
                }
            }
        }
    }
    
    if max_secs > 0 {
        max_secs
    } else {
        // Fallback: estimate from word count assuming 150 words per minute (2.5 words per second)
        let words = transcript.split_whitespace().count();
        (words as u32 * 2) / 5
    }
}

/// Formats a ProcessError into a user-friendly markdown string.
fn format_process_error(e: &ProcessError) -> String {
    let (title, detail, action) = match e {
        ProcessError::TranscriptTooShort => (
            "Transkript zu kurz".to_string(),
            "Das heruntergeladene Transkript enthält weniger als 30 Wörter.".to_string(),
            "Bitte wähle ein Video mit mehr gesprochenem Inhalt.".to_string(),
        ),
        ProcessError::TranscriptTooLong(words) => (
            "Transkript zu lang".to_string(),
            format!("Das Transkript enthält {} Wörter. Das Limit liegt bei 280.000 Wörtern.", words),
            "Bitte wähle ein kürzeres Video.".to_string(),
        ),
        ProcessError::RowNotFound => (
            "Eintrag nicht gefunden".to_string(),
            "Der Eintrag in der Datenbank konnte nicht geladen werden.".to_string(),
            "Bitte versuche es noch einmal.".to_string(),
        ),
        ProcessError::Database(db_err) => (
            "Datenbankfehler".to_string(),
            db_err.to_string(),
            "Bitte versuche es später noch einmal.".to_string(),
        ),
        ProcessError::Transcript(t_err) => match t_err {
            TranscriptError::InvalidUrl(url) => (
                "Ungültige Video-URL oder ID".to_string(),
                format!("Der Wert '{}' ist weder eine gültige YouTube-URL noch eine 11-stellige Video-ID.", url),
                "Bitte überprüfe deine Eingabe.".to_string(),
            ),
            TranscriptError::NoSubtitles => (
                "Keine Untertitel vorhanden".to_string(),
                "Für dieses Video sind keine Untertitel oder automatischen Transkripte verfügbar.".to_string(),
                "Bitte stelle sicher, dass Untertitel auf YouTube für dieses Video aktiviert sind.".to_string(),
            ),
            TranscriptError::Timeout(secs) => (
                "Zeitüberschreitung beim Download".to_string(),
                format!("Das Herunterladen des Transkripts hat länger als {} Sekunden gedauert.", secs),
                "Bitte versuche es später noch einmal.".to_string(),
            ),
            TranscriptError::ParseError(msg) => (
                "Fehler beim Verarbeiten des Transkripts".to_string(),
                msg.clone(),
                "Bitte versuche es später noch einmal oder wähle ein anderes Video.".to_string(),
            ),
            TranscriptError::YtDlpFailed(msg) => {
                if msg.contains("Sign in to confirm") || msg.contains("bot") || msg.contains("requires authentication") {
                    (
                        "YouTube-Download blockiert (Authentifizierung erforderlich)".to_string(),
                        "YouTube verlangt eine Anmeldung/Bestätigung (Bot-Schutz). Der Download wurde blockiert.".to_string(),
                        "Bitte versuche es später noch einmal.".to_string(),
                    )
                } else if msg.contains("429") || msg.contains("Too Many Requests") {
                    (
                        "YouTube Rate-Limit überschritten (429)".to_string(),
                        "Es wurden zu viele Anfragen an YouTube gesendet (Rate Limit).".to_string(),
                        "Bitte warte einen Moment und versuche es später noch einmal.".to_string(),
                    )
                } else if msg.contains("not available") || msg.contains("deleted") || msg.contains("private") {
                    (
                        "Video nicht verfügbar".to_string(),
                        "Das angegebene YouTube-Video ist nicht verfügbar (gelöscht oder privat).".to_string(),
                        "Bitte überprüfe die URL/ID.".to_string(),
                    )
                } else {
                    (
                        "Fehler beim Transkript-Download".to_string(),
                        msg.clone(),
                        "Bitte überprüfe die URL/ID oder versuche es später noch einmal.".to_string(),
                    )
                }
            }
        },
        ProcessError::Summary(s_err) => match s_err {
            SummaryError::RateLimited => (
                "Gemini API Rate-Limit überschritten".to_string(),
                "Das Kontingent der Gemini API für Anfragen (ResourceExhausted) wurde überschritten.".to_string(),
                "Bitte warte einen Moment und versuche es später noch einmal.".to_string(),
            ),
            SummaryError::TranscriptTooShort => (
                "Transkript zu kurz".to_string(),
                "Das Transkript enthält weniger als 30 Wörter.".to_string(),
                "Bitte wähle ein Video mit mehr gesprochenem Inhalt.".to_string(),
            ),
            SummaryError::TranscriptTooLong(words, max_words) => (
                "Transkript zu lang".to_string(),
                format!("Das Transkript enthält {} Wörter. Das Gemini-Limit liegt bei {} Wörtern.", words, max_words),
                "Bitte wähle ein kürzeres Video.".to_string(),
            ),
            SummaryError::ApiError(msg) => {
                if msg.contains("Internal error encountered") {
                    (
                        "Interner Fehler bei der Gemini API".to_string(),
                        "Der Gemini API-Server hat einen internen Fehler gemeldet.".to_string(),
                        "Bitte versuche es später noch einmal.".to_string(),
                    )
                } else if msg.contains("high demand") || msg.contains("experiencing high demand") {
                    (
                        "Gemini API überlastet (High Demand)".to_string(),
                        "Die Gemini API ist derzeit wegen hoher Nachfrage überlastet.".to_string(),
                        "Bitte versuche es in wenigen Minuten noch einmal.".to_string(),
                    )
                } else if msg.contains("quota") || msg.contains("exceeded your current quota") {
                    (
                        "Abrechnungs- oder Quoten-Limit überschritten".to_string(),
                        "Das Quoten-Limit deines Gemini API-Kontos wurde überschritten.".to_string(),
                        "Bitte überprüfe dein API-Konto und deine Abrechnungsdetails.".to_string(),
                    )
                } else {
                    (
                        "Fehler bei der Gemini API".to_string(),
                        msg.clone(),
                        "Bitte überprüfe deinen API-Schlüssel oder versuche es später noch einmal.".to_string(),
                    )
                }
            }
        },
        ProcessError::Embedding(e_err) => match e_err {
            EmbeddingError::ApiError(msg) => {
                if msg.contains("quota") || msg.contains("quota exceeded") {
                    (
                        "Abrechnungs- oder Quoten-Limit überschritten (Embeddings)".to_string(),
                        "Das Quoten-Limit deines Gemini API-Kontos wurde überschritten.".to_string(),
                        "Bitte überprüfe dein API-Konto und deine Abrechnungsdetails.".to_string(),
                    )
                } else {
                    (
                        "Fehler beim Berechnen des Embeddings".to_string(),
                        msg.clone(),
                        "Bitte versuche es später noch einmal.".to_string(),
                    )
                }
            }
            _ => (
                "Fehler beim Berechnen des Embeddings".to_string(),
                e_err.to_string(),
                "Bitte versuche es später noch einmal.".to_string(),
            ),
        },
    };

    format!(
        "### ⚠️ {}\n\n**Grund:** {}\n\n**Empfehlung:** {}",
        title, detail, action
    )
}

/// Inner implementation that returns Result for clean error handling.
async fn process_summary_inner(
    db_pool: &SqlitePool,
    identifier: i64,
    app: &AppState,
) -> Result<(), ProcessError> {
    // Step 1: Ensure row exists (retry with backoff)
    let summary = wait_until_row_exists(db_pool, identifier, Duration::from_millis(100), 400).await?;

    // Create services on-the-fly from AppState
    let transcript_svc = TranscriptService::new("/dev/shm");
    let summary_svc = SummaryService::new(app.gemini_api_key.clone());
    let embedding_svc = EmbeddingService::new(
        app.gemini_api_key.clone(),
        "gemini-embedding-001",
        3072,
    );

    // Step 2: Download transcripts if not provided
    let mut transcripts = Vec::new();
    let urls = split_urls(&summary.original_source_link);

    if summary.transcript.is_empty() {
        for url in &urls {
            let t = transcript_svc
                .download_transcript(url, identifier)
                .await?;
            transcripts.push((url.clone(), t));
        }

        // Save the combined transcript to the DB
        let mut combined_transcript = String::new();
        for (i, (url, t)) in transcripts.iter().enumerate() {
            if transcripts.len() > 1 {
                if i > 0 {
                    combined_transcript.push_str("\n\n");
                }
                combined_transcript.push_str(&format!("--- Transcript for {} ---\n", url));
            }
            combined_transcript.push_str(t);
        }
        db::update_transcript(db_pool, identifier, &combined_transcript).await?;
    } else {
        let url = urls.first().cloned().unwrap_or_default();
        transcripts.push((url, summary.transcript.clone()));
    }

    let mut total_input_tokens = 0;
    let mut total_output_tokens = 0;
    let mut total_thinking_tokens = 0;
    let mut total_cost = 0.0;
    let mut combined_thinking = String::new();
    let mut combined_summary = String::new();

    // Step 3-5: Validate, select model, and generate summary for each transcript serially
    for (_i, (url, transcript)) in transcripts.iter().enumerate() {
        // Validate transcript length
        let word_count = transcript.split_whitespace().count();
        if word_count < 30 {
            return Err(ProcessError::TranscriptTooShort);
        }
        if word_count > 280_000 {
            return Err(ProcessError::TranscriptTooLong(word_count));
        }

        // Parse model option & apply heuristic if model is "auto"
        let mut model_name = summary.model.clone();
        let was_auto = model_name == "auto";
        if was_auto {
            let duration_secs = get_transcript_duration_secs(transcript);
            model_name = if duration_secs < 1800 {
                "gemini-3.1-flash-lite".to_string()
            } else {
                "gemini-3.5-flash".to_string()
            };
            db::update_model(db_pool, identifier, &model_name).await?;
        }

        let model = parse_model_option(&model_name, &app.model_options)?;

        if was_auto {
            // Check local rate limit for the actual chosen model
            let allowed = crate::services::rate_limiter::RateLimiter::check_rate_limit(
                &model,
                &app.model_counts,
                &app.last_reset_day,
            )
            .await;
            if !allowed {
                return Err(ProcessError::Summary(SummaryError::RateLimited));
            }
            // Increment the actual model's counter
            crate::services::rate_limiter::RateLimiter::increment_counter(
                &model.name,
                &app.model_counts,
            )
            .await;
        }

        // Add header for this video if there are multiple
        if transcripts.len() > 1 {
            let header = format!("\n\n### Summary for {}\n", url);
            db::update_summary_chunk(db_pool, identifier, &header).await?;
            combined_summary.push_str(&header);
        }

        // Generate summary (streaming, updates DB progressively)
        let result = summary_svc
            .generate_summary(
                db_pool,
                identifier,
                transcript,
                &model,
                summary.google_search_grounding,
                summary.url_context,
            )
            .await?;

        total_input_tokens += result.input_tokens;
        total_output_tokens += result.output_tokens;
        total_thinking_tokens += result.thinking_tokens;
        total_cost += result.cost;

        if !combined_thinking.is_empty() {
            combined_thinking.push_str("\n\n");
        }
        combined_thinking.push_str(&format!("--- Thinking for {} ---\n", url));
        combined_thinking.push_str(&result.thinking_text);
        combined_summary.push_str(&result.summary_text);
    }

    // Step 5b: Mark summary as done (stops HTMX polling on the frontend)
    let timestamp_end = Utc::now().to_rfc3339();
    db::mark_summary_done(
        db_pool,
        identifier,
        total_input_tokens as i64,
        total_output_tokens as i64,
        total_thinking_tokens as i64,
        &combined_thinking,
        total_cost,
        &timestamp_end,
    )
    .await?;

    // Step 6: Convert to YouTube format and mark timestamps_done
    let youtube_text = convert_markdown_to_youtube_format(&combined_summary);
    db::mark_timestamps_done(db_pool, identifier, &youtube_text).await?;

    // Step 7: Compute and store embedding (non-fatal)
    match embedding_svc.embed_text(&combined_summary).await {
        Ok(embedding) => {
            let bytes = embedding_to_bytes(&embedding);
            if let Err(e) = db::store_embedding(db_pool, identifier, &bytes, "gemini-embedding-001").await {
                tracing::warn!(identifier = identifier, error = %e, "Failed to store embedding");
            }
        }
        Err(e) => {
            tracing::warn!(identifier = identifier, error = %e, "Failed to compute embedding");
        }
    }

    Ok(())
}

/// Polls the database until the row with the given identifier exists.
/// Returns the Summary once found, or ProcessError::RowNotFound after max attempts.
async fn wait_until_row_exists(
    db_pool: &SqlitePool,
    identifier: i64,
    interval: Duration,
    max_attempts: u32,
) -> Result<Summary, ProcessError> {
    for _ in 0..max_attempts {
        if let Some(summary) = db::fetch_summary(db_pool, identifier).await? {
            return Ok(summary);
        }
        sleep(interval).await;
    }
    Err(ProcessError::RowNotFound)
}

/// Finds the matching ModelOption by name from the configured options.
fn parse_model_option(model_name: &str, model_options: &[ModelOption]) -> Result<ModelOption, ProcessError> {
    model_options
        .iter()
        .find(|m| m.name == model_name)
        .cloned()
        .ok_or_else(|| {
            ProcessError::Summary(crate::errors::SummaryError::ApiError(
                format!("Unknown model: {}", model_name),
            ))
        })
}

/// Stores an error message in the summary field and marks summary_done=true.
/// This ensures the frontend stops polling and displays the error.
async fn mark_error(db_pool: &SqlitePool, identifier: i64, error_msg: &str) {
    let _ = db::update_summary_full(db_pool, identifier, error_msg).await;
    let _ = db::mark_summary_done(db_pool, identifier, 0, 0, 0, "", 0.0, "").await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_transcript_duration_secs_three_digits() {
        let transcript = "00:00:00 start\n01:15:30 end\n";
        assert_eq!(get_transcript_duration_secs(transcript), 3600 + 15 * 60 + 30);
    }

    #[test]
    fn test_get_transcript_duration_secs_two_digits() {
        let transcript = "00:00 start\n15:45 end\n";
        assert_eq!(get_transcript_duration_secs(transcript), 15 * 60 + 45);
    }

    #[test]
    fn test_get_transcript_duration_secs_fallback() {
        // Fallback assumes 150 words per minute -> 2.5 words per second
        // For 300 words: 300 * 2 / 5 = 120 seconds (2 minutes)
        let words = "word ".repeat(300);
        assert_eq!(get_transcript_duration_secs(&words), 120);
    }

    #[test]
    fn test_format_process_error_rate_limit() {
        let err = ProcessError::Summary(SummaryError::RateLimited);
        let formatted = format_process_error(&err);
        assert!(formatted.contains("Rate-Limit"));
        assert!(formatted.contains("⚠️"));
    }

    #[test]
    fn test_format_process_error_quota() {
        let err = ProcessError::Summary(SummaryError::ApiError("You exceeded your current quota, please check your plan and billing details.".to_string()));
        let formatted = format_process_error(&err);
        assert!(formatted.contains("Quoten-Limit"));
        assert!(formatted.contains("⚠️"));
    }
}
