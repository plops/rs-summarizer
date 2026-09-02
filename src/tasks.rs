use std::time::Duration;

use chrono::Utc;
use sqlx::SqlitePool;
use tokio::time::sleep;
use tracing;

use crate::db;
use crate::errors::{ProcessError, SummaryError, TranscriptError};
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

/// Formats a ProcessError into a user-friendly raw error message.
fn format_process_error(e: &ProcessError) -> String {
    match e {
        ProcessError::Summary(SummaryError::ApiError(msg)) => msg.clone(),
        ProcessError::Transcript(TranscriptError::YtDlpFailed(msg)) => msg.clone(),
        other => other.to_string(),
    }
}

/// Helper to construct the daily rate limit fallback chain for a starting model.
fn get_fallback_chain(model_name: &str) -> Vec<&str> {
    match model_name {
        "hetzner-qwen-3.6-35b" => vec![
            "hetzner-qwen-3.6-35b",
            "hetzner-qwen-3.8-27b",
            "gemini-3.8-flash",
	    "gemini-3.7-flash",
            "gemini-3.6-flash",
            "gemini-3.5-flash",
            "gemini-3.5-flash-lite",
        ],
        "hetzner-qwen-3.8-27b" => vec![
            "hetzner-qwen-3.8-27b",
            "hetzner-qwen-3.6-35b",
	    "gemini-3.8-flash",
            "gemini-3.7-flash",
            "gemini-3.6-flash",
            "gemini-3.5-flash",
            "gemini-3.5-flash-lite",
        ],
        "gemini-3.7-flash" => vec![
            "gemini-3.7-flash",
	    "gemini-3.8-flash",
            "gemini-3.6-flash",
            "gemini-3.5-flash",
            "gemini-3-flash-preview",
            "gemini-2.5-flash",
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite",
            "gemini-2.5-flash-lite",
            "hetzner-qwen-3.8-27b",
            "hetzner-qwen-3.6-35b",
        ],
        "gemini-3.6-flash" => vec![
            "gemini-3.6-flash",
	    "gemini-3.8-flash",
            "gemini-3.7-flash",
            "gemini-3.5-flash",
            "gemini-3-flash-preview",
            "gemini-2.5-flash",
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite",
            "gemini-2.5-flash-lite",
            "hetzner-qwen-3.8-27b",
            "hetzner-qwen-3.6-35b",
        ],
        "gemini-3.5-flash-lite" => vec![
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite",
            "gemini-2.5-flash-lite",
	    "gemini-3.8-flash",
            "gemini-3.7-flash",
            "gemini-3.6-flash",
            "gemini-3.5-flash",
            "hetzner-qwen-3.8-27b",
            "hetzner-qwen-3.6-35b",
        ],
        "gemini-3.5-flash" => vec![
            "gemini-3.5-flash",
            "gemini-3.7-flash",
            "gemini-3.6-flash",
	    "gemini-3.8-flash",
            "gemini-3-flash-preview",
            "gemini-2.5-flash",
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite",
            "hetzner-qwen-3.8-27b",
            "hetzner-qwen-3.6-35b",
        ],
        "gemini-3-flash-preview" => vec![
            "gemini-3-flash-preview",
            "gemini-3.7-flash",
	    "gemini-3.8-flash",
            "gemini-3.6-flash",
            "gemini-2.5-flash",
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite",
            "hetzner-qwen-3.8-27b",
            "hetzner-qwen-3.6-35b",
        ],
        "gemini-2.5-flash" => vec![
            "gemini-2.5-flash",
            "gemini-3.7-flash",
	    "gemini-3.8-flash",
            "gemini-3.6-flash",
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite",
            "hetzner-qwen-3.8-27b",
            "hetzner-qwen-3.6-35b",
        ],
        "gemini-3.1-flash-lite" => vec![
            "gemini-3.1-flash-lite",
            "gemini-2.5-flash-lite",
            "gemini-3.5-flash-lite",
	    "gemini-3.8-flash",
            "gemini-3.7-flash",
            "gemini-3.6-flash",
            "gemini-3.5-flash",
            "gemini-3-flash-preview",
            "gemini-2.5-flash",
            "hetzner-qwen-3.8-27b",
            "hetzner-qwen-3.6-35b",
        ],
        "gemini-2.5-flash-lite" => vec![
            "gemini-2.5-flash-lite",
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite",
	    "gemini-3.8-flash",
            "gemini-3.7-flash",
            "gemini-3.6-flash",
            "hetzner-qwen-3.8-27b",
            "hetzner-qwen-3.6-35b",
        ],
        other => vec![other],
    }
}

