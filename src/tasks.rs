use std::time::Duration;

use chrono::Utc;
use sqlx::SqlitePool;
use tokio::time::sleep;
use tracing;

use crate::db;
use crate::errors::ProcessError;
use crate::models::Summary;
use crate::services::embedding::{embedding_to_bytes, EmbeddingService};
use crate::services::summary::SummaryService;
use crate::services::transcript::TranscriptService;
use crate::state::{AppState, ModelOption};
use crate::utils::markdown_converter::convert_markdown_to_youtube_format;

/// Core background task that orchestrates the full summarization pipeline.
/// Spawned by tokio after a new summary row is inserted.
///
/// Requirements: 9.1, 9.2, 9.3, 9.4, 9.5
pub async fn process_summary(db_pool: SqlitePool, identifier: i64, app: AppState) {
    if let Err(e) = process_summary_inner(&db_pool, identifier, &app).await {
        tracing::error!(identifier = identifier, error = %e, "Processing failed");
        mark_error(&db_pool, identifier, &e.to_string()).await;
    }
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

    // Step 2: Download transcript if not provided
    if summary.transcript.is_empty() {
        let transcript = transcript_svc
            .download_transcript(&summary.original_source_link, identifier)
            .await?;
        db::update_transcript(db_pool, identifier, &transcript).await?;
    }

    // Step 3: Validate transcript
    let summary = db::fetch_summary(db_pool, identifier)
        .await?
        .ok_or(ProcessError::RowNotFound)?;
    let word_count = summary.transcript.split_whitespace().count();
    if word_count < 30 {
        return Err(ProcessError::TranscriptTooShort);
    }
    if word_count > 280_000 {
        return Err(ProcessError::TranscriptTooLong(word_count));
    }

    // Step 4: Parse model option
    let model = parse_model_option(&summary.model, &app.model_options)?;

    // Step 5: Generate summary (streaming, updates DB progressively)
    let result = summary_svc
        .generate_summary(db_pool, identifier, &summary.transcript, &model)
        .await?;

    // Step 5b: Mark summary as done (stops HTMX polling on the frontend)
    let timestamp_end = Utc::now().to_rfc3339();
    db::mark_summary_done(
        db_pool,
        identifier,
        result.input_tokens as i64,
        result.output_tokens as i64,
        result.cost,
        &timestamp_end,
    )
    .await?;

    // Step 6: Convert to YouTube format and mark timestamps_done
    let youtube_text = convert_markdown_to_youtube_format(&result.summary_text);
    db::mark_timestamps_done(db_pool, identifier, &youtube_text).await?;

    // Step 7: Compute and store embedding (non-fatal)
    match embedding_svc.embed_text(&result.summary_text).await {
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
    let _ = db::update_summary_chunk(db_pool, identifier, error_msg).await;
    let _ = db::mark_summary_done(db_pool, identifier, 0, 0, 0.0, "").await;
}
