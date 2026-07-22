use std::time::Duration;

use chrono::Utc;
use sqlx::SqlitePool;
use tokio::time::sleep;
use tracing;

use crate::db;
use crate::errors::{ProcessError, TranscriptError, SummaryError};
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
        "gemini-3.6-flash" => vec![
            "gemini-3.6-flash",
            "gemini-3.5-flash",
            "gemini-3-flash-preview",
            "gemini-2.5-flash",
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite",
            "gemini-2.5-flash-lite",
        ],
        "gemini-3.5-flash-lite" => vec![
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite",
            "gemini-2.5-flash-lite",
            "gemini-3.5-flash",
            "gemini-3.6-flash",
        ],
        "gemini-3.5-flash" => vec![
            "gemini-3.5-flash",
            "gemini-3-flash-preview",
            "gemini-2.5-flash",
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite",
        ],
        "gemini-3-flash-preview" => vec![
            "gemini-3-flash-preview",
            "gemini-2.5-flash",
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite",
        ],
        "gemini-2.5-flash" => vec![
            "gemini-2.5-flash",
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite",
        ],
        "gemini-3.1-flash-lite" => vec![
            "gemini-3.1-flash-lite",
            "gemini-2.5-flash-lite",
            "gemini-3.5-flash-lite",
            "gemini-3.6-flash",
            "gemini-3.5-flash",
            "gemini-3-flash-preview",
            "gemini-2.5-flash",
        ],
        "gemini-2.5-flash-lite" => vec![
            "gemini-2.5-flash-lite",
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite",
        ],
        other => vec![other],
    }
}