/// Resolves the model name to use, falling back to alternatives if a model's daily rate limit has been hit.
#[allow(dead_code)]
async fn resolve_model_with_fallback(
    initial_model_name: &str,
    app: &AppState,
) -> Result<String, ProcessError> {
    let chain = get_fallback_chain(initial_model_name);
    for model_name in chain {
        if let Some(model) = app.model_options.iter().find(|m| m.name == model_name) {
            let allowed = crate::services::rate_limiter::RateLimiter::check_rate_limit(
                model,
                &app.model_counts,
                &app.last_reset_day,
            )
            .await;
            if allowed {
                return Ok(model_name.to_string());
            }
        }
    }
    // Default to the initial model if all are exhausted
    Ok(initial_model_name.to_string())
}

/// Global rate limiter implementation to ensure requests to a specific model do not exceed its RPM limit.
async fn enforce_rpm_limit(model_name: &str, rpm_limit: u32, app: &AppState) {
    if rpm_limit == 0 {
        return;
    }
    let delay_per_request = Duration::from_secs_f64(60.0 / (rpm_limit as f64));
    let lock = app.get_model_lock(model_name).await;
    let mut last_request_time = lock.lock().await;
    let now = std::time::Instant::now();
    if let Some(last_time) = *last_request_time {
        if let Some(elapsed) = now.checked_duration_since(last_time) {
            if elapsed < delay_per_request {
                let sleep_dur = delay_per_request - elapsed;
                tracing::info!(
                    model = model_name,
                    sleep_ms = sleep_dur.as_millis(),
                    "RPM limit delay active, sleeping"
                );
                tokio::time::sleep(sleep_dur).await;
            }
        }
    }
    *last_request_time = Some(std::time::Instant::now());
}

use crate::services::hacker_news::HackerNewsService;

/// Represents the aggregated output of the model pipeline run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SummaryOutput {
    pub summary_text: String,
    pub thinking_text: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub thinking_tokens: u64,
    pub cost: f64,
}

/// Extracts YouTube transcript for a given URL using TranscriptService.
pub async fn fetch_youtube_content(
    url: &str,
    identifier: i64,
    app: &AppState,
) -> Result<String, ProcessError> {
    let _permit = app.download_limiter.acquire_yt_dlp_permit().await;
    let transcript_svc = TranscriptService::new("/dev/shm");
    tracing::info!(
        identifier = identifier,
        url = %url,
        "Downloading transcript for video"
    );

    let text = transcript_svc.download_transcript(url, identifier).await?;

    let size_bytes = text.len();
    let word_count = text.split_whitespace().count();

    tracing::info!(
        identifier = identifier,
        url = %url,
        size_bytes = size_bytes,
        word_count = word_count,
        "Downloaded transcript successfully"
    );

    Ok(text)
}

