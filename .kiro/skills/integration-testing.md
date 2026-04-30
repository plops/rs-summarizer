---
inclusion: manual
---

# Integration Testing Approach

## Overview

Integration tests exercise real external services (YouTube via yt-dlp, Gemini API). They are marked `#[ignore]` so they don't run by default, and gracefully skip on rate limits.

## Running Integration Tests

```bash
# Transcript tests (requires network + Firefox cookies)
cargo test --test integration_transcript -- --ignored

# Full pipeline tests (requires network + GEMINI_API_KEY)
GEMINI_API_KEY=$(cat ~/api_key.txt) cargo test --test integration_pipeline -- --ignored

# With output visible
GEMINI_API_KEY=$(cat ~/api_key.txt) cargo test --test integration_pipeline -- --ignored --nocapture

# All tests including integration
GEMINI_API_KEY=$(cat ~/api_key.txt) cargo test -- --include-ignored
```

## Test Model Choice

Integration tests use `gemma-3-27b-it` (14,400 RPD) to avoid hitting rate limits during development. Gemini models have much lower quotas (20-500 RPD).

## Graceful Skip Pattern

Tests that hit external APIs gracefully skip on rate limits instead of failing:

```rust
match result {
    Ok(data) => { /* assertions */ }
    Err(e) => {
        let err_str = e.to_string();
        if err_str.contains("429") || err_str.contains("bot") || err_str.contains("authentication") {
            println!("SKIPPED: Rate-limited: {}", err_str);
            return;
        }
        panic!("Unexpected failure: {}", e);
    }
}
```

## In-Memory SQLite for Test Isolation

Tests that need a database use `sqlite::memory:` to avoid file system side effects:

```rust
let db_pool = db::init_db("sqlite::memory:").await.expect("Failed to init test DB");
```

## Test Structure

### `tests/integration_transcript.rs`
- `test_list_subtitles_real_video` — yt-dlp can list subs
- `test_download_auto_subtitles` — VTT file is created
- `test_full_transcript_pipeline` — List → download → verify content

### `tests/integration_pipeline.rs`
- `test_transcript_download` — TranscriptService end-to-end
- `test_summary_generation` — Gemini generates summary from text
- `test_embedding_computation` — Embedding API returns correct dimensions
- `test_cosine_similarity_integration` — Math verification
- `test_full_pipeline_end_to_end` — Transcript → summary → YouTube format → embedding

## Library Crate for Integration Tests

`src/lib.rs` re-exports all modules so integration tests can import them:

```rust
// In tests:
use rs_summarizer::db;
use rs_summarizer::services::embedding::EmbeddingService;
use rs_summarizer::services::summary::SummaryService;
```

## Relevant Files

- `tests/integration_transcript.rs` — yt-dlp download tests
- `tests/integration_pipeline.rs` — Full pipeline tests
- `src/lib.rs` — Module re-exports for test access