/// Resolves the model name to use, falling back to alternatives if a model's daily rate limit has been hit.
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

    let urls = split_urls(&summary.original_source_link);
    let mut downloaded_transcripts = Vec::new();

    let mut total_input_tokens = 0;
    let mut total_output_tokens = 0;
    let mut total_thinking_tokens = 0;
    let mut total_cost = 0.0;
    let mut combined_thinking = String::new();
    let mut combined_summary = String::new();
    let first_url = urls.first().cloned().unwrap_or_default();
    let is_hn_url = crate::utils::url_validator::validate_hn_url(&first_url).is_some();

    if summary.transcript.is_empty() {
        // Sequential download and summarization loop
        for url in &urls {
            let is_hn = crate::utils::url_validator::validate_hn_url(url).is_some();
            let process_result = async {
                let transcript = if is_hn {
                    let hn_id = crate::utils::url_validator::validate_hn_url(url).unwrap();
                    let hn_svc = crate::services::hacker_news::HackerNewsService::new();
                    
                    tracing::info!(
                        identifier = identifier,
                        url = %url,
                        story_id = hn_id,
                        "Fetching Hacker News submission"
                    );

                    let hn_res = hn_svc
                        .fetch_hn_submission(hn_id, None)
                        .await
                        .map_err(|e| ProcessError::Summary(SummaryError::ApiError(e)))?;

                    let text = hn_res.combined_text;
                    let size_bytes = text.len();
                    let word_count = text.split_whitespace().count();
                    
                    tracing::info!(
                        identifier = identifier,
                        url = %url,
                        size_bytes = size_bytes,
                        word_count = word_count,
                        "Fetched Hacker News submission successfully"
                    );

                    text
                } else {
                    tracing::info!(
                        identifier = identifier,
                        url = %url,
                        "Downloading transcript for video"
                    );

                    let text = transcript_svc
                        .download_transcript(url, identifier)
                        .await?;

                    let size_bytes = text.len();
                    let word_count = text.split_whitespace().count();

                    tracing::info!(
                        identifier = identifier,
                        url = %url,
                        size_bytes = size_bytes,
                        word_count = word_count,
                        "Downloaded transcript successfully"
                    );

                    text
                };

                // Validate transcript length
                let word_count = transcript.split_whitespace().count();
                if word_count < 30 {
                    return Err(ProcessError::TranscriptTooShort);
                }
                if word_count > 280_000 {
                    return Err(ProcessError::TranscriptTooLong(word_count));
                }

                let mut model_name = summary.model.clone();
                if model_name == "auto" {
                    if is_hn {
                        model_name = "gemini-3.6-flash".to_string();
                    } else {
                        let duration_secs = get_transcript_duration_secs(&transcript);
                        model_name = if duration_secs < 1800 {
                            "gemini-3.5-flash-lite".to_string()
                        } else {
                            "gemini-3.6-flash".to_string()
                        };
                    }
                }

                // Resolve model name using the daily rate limit fallback chain
                let resolved_model_name = resolve_model_with_fallback(&model_name, app).await?;
                let model = parse_model_option(&resolved_model_name, &app.model_options)?;

                // Check rate limit for the actual chosen model
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

                // Update database model name to the resolved model
                db::update_model(db_pool, identifier, &model.name).await?;

                // Add header for this item if there are multiple
                if urls.len() > 1 {
                    let header = format!("\n\n### Summary for {}\n", url);
                    db::update_summary_chunk(db_pool, identifier, &header).await?;
                }

                // Generate summary (streaming, updates DB progressively)
                let mut attempts = 0;
                let result = loop {
                    attempts += 1;

                    // Enforce RPM limit
                    enforce_rpm_limit(&model.name, model.rpm_limit, app).await;

                    match summary_svc
                        .generate_summary(
                            db_pool,
                            identifier,
                            &transcript,
                            &model,
                            summary.google_search_grounding,
                            summary.url_context,
                        )
                        .await
                    {
                        Ok(res) => break res,
                        Err(e) => {
                            let err_str = e.to_string();
                            if err_str.contains("experiencing high demand") || err_str.contains("high demand") {
                                if attempts < 3 {
                                    let sleep_dur = if attempts == 1 {
                                        if std::env::var("INTEGRATION_TEST").is_ok() || std::env::var("TEST_MODE").is_ok() {
                                            Duration::from_millis(10)
                                        } else {
                                            Duration::from_secs(600)
                                        }
                                    } else {
                                        if std::env::var("INTEGRATION_TEST").is_ok() || std::env::var("TEST_MODE").is_ok() {
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
                            }
                            return Err(ProcessError::Summary(e));
                        }
                    }
                };

                Ok((transcript, result))
            }.await;

            match process_result {
                Ok((transcript, result)) => {
                    downloaded_transcripts.push((url.clone(), transcript));

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
                Err(err) => {
                    tracing::error!(url = %url, error = %err, "Failed to process item");
                    let error_card = format!(
                        "\n\n### Error for {}\nError: {}\n",
                        url,
                        format_process_error(&err)
                    );
                    let _ = db::update_summary_chunk(db_pool, identifier, &error_card).await;
                    combined_summary.push_str(&error_card);
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
        let hn_svc = crate::services::hacker_news::HackerNewsService::new();
        let user_pasted = if summary.transcript.trim().is_empty() {
            None
        } else {
            Some(summary.transcript.as_str())
        };

        let hn_res = hn_svc
            .fetch_hn_submission(hn_id, user_pasted)
            .await
            .map_err(|e| ProcessError::Summary(SummaryError::ApiError(e)))?;

        let transcript = hn_res.combined_text;
        db::update_transcript(db_pool, identifier, &transcript).await?;

        let mut model_name = summary.model.clone();
        if model_name == "auto" {
            model_name = "gemini-3.6-flash".to_string();
        }

        let resolved_model_name = resolve_model_with_fallback(&model_name, app).await?;
        let model = parse_model_option(&resolved_model_name, &app.model_options)?;

        let allowed = crate::services::rate_limiter::RateLimiter::check_rate_limit(
            &model,
            &app.model_counts,
            &app.last_reset_day,
        )
        .await;
        if !allowed {
            return Err(ProcessError::Summary(SummaryError::RateLimited));
        }

        crate::services::rate_limiter::RateLimiter::increment_counter(
            &model.name,
            &app.model_counts,
        )
        .await;

        db::update_model(db_pool, identifier, &model.name).await?;

        enforce_rpm_limit(&model.name, model.rpm_limit, app).await;

        let result = summary_svc
            .generate_summary(
                db_pool,
                identifier,
                &transcript,
                &model,
                summary.google_search_grounding,
                summary.url_context,
            )
            .await
            .map_err(ProcessError::Summary)?;

        total_input_tokens = result.input_tokens;
        total_output_tokens = result.output_tokens;
        total_thinking_tokens = result.thinking_tokens;
        total_cost = result.cost;
        combined_thinking = result.thinking_text;
        combined_summary = result.summary_text;
    } else {
        // Paste mode: transcript is already in DB
        let url = urls.first().cloned().unwrap_or_default();
        let process_video_result = async {
            let transcript = &summary.transcript;

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
            if model_name == "auto" {
                let duration_secs = get_transcript_duration_secs(transcript);
                model_name = if duration_secs < 1800 {
                    "gemini-3.5-flash-lite".to_string()
                } else {
                    "gemini-3.6-flash".to_string()
                };
            }

            // Resolve model name using the daily rate limit fallback chain
            let resolved_model_name = resolve_model_with_fallback(&model_name, app).await?;
            let model = parse_model_option(&resolved_model_name, &app.model_options)?;

            // Check rate limit for the actual chosen model
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

            // Update database model name to the resolved model
            db::update_model(db_pool, identifier, &model.name).await?;

            // Generate summary (streaming, updates DB progressively)
            let mut attempts = 0;
            let result = loop {
                attempts += 1;

                // Enforce RPM limit
                enforce_rpm_limit(&model.name, model.rpm_limit, app).await;

                match summary_svc
                    .generate_summary(
                        db_pool,
                        identifier,
                        transcript,
                        &model,
                        summary.google_search_grounding,
                        summary.url_context,
                    )
                    .await
                {
                    Ok(res) => break res,
                    Err(e) => {
                        let err_str = e.to_string();
                        if err_str.contains("experiencing high demand") || err_str.contains("high demand") {
                            if attempts < 3 {
                                let sleep_dur = if attempts == 1 {
                                    if std::env::var("INTEGRATION_TEST").is_ok() || std::env::var("TEST_MODE").is_ok() {
                                        Duration::from_millis(10)
                                    } else {
                                        Duration::from_secs(600)
                                    }
                                } else {
                                    if std::env::var("INTEGRATION_TEST").is_ok() || std::env::var("TEST_MODE").is_ok() {
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
                        }
                        return Err(ProcessError::Summary(e));
                    }
                }
            };

            Ok(result)
        }.await;

        match process_video_result {
            Ok(result) => {
                total_input_tokens += result.input_tokens;
                total_output_tokens += result.output_tokens;
                total_thinking_tokens += result.thinking_tokens;
                total_cost += result.cost;

                combined_thinking.push_str(&result.thinking_text);
                combined_summary.push_str(&result.summary_text);
            }
            Err(err) => {
                tracing::error!(url = %url, error = %err, "Failed to process video");
                let error_card = format!(
                    "Error: {}",
                    format_process_error(&err)
                );
                let _ = db::update_summary_full(db_pool, identifier, &error_card).await;
                combined_summary.push_str(&error_card);
            }
        }
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
        assert!(formatted.contains("rate limited"));
    }

    #[test]
    fn test_format_process_error_quota() {
        let err = ProcessError::Summary(SummaryError::ApiError("You exceeded your current quota, please check your plan and billing details.".to_string()));
        let formatted = format_process_error(&err);
        assert_eq!(formatted, "You exceeded your current quota, please check your plan and billing details.");
    }

    #[test]
    fn test_get_fallback_chain_new_models() {
        let chain_36_flash = get_fallback_chain("gemini-3.6-flash");
        assert_eq!(chain_36_flash[0], "gemini-3.6-flash");
        assert!(chain_36_flash.contains(&"gemini-3.5-flash"));
        assert!(chain_36_flash.contains(&"gemini-3.5-flash-lite"));

        let chain_35_lite = get_fallback_chain("gemini-3.5-flash-lite");
        assert_eq!(chain_35_lite[0], "gemini-3.5-flash-lite");
        assert!(chain_35_lite.contains(&"gemini-3.1-flash-lite"));
        assert!(chain_35_lite.contains(&"gemini-2.5-flash-lite"));
    }
}