/// Fetches Hacker News submission, comments, and linked article content.
pub async fn fetch_hn_content(
    hn_id: u64,
    user_pasted: Option<&str>,
    hn_svc: &HackerNewsService,
    app: &AppState,
) -> Result<String, ProcessError> {
    let _permit = app.download_limiter.acquire_hn_permit().await;
    tracing::info!(story_id = hn_id, "Fetching Hacker News submission");

    let hn_res = hn_svc
        .fetch_hn_submission(hn_id, user_pasted)
        .await
        .map_err(|e| ProcessError::Summary(SummaryError::ApiError(e)))?;

    let text = hn_res.combined_text;
    let size_bytes = text.len();
    let word_count = text.split_whitespace().count();

    tracing::info!(
        story_id = hn_id,
        size_bytes = size_bytes,
        word_count = word_count,
        "Fetched Hacker News submission successfully"
    );

    Ok(text)
}

/// Validates transcript text length and returns it if within bounds (30 to 280,000 words).
pub fn process_pasted_transcript(raw_text: &str) -> Result<String, ProcessError> {
    let word_count = raw_text.split_whitespace().count();
    if word_count < 30 {
        return Err(ProcessError::TranscriptTooShort);
    }
    if word_count > 280_000 {
        return Err(ProcessError::TranscriptTooLong(word_count));
    }
    Ok(raw_text.to_string())
}

/// Encapsulates model resolution, fallback chain execution, rate limiting, and streaming summary generation.
#[allow(clippy::too_many_arguments)]
/// Helper to detect if a SummaryError represents a rate limit / 429 / quota error.
fn is_summary_rate_limited(err: &SummaryError) -> bool {
    match err {
        SummaryError::RateLimited => true,
        SummaryError::ApiError(msg) => {
            msg.contains("ResourceExhausted")
                || msg.contains("429")
                || msg.contains("RESOURCE_EXHAUSTED")
                || msg.contains("quota")
                || msg.contains("Quota")
        }
        _ => false,
    }
}

