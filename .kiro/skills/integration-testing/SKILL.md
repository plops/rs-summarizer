---
name: integration-testing
description: Use when writing or running integration tests that call the Gemini API, yt-dlp, or browser tests with geckodriver and fantoccini.
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

## Browser Integration Tests

Browser integration tests drive a real headless Firefox browser against the application server using fantoccini (WebDriver) + geckodriver. They verify end-to-end user-facing behavior including HTMX interactions, form submissions, pagination, and accessibility.

### Prerequisites

- geckodriver in PATH (or `~/bin/geckodriver`)
- Firefox installed
- `GEMINI_API_KEY` for search tests only

### Running Browser Tests

```bash
# All browser integration tests
cargo test --test integration_browser -- --ignored

# A specific browser test
cargo test --test integration_browser test_browse_pagination_page_0 -- --ignored

# With output for debugging
cargo test --test integration_browser -- --ignored --nocapture
```

### Test Harness

Each browser test:
1. Starts an axum server on a random port with in-memory SQLite
2. Starts geckodriver on a unique port (4444–4468)
3. Connects headless Firefox via WebDriver
4. Performs browser interactions and DOM assertions
5. Cleans up (close browser, kill geckodriver)

### Helper Functions

| Helper | Purpose |
|--------|---------|
| `test_app_state()` | Creates default AppState with in-memory DB |
| `start_test_server()` | Starts server with default state on random port |
| `start_test_server_with_state(state)` | Starts server with pre-configured AppState |
| `start_test_server_controllable(state)` | Returns shutdown handle for graceful restart testing |
| `seed_summaries(db, count)` | Inserts `count` summaries with markdown content |
| `seed_summary_with_timestamps(db, url)` | Inserts a summary with timestamped content |
| `test_app_state_with_low_limit()` | Creates AppState with `rpd_limit=1` for rate limit testing |

### Port Allocation

| Port Range | Tests |
|---|---|
| 4444–4451 | Original tests (index, browse, form, navigation, assets, search, e2e) |
| 4452–4468 | Extended tests (HTMX behavior, pagination, search, concurrency, accessibility) |

### JavaScript Fetch Pattern

For testing generation partials (POST endpoints), tests use JavaScript fetch:

```rust
let script = format!(
    r#"
    const response = await fetch('/generations/{}', {{ method: 'POST' }});
    const html = await response.text();
    document.getElementById('result').innerHTML = html;
    return html;
    "#,
    id
);
let result: serde_json::Value = client
    .execute(&format!("return (async () => {{ {} }})()", script), vec![])
    .await.unwrap();
```

## Relevant Files

- `tests/integration_transcript.rs` — yt-dlp download tests
- `tests/integration_pipeline.rs` — Full pipeline tests
- `tests/integration_browser.rs` — Browser integration tests (fantoccini + geckodriver)
- `src/lib.rs` — Module re-exports for test access
