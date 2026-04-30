---
inclusion: manual
---

# Background Task Orchestration

## Overview

rs-summarizer uses `tokio::spawn` to run the summarization pipeline in the background while the web layer remains responsive. The HTMX frontend polls for progress.

## Entry Point

File: `src/tasks.rs`

```rust
pub async fn process_summary(db_pool: SqlitePool, identifier: i64, app: AppState) {
    if let Err(e) = process_summary_inner(&db_pool, identifier, &app).await {
        mark_error(&db_pool, identifier, &e.to_string()).await;
    }
}
```

The outer function catches all errors and stores them in the summary field.

## Pipeline Steps

1. **Wait for row** — `wait_until_row_exists()` polls DB every 100ms, max 400 attempts (40s)
2. **Download transcript** — If `transcript` field is empty, download via yt-dlp
3. **Validate** — Reject if < 30 words or > 280,000 words
4. **Parse model** — Find matching `ModelOption` from config
5. **Generate summary** — Streaming via Gemini, chunks appended to DB progressively
6. **YouTube format** — Convert markdown to YouTube-compatible text, set `timestamps_done=true`
7. **Embedding** — Compute vector embedding (non-fatal if it fails)

## Spawning from Route Handler

```rust
// In process_transcript():
let app_clone = app.clone();
let db_clone = app.db.clone();
tokio::spawn(async move {
    tasks::process_summary(db_clone, id, app_clone).await;
});
```

## Wait for Row Pattern

The row is inserted by the route handler, but due to SQLite WAL mode there can be a brief delay before it's readable by the background task:

```rust
async fn wait_until_row_exists(
    db_pool: &SqlitePool,
    identifier: i64,
    interval: Duration,    // 100ms
    max_attempts: u32,     // 400
) -> Result<Summary, ProcessError> {
    for _ in 0..max_attempts {
        if let Some(summary) = db::fetch_summary(db_pool, identifier).await? {
            return Ok(summary);
        }
        sleep(interval).await;
    }
    Err(ProcessError::RowNotFound)
}
```

## Error Handling

All errors are caught and stored in the DB so the frontend can display them:

```rust
async fn mark_error(db_pool: &SqlitePool, identifier: i64, error_msg: &str) {
    let _ = db::update_summary_chunk(db_pool, identifier, error_msg).await;
    let _ = db::mark_summary_done(db_pool, identifier, 0, 0, 0.0, "").await;
}
```

**Critical invariant**: `summary_done` must always eventually become `true` — this is what stops HTMX polling.

## Non-Fatal Embedding

Embedding failure doesn't fail the overall task:

```rust
match embedding_svc.embed_text(&result.summary_text).await {
    Ok(embedding) => { /* store it */ }
    Err(e) => {
        tracing::warn!("Failed to compute embedding");
        // Continue — summary is still usable
    }
}
```

## Service Creation

Services are created on-the-fly from `AppState` (not stored as Arc in state):

```rust
let transcript_svc = TranscriptService::new("/dev/shm");
let summary_svc = SummaryService::new(app.gemini_api_key.clone());
let embedding_svc = EmbeddingService::new(app.gemini_api_key.clone(), "gemini-embedding-001", 3072);
```

## Relevant Files

- `src/tasks.rs` — Pipeline orchestrator
- `src/routes/mod.rs` — `tokio::spawn` call in `process_transcript()`
- `src/services/summary.rs` — Streaming chunk generation
- `src/db.rs` — `mark_summary_done()`, `mark_error()` pattern
