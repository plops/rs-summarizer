---
name: testing
description: Use when adding unit tests, running cargo test, checking test coverage, or matching Python ground truth fixtures for utils.
inclusion: manual
---

# Unit and Integration Tests

## Overview

rs-summarizer has two test layers: unit tests (fast, offline, in `src/`) and integration tests (require network + API keys, in `tests/`). Unit tests validate pure logic against Python ground truth. Integration tests exercise the real external services and verify the full pipeline lifecycle.

## Running Tests

```bash
# All unit tests (fast, no network)
cargo test

# Specific module
cargo test utils::url_validator
cargo test services::summary
cargo test cache

# Integration tests (require network + API key)
GEMINI_API_KEY=$(cat ~/api_key.txt) cargo test --test integration_pipeline -- --ignored
cargo test --test integration_transcript -- --ignored

# Single integration test with output
GEMINI_API_KEY=$(cat ~/api_key.txt) cargo test --test integration_pipeline test_summary_done_flag_transitions -- --ignored --nocapture

# All tests including integration
GEMINI_API_KEY=$(cat ~/api_key.txt) cargo test -- --include-ignored
```

## Unit Tests

Unit tests live alongside the code in `#[cfg(test)] mod tests` blocks. They cover:

### utils/url_validator.rs
- YouTube URL pattern matching (watch, live, shorts, youtu.be, mobile)
- HTTPS enforcement, 11-char ID extraction
- Ported from Python `t01_validate_youtube_url.py`

### utils/vtt_parser.rs
- WebVTT parsing with timestamp truncation
- Consecutive duplicate deduplication
- Byte-for-byte match with Python output (`t02_parse_vtt_file.py`)
- Uses fixture: `tests/fixtures/cW3tzRzTHKI.en.vtt`

### utils/markdown_converter.rs
- `**bold**` → `*bold*` conversion
- `## Heading` → `*Heading*` conversion
- URL dot replacement, punctuation repositioning
- Ported from Python `t03_convert_markdown_to_youtube_format.py`

### utils/timestamp_linker.rs
- MM:SS and HH:MM:SS timestamp detection
- Anchor tag generation with correct `t=` offset
- Invalid URL passthrough, canonical URL form
- Ported from Python `t04_convert_html_timestamps_to_youtube_links.py`

### services/summary.rs
- Cost computation formula
- Prompt building
- Transcript length validation boundaries (30 and 280,000 words)
- Rate limit error detection

### services/embedding.rs
- Cosine similarity (identical, orthogonal, opposite vectors)
- Matryoshka dimension truncation
- Zero-magnitude handling
- Embedding byte serialization/deserialization

### services/rate_limiter.rs
- Daily counter reset logic
- Per-model limit enforcement

## Integration Tests

Integration tests live in `tests/` and are all marked `#[ignore]` (require external services).

### tests/integration_transcript.rs

Tests yt-dlp subtitle download. Requires network + Firefox cookies.

| Test | What it verifies |
|------|-----------------|
| `test_list_subtitles_real_video` | yt-dlp can list available subtitle languages |
| `test_download_auto_subtitles` | VTT file is created on disk for auto-generated captions |
| `test_full_transcript_pipeline` | List → download → parse produces valid timestamped transcript |

### tests/integration_pipeline.rs

Tests the full summarization pipeline. Requires network + `GEMINI_API_KEY`.

| Test | What it verifies | Needs API? |
|------|-----------------|------------|
| `test_transcript_download` | TranscriptService downloads real video transcript | No (network only) |
| `test_summary_generation` | Gemini generates summary, tokens/cost recorded | Yes |
| `test_embedding_computation` | Embedding API returns correct dimensions | Yes |
| `test_cosine_similarity_integration` | Cosine similarity math (no network) | No |
| `test_full_pipeline_end_to_end` | Transcript → summary → YouTube format → embedding | Yes |
| `test_summary_done_flag_transitions` | `summary_done` goes false→true after process_summary | Yes |
| `test_timestamps_done_after_pipeline` | `timestamps_done` set, YouTube format populated | Yes |
| `test_error_sets_summary_done` | Short transcript error still sets `summary_done=true` | No |
| `test_invalid_model_sets_summary_done` | Unknown model error still sets `summary_done=true` | No |
| `test_polling_lifecycle_simulation` | Simulates HTMX polling, verifies it terminates | Yes |

### Test Patterns

**In-memory SQLite**: Integration tests use `db::init_db("sqlite::memory:")` for isolated, fast databases.