/// Encapsulates model resolution, fallback chain execution, rate limiting, and streaming summary generation.
#[allow(clippy::too_many_arguments)]
pub async fn run_model_pipeline(
    db_pool: &SqlitePool,
    identifier: i64,
    app: &AppState,
    input_text: &str,
    initial_model_name: &str,
    is_hn: bool,
    google_search_grounding: bool,
    url_context: bool,
    include_glossary: bool,
    output_language: &str,
) -> Result<SummaryOutput, ProcessError> {
    let summary_svc = SummaryService::new(app.gemini_api_key.clone());

    let mut model_name = initial_model_name.to_string();
    if model_name == "auto" {
        if is_hn {
            let word_count = input_text.split_whitespace().count();
            model_name = if word_count < 15000 {
                "gemini-3.5-flash-lite".to_string()
            } else {
                "gemini-3.6-flash".to_string()
            };
        } else {
            let duration_secs = get_transcript_duration_secs(input_text);
            model_name = if duration_secs < 1800 {
                "gemini-3.5-flash-lite".to_string()
            } else {
                "gemini-3.6-flash".to_string()
            };
        }
    }

    let fallback_chain = get_fallback_chain(&model_name);
    let mut last_error = None;

    for candidate_name in fallback_chain {
        let model = match parse_model_option(candidate_name, &app.model_options) {
            Ok(m) => m,
            Err(_) => continue,
        };

        // Check daily rate limit for candidate model
        let allowed = crate::services::rate_limiter::RateLimiter::check_rate_limit(
            &model,
            &app.model_counts,
            &app.last_reset_day,
        )
        .await;

        if !allowed {
            tracing::warn!(
                model = %model.name,
                "Candidate model daily rate limit exceeded, skipping to next fallback"
            );
            continue;
        }

        // Increment candidate model's counter
        crate::services::rate_limiter::RateLimiter::increment_counter(
            &model.name,
            &app.model_counts,
        )
        .await;

        // Update database model name to the candidate model being attempted
        if let Err(e) = db::update_model(db_pool, identifier, &model.name).await {
            tracing::warn!(identifier = identifier, error = %e, "Failed to update DB model name");
        }

        let mut attempts = 0;
        let mut model_succeeded = false;
        let mut model_output = SummaryOutput::default();

        loop {
            attempts += 1;

            // Enforce RPM limit for candidate model
            enforce_rpm_limit(&model.name, model.rpm_limit, app).await;

            match summary_svc
                .generate_summary(
                    db_pool,
                    identifier,
                    input_text,
                    &model,
                    google_search_grounding,
                    url_context,
                    include_glossary,
                    output_language,
                )
                .await
            {
                Ok(res) => {
                    model_output = SummaryOutput {
                        summary_text: res.summary_text,
                        thinking_text: res.thinking_text,
                        input_tokens: res.input_tokens,
                        output_tokens: res.output_tokens,
                        thinking_tokens: res.thinking_tokens,
                        cost: res.cost,
                    };
                    model_succeeded = true;
                    break;
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if (err_str.contains("experiencing high demand")
                        || err_str.contains("high demand"))
                        && attempts < 3
                    {
                        let sleep_dur = if attempts == 1 {
                            if std::env::var("INTEGRATION_TEST").is_ok()
                                || std::env::var("TEST_MODE").is_ok()
                            {
                                Duration::from_millis(10)
                            } else {
                                Duration::from_secs(600)
                            }
                        } else {
                            if std::env::var("INTEGRATION_TEST").is_ok()
                                || std::env::var("TEST_MODE").is_ok()
                            {
                                Duration::from_millis(20)
                            } else {
                                Duration::from_secs(14400)
                            }
                        };
                        tracing::warn!(
                            model = %model.name,
                            attempt = attempts,
                            sleep_secs = sleep_dur.as_secs(),
                            "Gemini high demand error, retrying later"
                        );
                        tokio::time::sleep(sleep_dur).await;
                        continue;
                    }

                    if is_summary_rate_limited(&e) {
                        tracing::warn!(
                            model = %model.name,
                            error = %err_str,
                            "Gemini API rate limited during generation, attempting next model in fallback chain"
                        );
                        last_error = Some(ProcessError::Summary(e));
                        break; // Try next model in fallback_chain
                    }

                    return Err(ProcessError::Summary(e));
                }
            }
        }

        if model_succeeded {
            return Ok(model_output);
        }
    }

    Err(last_error.unwrap_or(ProcessError::Summary(SummaryError::RateLimited)))
}

/// Saves final summary state to database, converts markdown for YouTube format, and generates vector embeddings.
pub async fn finalize_and_embed(
    db_pool: &SqlitePool,
    app: &AppState,
    identifier: i64,
    summary: &SummaryOutput,
) -> Result<(), ProcessError> {
    let timestamp_end = Utc::now().to_rfc3339();
    db::mark_summary_done(
        db_pool,
        identifier,
        summary.input_tokens as i64,
        summary.output_tokens as i64,
        summary.thinking_tokens as i64,
        &summary.thinking_text,
        summary.cost,
        &timestamp_end,
    )
    .await?;

    let youtube_text = convert_markdown_to_youtube_format(&summary.summary_text);
    db::mark_timestamps_done(db_pool, identifier, &youtube_text).await?;

    let embedding_svc =
        EmbeddingService::new(app.gemini_api_key.clone(), "gemini-embedding-001", 3072);

    match embedding_svc.embed_text(&summary.summary_text).await {
        Ok(embedding) => {
            let bytes = embedding_to_bytes(&embedding);
            if let Err(e) =
                db::store_embedding(db_pool, identifier, &bytes, "gemini-embedding-001").await
            {
                tracing::warn!(identifier = identifier, error = %e, "Failed to store embedding");
            }
        }
        Err(e) => {
            tracing::warn!(identifier = identifier, error = %e, "Failed to compute embedding");
        }
    }

    Ok(())
}

