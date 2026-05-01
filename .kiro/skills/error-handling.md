---
name: error-handling
description: Use when adding new error variants, propagating errors between service layers, or working with the thiserror error hierarchy and the mark_error pattern.
inclusion: manual
---

# Error Handling Architecture

## Overview

rs-summarizer uses a layered error hierarchy built with `thiserror`. Each service layer has its own error enum, and `ProcessError` at the task orchestration layer wraps them all via `#[from]` conversions.

## Error Hierarchy

```
ProcessError (tasks.rs orchestrator)
├── Transcript(TranscriptError)   — via #[from]
├── Summary(SummaryError)         — via #[from]
├── Embedding(EmbeddingError)     — via #[from]
├── Database(sqlx::Error)         — via #[from]
├── RowNotFound                   — wait_until_row_exists timeout
├── TranscriptTooShort            — < 30 words
└── TranscriptTooLong(usize)      — > 280,000 words
```

## Service-Level Errors

### TranscriptError (yt-dlp / download layer)

```rust
pub enum TranscriptError {
    InvalidUrl(String),      // URL validation failed
    NoSubtitles,             // Video has no captions
    YtDlpFailed(String),     // yt-dlp process error (429, auth, etc.)
    Timeout(u64),            // Download exceeded timeout
    ParseError(String),      // VTT parsing failed
}
```

### SummaryError (Gemini generation layer)

```rust
pub enum SummaryError {
    ApiError(String),        // Gemini API returned an error
    RateLimited,             // ResourceExhausted / 429
    TranscriptTooShort,      // < 30 words
    TranscriptTooLong(usize, usize),  // (actual, max) words
}
```

### EmbeddingError (embedding layer)

```rust
pub enum EmbeddingError {
    ApiError(String),        // Embedding API error
    EmptyText,               // Empty input
    DbError(sqlx::Error),    // Database storage failed
}
```

## Error Propagation Pattern

In `tasks.rs`, the `?` operator auto-converts service errors into `ProcessError`:

```rust
async fn process_summary_inner(db: &SqlitePool, id: i64, app: &AppState) -> Result<(), ProcessError> {
    let transcript = transcript_svc.download(&url).await?;  // TranscriptError → ProcessError
    let result = summary_svc.generate(&text, &model).await?;  // SummaryError → ProcessError
    embedding_svc.embed_text(&text).await?;  // EmbeddingError → ProcessError
    Ok(())
}
```

## The mark_error Pattern

The outer `process_summary()` function catches ALL errors and stores them in the DB so the frontend can display them:

```rust
pub async fn process_summary(db_pool: SqlitePool, identifier: i64, app: AppState) {
    if let Err(e) = process_summary_inner(&db_pool, identifier, &app).await {
        mark_error(&db_pool, identifier, &e.to_string()).await;
    }
}

async fn mark_error(db_pool: &SqlitePool, identifier: i64, error_msg: &str) {
    let _ = db::update_summary_chunk(db_pool, identifier, error_msg).await;
    let _ = db::mark_summary_done(db_pool, identifier, 0, 0, 0.0, "").await;
}
```

**Critical invariant**: `mark_error` always sets `summary_done = true`, which stops HTMX polling. Without this, errors would cause infinite polling.

## Adding a New Error Variant

1. Add the variant to the appropriate enum in `src/errors.rs`
2. Include a `#[error("...")]` message (this becomes the user-visible text via `mark_error`)
3. If the new error comes from an external crate, add `#[from]` for automatic conversion
4. If it needs to propagate to `ProcessError`, add a wrapping variant there too

## Route-Level Error Handling

Route handlers don't use the error hierarchy — they return `Html<String>` directly:

```rust
let model = match model {
    Some(m) => m.clone(),
    None => return Html("<p>Invalid model selected.</p>".to_string()),
};
```

Only the background task uses the full error chain.

## Relevant Files

- `src/errors.rs` — All error enum definitions
- `src/tasks.rs` — `process_summary()`, `mark_error()`, error propagation
- `src/services/transcript.rs` — Returns `TranscriptError`
- `src/services/summary.rs` — Returns `SummaryError`
- `src/services/embedding.rs` — Returns `EmbeddingError`
