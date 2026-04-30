# rs-summarizer Project Context

## Overview

rs-summarizer is a Rust web application that summarizes YouTube video transcripts using Google Gemini AI. It downloads captions via yt-dlp (using Firefox cookies for authentication), generates streaming summaries, computes vector embeddings for similarity search, and stores everything in SQLite with WAL mode. The frontend uses HTMX for real-time progressive updates.

## Architecture

Layered architecture: Web (axum) → Services → Database (SQLite via sqlx).

- **Web layer**: axum routes + askama HTML templates + HTMX polling
- **Service layer**: transcript download, summary generation, embedding, deduplication, rate limiting
- **Data layer**: SQLite with WAL mode, in-memory metadata cache
- **Background tasks**: tokio::spawn for long-running summarization pipelines

## Key Technical Decisions

- yt-dlp is invoked via `uvx yt-dlp` (not installed globally) with `--cookies-from-browser firefox`
- Subtitle download uses `--format "mhtml" --skip-download --write-sub --write-auto-sub` to avoid format resolution errors
- Gemini API models use `Model::Custom(format!("models/{}", name))` — model names must match the API exactly (e.g. `gemini-3-flash-preview`, not `gemini-3-flash`)
- Embedding model is `gemini-embedding-001` (not `text-embedding-004`)
- Python dependencies (if needed) use `uv`, not pip

## Project Structure

```
rs-summarizer/
├── Cargo.toml
├── src/
│   ├── main.rs          # Entry point, router, model config
│   ├── lib.rs           # Re-exports for integration tests
│   ├── cache.rs         # In-memory metadata cache
│   ├── db.rs            # SQLite init + CRUD operations
│   ├── errors.rs        # Error enums (TranscriptError, SummaryError, etc.)
│   ├── models.rs        # Data structs (Summary, SubmitForm, etc.)
│   ├── state.rs         # AppState, ModelOption
│   ├── tasks.rs         # Background task orchestrator (process_summary)
│   ├── templates.rs     # Askama template structs
│   ├── routes/mod.rs    # Axum route handlers
│   ├── services/
│   │   ├── deduplication.rs  # 5-min window duplicate check
│   │   ├── embedding.rs      # Gemini embeddings + cosine similarity
│   │   ├── rate_limiter.rs   # Per-model daily counters
│   │   ├── summary.rs        # Gemini streaming summary generation
│   │   └── transcript.rs     # yt-dlp subtitle download + VTT parsing
│   └── utils/
│       ├── markdown_converter.rs  # ** → * YouTube format
│       ├── timestamp_linker.rs    # Timestamps → clickable YouTube links
│       ├── url_validator.rs       # YouTube URL → video ID extraction
│       └── vtt_parser.rs          # WebVTT → plain text with timestamps
├── templates/           # Askama HTML templates (index, browse, partials)
├── static/              # pico.min.css, htmx.min.js
├── migrations/          # SQLite schema (001_initial.sql)
├── tests/
│   ├── fixtures/        # cW3tzRzTHKI.en.vtt test fixture
│   ├── integration_transcript.rs  # yt-dlp download tests
│   └── integration_pipeline.rs    # Full pipeline tests (summary + embedding)
└── data/                # SQLite database (gitignored)
```

## Running

```bash
export GEMINI_API_KEY=$(cat ~/api_key.txt)
cargo run --release
# Server on http://localhost:5001
```

## Module Responsibilities

- `utils/` — Pure functions, no I/O, fully unit-tested against Python ground truth
- `services/` — Business logic with external dependencies (Gemini API, yt-dlp, SQLite)
- `routes/` — HTTP handlers, form parsing, HTMX response rendering
- `tasks.rs` — Orchestrates the full pipeline: download → validate → summarize → convert → embed
- `db.rs` — All SQL queries (parameterized, no string interpolation)
- `cache.rs` — In-memory metadata for fast browse/filter without hitting SQLite
