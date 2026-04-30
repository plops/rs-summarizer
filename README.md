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
└── tests/fixtures/      # Test fixture files
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

- `gemini-2.0-flash` — fast, cost-effective (1500 requests/day)
- `gemini-2.5-flash-preview-04-17` — higher quality (500 requests/day)
