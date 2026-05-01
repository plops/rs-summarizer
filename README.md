# rs-summarizer

A Rust web application that summarizes YouTube video transcripts using Google Gemini AI. It downloads captions via yt-dlp, generates streaming summaries, computes vector embeddings for similarity search, and stores everything in SQLite. The frontend uses HTMX for real-time progressive updates.

## Prerequisites

- Rust toolchain (1.75+)
- [yt-dlp](https://github.com/yt-dlp/yt-dlp) installed and on PATH
- A Google Gemini API key

## Setup

```bash
# Clone and enter the project
cd rs-summarizer

# Set your Gemini API key
export GEMINI_API_KEY="your-api-key-here"

# Create the data directory (SQLite DB will be created automatically)
mkdir -p data
```

## Running

```bash
cargo run
```

The server starts on `http://0.0.0.0:5001`.

Open `http://localhost:5001` in your browser to use the web interface.

## Running in release mode

```bash
cargo run --release
```

## Running tests

```bash
cargo test
```

## Testing

### Unit tests

Unit tests run offline with no external dependencies. They cover the utility modules (URL validator, VTT parser, markdown converter, timestamp linker), service logic (cosine similarity, cost computation, rate limiting, language selection, metadata cache grouping), and are ported from the original Python test suite for byte-for-byte compatibility.

```bash
# Run all unit tests
cargo test

# Run tests for a specific module
cargo test utils::url_validator
cargo test services::embedding
cargo test cache
```

### Integration tests

Integration tests exercise the real external services (YouTube via yt-dlp, Gemini API). They require network access and a valid `GEMINI_API_KEY`. They are marked `#[ignore]` so they don't run by default.

```bash
# Run transcript download tests (requires network + yt-dlp)
cargo test --test integration_transcript -- --ignored

# Run summarization and embedding tests (requires network + GEMINI_API_KEY)
GEMINI_API_KEY=$(cat ~/api_key.txt) cargo test --test integration_pipeline -- --ignored

# Run a specific integration test with output
GEMINI_API_KEY=$(cat ~/api_key.txt) cargo test --test integration_pipeline test_summary_generation -- --ignored --nocapture

# Run all tests including integration
GEMINI_API_KEY=$(cat ~/api_key.txt) cargo test -- --include-ignored
```

**Integration test coverage:**

| Test | What it verifies |
|------|-----------------|
| `test_list_subtitles_real_video` | yt-dlp can list available subtitles |
| `test_download_auto_subtitles` | VTT file is created for auto-generated captions |
| `test_full_transcript_pipeline` | List → download → parse produces valid transcript |
| `test_transcript_download` | TranscriptService end-to-end download |
| `test_summary_generation` | Gemini generates a summary from transcript text |
| `test_embedding_computation` | Gemini embedding API returns correct dimensions |
| `test_cosine_similarity_integration` | Similarity math works correctly |
| `test_full_pipeline_end_to_end` | Transcript → summary → YouTube format → embedding |
| `test_summary_done_flag_transitions` | `summary_done` transitions false→true after pipeline |
| `test_timestamps_done_after_pipeline` | `timestamps_done` set, YouTube format populated |
| `test_error_sets_summary_done` | Error path still sets `summary_done=true` (spinner stops) |
| `test_invalid_model_sets_summary_done` | Invalid model error sets `summary_done=true` |
| `test_polling_lifecycle_simulation` | Simulates HTMX polling loop, verifies it terminates |

Note: Integration tests gracefully skip (instead of failing) when YouTube rate-limits or the Gemini API returns 429. Tests marked "No API" (`test_error_sets_summary_done`, `test_invalid_model_sets_summary_done`, `test_cosine_similarity_integration`) can run without `GEMINI_API_KEY`.

### Browser integration tests

Browser tests drive a real headless Firefox browser against the application server using fantoccini (WebDriver) + geckodriver. They verify end-to-end user-facing behavior including HTMX interactions, form validation, pagination, search, concurrency, and accessibility.

**Prerequisites:** geckodriver in PATH (or `~/bin/geckodriver`) + Firefox installed.

```bash
# Run all browser integration tests
cargo test --test integration_browser -- --ignored

# Run a specific browser test
cargo test --test integration_browser test_browse_pagination_page_0 -- --ignored

# With output for debugging
cargo test --test integration_browser -- --ignored --nocapture
```

**Browser test coverage (24 tests):**

| Category | Tests | What they verify |
|----------|-------|-----------------|
| Page loading | 3 | Index, browse, navigation render correctly |
| HTMX behavior | 5 | Form submission, deduplication, rate limiting, error handling, validation |
| Browse pagination | 3 | 20-per-page pagination, Next/Previous links |
| Content rendering | 2 | Markdown → HTML, timestamps → YouTube links |
| Search | 3 | HTMX search form, results with embeddings, empty results |
| Concurrency | 2 | Concurrent submissions isolated, server restart recovery |
| Accessibility | 3 | aria-busy states, form labels, keyboard navigation |
| End-to-end | 1 | Full summarization lifecycle (requires GEMINI_API_KEY) |

Note: Search tests (`test_search_returns_results`, `test_search_empty_results`) and the full e2e test require `GEMINI_API_KEY`. All other browser tests run without API keys.

## Environment variables

| Variable | Required | Description |
|----------|----------|-------------|
| `GEMINI_API_KEY` | Yes | Google Gemini API key for summary generation and embeddings |
| `RUST_LOG` | No | Log level filter (e.g. `info`, `debug`, `rs_summarizer=debug`) |

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Main page with submission form |
| POST | `/process_transcript` | Submit a YouTube URL for summarization |
| POST | `/generations/{id}` | Poll for summary progress (used by HTMX) |
| GET | `/browse` | Browse paginated summaries |
| POST | `/search` | Similarity search across summaries |
| GET | `/static/*` | Static assets (pico.css, htmx.min.js) |

## Project structure

```
rs-summarizer/
├── Cargo.toml
├── migrations/          # SQLite schema migrations
├── src/
│   ├── main.rs          # Entry point, router setup
│   ├── cache.rs         # In-memory metadata cache
│   ├── db.rs            # Database init and CRUD operations
│   ├── errors.rs        # Error types
│   ├── models.rs        # Data models (Summary, forms)
│   ├── state.rs         # AppState and ModelOption
│   ├── tasks.rs         # Background task orchestrator
│   ├── templates.rs     # Askama template structs
│   ├── routes/          # Axum route handlers
│   ├── services/        # Business logic services
│   │   ├── deduplication.rs
│   │   ├── embedding.rs
│   │   ├── rate_limiter.rs
│   │   ├── summary.rs
│   │   └── transcript.rs
│   └── utils/           # Utility modules
│       ├── markdown_converter.rs
│       ├── timestamp_linker.rs
│       ├── url_validator.rs
│       └── vtt_parser.rs
├── static/              # CSS and JS assets
├── templates/           # Askama HTML templates
└── tests/
    ├── fixtures/        # Test fixture files
    └── integration_browser.rs  # Browser tests (fantoccini + geckodriver)
```

## How it works

1. User submits a YouTube URL via the web form
2. The system checks for duplicate submissions (5-minute window)
3. A background task downloads subtitles via yt-dlp
4. The transcript is sent to Gemini for streaming summarization
5. Summary chunks are persisted to SQLite progressively
6. The frontend polls via HTMX and displays partial results in real-time
7. Once complete, the summary is converted to YouTube format and an embedding is computed for similarity search

## Available models

- `gemini-3-flash-preview` — best quality (5 RPM, 20 RPD)
- `gemini-3.1-flash-lite-preview` — best quota for bulk use (15 RPM, 500 RPD)
- `gemini-2.5-flash` — solid all-rounder (5 RPM, 20 RPD)
- `gemini-2.5-flash-lite` — lightweight (10 RPM, 20 RPD)
- `gemma-4-31b-it` — free, large open model (15 RPM, 1500 RPD)
- `gemma-4-26b-a4b-it` — free, efficient (15 RPM, 1500 RPD)
- `gemma-3-27b-it` — free, massive daily quota (30 RPM, 14400 RPD)
- `gemma-3-12b-it` — free, mid-size (30 RPM, 14400 RPD)
- `gemma-3-4b-it` — free, small (30 RPM, 14400 RPD)
- `gemma-3-1b-it` — free, tiny (30 RPM, 14400 RPD)
