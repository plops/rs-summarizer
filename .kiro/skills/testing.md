---
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

## Adding New Tests

### Unit test
Add to the `#[cfg(test)] mod tests` block in the relevant source file. No special setup needed.

### Integration test
1. Add to `tests/integration_pipeline.rs` or `tests/integration_transcript.rs`
2. Mark with `#[tokio::test]` and `#[ignore]`
3. Use `get_api_key()` for Gemini tests
4. Handle rate-limiting gracefully (skip, don't panic)
5. Use `db::init_db("sqlite::memory:")` for database
6. Print progress for `--nocapture` visibility

## Test Fixture Files

- `tests/fixtures/cW3tzRzTHKI.en.vtt` — Real WebVTT subtitle file used for VTT parser ground-truth testing