/// Inner implementation that returns Result for clean error handling.
async fn process_summary_inner(
    db_pool: &SqlitePool,
    identifier: i64,
    app: &AppState,
) -> Result<(), ProcessError> {
    // Step 1: Ensure row exists (retry with backoff)
    let summary =
        wait_until_row_exists(db_pool, identifier, Duration::from_millis(100), 400).await?;

    let urls = split_urls(&summary.original_source_link);
    let first_url = urls.first().cloned().unwrap_or_default();
    let is_hn_url = crate::utils::url_validator::validate_hn_url(&first_url).is_some();

    let mut combined_output = SummaryOutput::default();
    let hn_svc = HackerNewsService::new();

    if summary.transcript.is_empty() {
        // Sequential download and summarization loop
        let mut downloaded_transcripts = Vec::new();

        for url in &urls {
            let is_hn = crate::utils::url_validator::validate_hn_url(url).is_some();
            let process_result = async {
                let raw_transcript = if is_hn {
                    let hn_id = crate::utils::url_validator::validate_hn_url(url).unwrap();
                    fetch_hn_content(hn_id, None, &hn_svc, app).await?
                } else {
                    fetch_youtube_content(url, identifier, app).await?
                };

                let transcript = process_pasted_transcript(&raw_transcript)?;

                if urls.len() > 1 {
                    let header = format!("\n\n### Summary for {}\n", url);
                    db::update_summary_chunk(db_pool, identifier, &header).await?;
                }

                let pipeline_res = run_model_pipeline(
                    db_pool,
                    identifier,
                    app,
                    &transcript,
                    &summary.model,
                    is_hn,
                    summary.google_search_grounding,
                    summary.url_context,
                    summary.include_glossary,
                    &summary.output_language,
                )
                .await?;

                Ok((transcript, pipeline_res))
            }
            .await;

            match process_result {
                Ok((transcript, res)) => {
                    downloaded_transcripts.push((url.clone(), transcript));

                    combined_output.input_tokens += res.input_tokens;
                    combined_output.output_tokens += res.output_tokens;
                    combined_output.thinking_tokens += res.thinking_tokens;
                    combined_output.cost += res.cost;

                    if !combined_output.thinking_text.is_empty() {
                        combined_output.thinking_text.push_str("\n\n");
                    }
                    combined_output
                        .thinking_text
                        .push_str(&format!("--- Thinking for {} ---\n", url));
                    combined_output.thinking_text.push_str(&res.thinking_text);
                    combined_output.summary_text.push_str(&res.summary_text);
                }
                Err(err) => {
                    tracing::error!(url = %url, error = %err, "Failed to process item");
                    let error_card = format!(
                        "\n\n### Error for {}\nError: {}\n",
                        url,
                        format_process_error(&err)
                    );
                    let _ = db::update_summary_chunk(db_pool, identifier, &error_card).await;
                    combined_output.summary_text.push_str(&error_card);
                }
            }
        }

        // Save the combined transcript to the DB
        let mut combined_transcript = String::new();
        for (i, (url, t)) in downloaded_transcripts.iter().enumerate() {
            if urls.len() > 1 {
                if i > 0 {
                    combined_transcript.push_str("\n\n");
                }
                combined_transcript.push_str(&format!("--- Transcript for {} ---\n", url));
            }
            combined_transcript.push_str(t);
        }
        db::update_transcript(db_pool, identifier, &combined_transcript).await?;
    } else if is_hn_url {
        let hn_id = crate::utils::url_validator::validate_hn_url(&first_url).unwrap();
        let user_pasted = if summary.transcript.trim().is_empty() {
            None
        } else {
            Some(summary.transcript.as_str())
        };

        let transcript = fetch_hn_content(hn_id, user_pasted, &hn_svc, app).await?;
        db::update_transcript(db_pool, identifier, &transcript).await?;

        combined_output = run_model_pipeline(
            db_pool,
            identifier,
            app,
            &transcript,
            &summary.model,
            true,
            summary.google_search_grounding,
            summary.url_context,
            summary.include_glossary,
            &summary.output_language,
        )
        .await?;
    } else {
        // Paste mode: transcript is already in DB
        let url = urls.first().cloned().unwrap_or_default();
        let process_video_result = async {
            let transcript = process_pasted_transcript(&summary.transcript)?;

            let res = run_model_pipeline(
                db_pool,
                identifier,
                app,
                &transcript,
                &summary.model,
                false,
                summary.google_search_grounding,
                summary.url_context,
                summary.include_glossary,
                &summary.output_language,
            )
            .await?;

            Ok(res)
        }
        .await;

        match process_video_result {
            Ok(res) => {
                combined_output = res;
            }
            Err(err) => {
                tracing::error!(url = %url, error = %err, "Failed to process video");
                let error_card = format!("Error: {}", format_process_error(&err));
                let _ = db::update_summary_full(db_pool, identifier, &error_card).await;
                combined_output.summary_text.push_str(&error_card);
            }
        }
    }

    // Step 5b, 6 & 7: Finalize database records, formatting, and generate embeddings
    finalize_and_embed(db_pool, app, identifier, &combined_output).await?;

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
fn parse_model_option(
    model_name: &str,
    model_options: &[ModelOption],
) -> Result<ModelOption, ProcessError> {
    model_options
        .iter()
        .find(|m| m.name == model_name)
        .cloned()
        .ok_or_else(|| {
            ProcessError::Summary(crate::errors::SummaryError::ApiError(format!(
                "Unknown model: {}",
                model_name
            )))
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
        assert_eq!(
            get_transcript_duration_secs(transcript),
            3600 + 15 * 60 + 30
        );
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
        assert!(formatted.contains("rate limited"));
    }

    #[test]
    fn test_format_process_error_quota() {
        let err = ProcessError::Summary(SummaryError::ApiError(
            "You exceeded your current quota, please check your plan and billing details."
                .to_string(),
        ));
        let formatted = format_process_error(&err);
        assert_eq!(
            formatted,
            "You exceeded your current quota, please check your plan and billing details."
        );
    }

    #[test]
    fn test_get_fallback_chain_new_models() {
        let chain_37_flash = get_fallback_chain("gemini-3.7-flash");
        assert_eq!(chain_37_flash[0], "gemini-3.7-flash");
        assert!(chain_37_flash.contains(&"gemini-3.6-flash"));
        assert!(chain_37_flash.contains(&"gemini-3.5-flash"));
        assert!(chain_37_flash.contains(&"gemini-3.5-flash-lite"));
        assert!(chain_37_flash.contains(&"hetzner-qwen-3.8-27b"));
        assert!(chain_37_flash.contains(&"hetzner-qwen-3.6-35b"));

        let chain_36_flash = get_fallback_chain("gemini-3.6-flash");
        assert_eq!(chain_36_flash[0], "gemini-3.6-flash");
        assert!(chain_36_flash.contains(&"gemini-3.7-flash"));
        assert!(chain_36_flash.contains(&"gemini-3.5-flash"));
        assert!(chain_36_flash.contains(&"gemini-3.5-flash-lite"));
        assert!(chain_36_flash.contains(&"hetzner-qwen-3.8-27b"));
        assert!(chain_36_flash.contains(&"hetzner-qwen-3.6-35b"));

        let chain_35_lite = get_fallback_chain("gemini-3.5-flash-lite");
        assert_eq!(chain_35_lite[0], "gemini-3.5-flash-lite");
        assert!(chain_35_lite.contains(&"gemini-3.7-flash"));
        assert!(chain_35_lite.contains(&"gemini-3.1-flash-lite"));
        assert!(chain_35_lite.contains(&"gemini-2.5-flash-lite"));
        assert!(chain_35_lite.contains(&"hetzner-qwen-3.8-27b"));
        assert!(chain_35_lite.contains(&"hetzner-qwen-3.6-35b"));

        let chain_hetzner36 = get_fallback_chain("hetzner-qwen-3.6-35b");
        assert_eq!(chain_hetzner36[0], "hetzner-qwen-3.6-35b");
        assert!(chain_hetzner36.contains(&"hetzner-qwen-3.8-27b"));
        assert!(chain_hetzner36.contains(&"gemini-3.7-flash"));
        assert!(chain_hetzner36.contains(&"gemini-3.6-flash"));
    }

    #[test]
    fn test_fallback_chain_hetzner_qwen38() {
        let chain = get_fallback_chain("hetzner-qwen-3.8-27b");
        assert_eq!(chain[0], "hetzner-qwen-3.8-27b");
        assert!(chain.contains(&"hetzner-qwen-3.6-35b"));
        assert!(chain.contains(&"gemini-3.7-flash"));
        assert!(chain.contains(&"gemini-3.6-flash"));
    }

    #[test]
    fn test_gemini_chains_include_all_hetzner_models() {
        let hetzner_models = vec!["hetzner-qwen-3.8-27b", "hetzner-qwen-3.6-35b"];
        for gemini_model in &["gemini-3.7-flash", "gemini-3.6-flash", "gemini-3.5-flash"] {
            let chain = get_fallback_chain(gemini_model);
            for hetzner in &hetzner_models {
                assert!(
                    chain.contains(hetzner),
                    "Fallback chain for {} should contain {}",
                    gemini_model,
                    hetzner
                );
            }
        }
    }

    #[test]
    fn test_process_pasted_transcript_bounds() {
        let text_short = "hello world ";
        assert!(matches!(
            process_pasted_transcript(text_short),
            Err(ProcessError::TranscriptTooShort)
        ));

        let words_valid = "word ".repeat(50);
        let res = process_pasted_transcript(&words_valid);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), words_valid);

        let words_too_long = "word ".repeat(280_001);
        assert!(matches!(
            process_pasted_transcript(&words_too_long),
            Err(ProcessError::TranscriptTooLong(280001))
        ));
    }

    #[test]
    fn test_is_summary_rate_limited_variations() {
        assert!(is_summary_rate_limited(&SummaryError::RateLimited));
        assert!(is_summary_rate_limited(&SummaryError::ApiError(
            "RESOURCE_EXHAUSTED".into()
        )));
        assert!(is_summary_rate_limited(&SummaryError::ApiError(
            "code 429; description: Quota exceeded".into()
        )));
        assert!(!is_summary_rate_limited(&SummaryError::TranscriptTooShort));
    }

    #[test]
    fn test_hn_model_auto_selection_thresholds() {
        let short_hn_text = "word ".repeat(500);
        let long_hn_text = "word ".repeat(15300);

        let short_words = short_hn_text.split_whitespace().count();
        let long_words = long_hn_text.split_whitespace().count();

        let model_short = if short_words < 15000 {
            "gemini-3.5-flash-lite"
        } else {
            "gemini-3.6-flash"
        };
        let model_long = if long_words < 15000 {
            "gemini-3.5-flash-lite"
        } else {
            "gemini-3.6-flash"
        };

        assert_eq!(model_short, "gemini-3.5-flash-lite");
        assert_eq!(model_long, "gemini-3.6-flash");
    }

    #[test]
    fn test_transcript_duration_auto_selection_thresholds() {
        let short_duration = 1799;
        let long_duration = 1800;

        let model_short = if short_duration < 1800 {
            "gemini-3.5-flash-lite"
        } else {
            "gemini-3.6-flash"
        };
        let model_long = if long_duration < 1800 {
            "gemini-3.5-flash-lite"
        } else {
            "gemini-3.6-flash"
        };

        assert_eq!(model_short, "gemini-3.5-flash-lite");
        assert_eq!(model_long, "gemini-3.6-flash");
    }
}