**AppState construction**: Tests that exercise `process_summary()` build a full `AppState`:
```rust
async fn build_test_app_state() -> AppState {
    let api_key = get_api_key();
    let db_pool = db::init_db("sqlite::memory:").await.unwrap();
    AppState {
        db: db_pool,
        model_options: Arc::new(vec![test_model()]),
        model_counts: Arc::new(RwLock::new(HashMap::new())),
        last_reset_day: Arc::new(RwLock::new(None)),
        gemini_api_key: api_key,
    }
}
```

**Graceful rate-limit handling**: Tests check error strings for `429`, `ResourceExhausted`, or `rate` and skip (print + return) instead of panicking:
```rust
Err(e) => {
    let err_str = e.to_string();
    if err_str.contains("429") || err_str.contains("ResourceExhausted") {
        println!("SKIPPED: API rate-limited: {}", err_str);
        return;
    }
    panic!("Failed: {}", e);
}
```

**Lifecycle simulation**: The `test_polling_lifecycle_simulation` test spawns the background task and polls the DB in a loop (like HTMX would), asserting that `summary_done` eventually becomes true within a timeout.

### tests/integration_browser.rs

Browser integration tests using fantoccini (WebDriver) + geckodriver + headless Firefox. They verify end-to-end user-facing behavior including HTMX interactions, form submissions, pagination, and accessibility. Requires geckodriver + Firefox installed.

```bash
# Run all browser tests
cargo test --test integration_browser -- --ignored

# Run a specific browser test
cargo test --test integration_browser test_browse_pagination_page_0 -- --ignored
```

| Test | Port | What it verifies | Needs API? |
|------|------|-----------------|------------|
| `test_index_page_loads` | 4444 | Index page renders form correctly | No |
| `test_browse_page_empty` | 4445 | Browse page loads with empty DB | No |
| `test_form_submission_shows_processing` | 4446 | Form submission triggers HTMX polling | No |
| `test_invalid_model_shows_error` | 4447 | Invalid model shows error message | No |
| `test_navigation_between_pages` | 4448 | Navigation links work | No |
| `test_static_assets_loaded` | 4449 | CSS and JS served correctly | No |
| `test_search_form_htmx` | 4450 | Search form submits via HTMX | No |
| `test_full_summarization_e2e` | 4451 | Full summarization lifecycle | Yes |
| `test_deduplication_returns_same_id` | 4452 | Duplicate URL returns same identifier | No |
| `test_rate_limit_error_display` | 4453 | Rate limit shows error, no polling | No |
| `test_polling_stops_on_error` | 4454 | Error stops HTMX polling | No |
| `test_form_required_validation` | 4455 | Empty URL blocked by validation | No |
| `test_browse_pagination_page_0` | 4456 | 20 articles on page 0, Next link | No |
| `test_browse_pagination_page_1` | 4457 | 5 articles on page 1, Previous link | No |
| `test_browse_no_next_on_last_page` | 4458 | No Next link on last page | No |
| `test_summary_markdown_rendering` | 4459 | Markdown → HTML (strong, li, h2) | No |
| `test_timestamp_links_rendered` | 4460 | Timestamps → YouTube links with &t= | No |
| `test_search_returns_results` | 4461 | Search with embeddings returns results | Yes |
| `test_search_empty_results` | 4462 | Empty DB shows "No results found" | Yes |
| `test_concurrent_submissions` | 4463–4464 | Two browsers get distinct IDs | No |
| `test_server_restart_recovery` | 4465 | Browser recovers after server restart | No |
| `test_aria_busy_during_generation` | 4466 | aria-busy present/absent correctly | No |
| `test_form_input_labels` | 4467 | Labels for URL, model, search inputs | No |
| `test_keyboard_navigation` | 4468 | Tab order and Enter submission | No |

## Adding New Tests

### Unit test
Add to the `#[cfg(test)] mod tests` block in the relevant source file. No special setup needed.

### Integration test (pipeline/transcript)
1. Add to `tests/integration_pipeline.rs` or `tests/integration_transcript.rs`
2. Mark with `#[tokio::test]` and `#[ignore]`
3. Use `get_api_key()` for Gemini tests
4. Handle rate-limiting gracefully (skip, don't panic)
5. Use `db::init_db("sqlite::memory:")` for database
6. Print progress for `--nocapture` visibility

### Browser integration test
1. Add to `tests/integration_browser.rs`
2. Mark with `#[tokio::test]` and `#[ignore]`
3. Use a unique geckodriver port (next available after 4468)
4. Use `start_test_server()` or `start_test_server_with_state()` for the server
5. Use `seed_summaries()` / `seed_summary_with_timestamps()` for pre-seeded data
6. Always clean up: `client.close()` + `geckodriver.kill()`
7. For POST endpoints, use JavaScript fetch pattern (see integration-testing skill)

## Test Fixture Files

- `tests/fixtures/cW3tzRzTHKI.en.vtt` — Real WebVTT subtitle file used for VTT parser ground-truth testing
