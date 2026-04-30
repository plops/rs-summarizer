# Implementation Plan: rs-summarizer

## Overview

Incremental implementation of the rs-summarizer Rust web application, a port of the Python RocketRecap YouTube transcript summarizer. Tasks are ordered to build utility modules first (with ground-truth tests from Python), then the database layer, service layer, web layer, and finally integration/property-based tests. Each task builds on previous work so there is no orphaned code.

## Tasks

- [ ] 1. Project scaffolding and core setup
  - [x] 1.1 Initialize Cargo project and configure dependencies
    - Run `cargo init` in the `rs-summarizer/` directory
    - Set up `Cargo.toml` with all dependencies: tokio, axum, sqlx (sqlite, runtime-tokio, macros), yt-dlp, gemini-rust, serde, askama, regex, tracing, tracing-subscriber, thiserror, anyhow, chrono, tower-http, proptest (dev)
    - Create directory structure: `src/`, `src/services/`, `src/utils/`, `src/routes/`, `src/templates/`, `migrations/`, `tests/`, `tests/fixtures/`, `static/`
    - _Requirements: 11.1, 11.2, 11.5_

  - [x] 1.2 Create SQLite migration file
    - Write `migrations/001_initial.sql` with the full `summaries` table schema matching the `Summary` struct from the design
    - Include composite index on `(original_source_link, model, summary_timestamp_start)`
    - Configure WAL journal mode in the migration or connection options
    - _Requirements: 11.1, 11.4, 11.5_

  - [x] 1.3 Define core data models and AppState
    - Create `src/models.rs` with `Summary`, `SubmitForm`, `SearchForm`, `BrowseParams` structs
    - Create `src/state.rs` with `AppState` struct and `ModelOption` configuration
    - Define error types with `thiserror` for each service module
    - _Requirements: 11.3, 13.1_

- [ ] 2. Utility modules with ground-truth tests
  - [x] 2.1 Implement YouTube URL validator
    - Create `src/utils/url_validator.rs` with `validate_youtube_url(url: &str) -> Option<String>`
    - Support all URL patterns: watch, live, shorts, youtu.be, mobile
    - Enforce HTTPS-only and 11-character ID constraint
    - Add unit tests ported from `t01_validate_youtube_url.py` (9 assertions)
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9_

  - [ ]* 2.2 Write property tests for URL validator
    - **Property 1: URL Validator Output Invariant** — For any input where validate_youtube_url returns Some(id), id is exactly 11 chars from [A-Za-z0-9_-]
    - **Property 2: URL Validator Rejects Invalid URLs** — For any non-HTTPS or non-YouTube URL, returns None
    - **Validates: Requirements 1.1–1.9**

  - [x] 2.3 Implement VTT parser
    - Create `src/utils/vtt_parser.rs` with `parse_vtt(vtt_content: &str) -> String`
    - Use the `vtt` crate (v1.0) to parse WebVTT content via `WebVtt::from_str()`
    - Extract cue start timestamps and payload text from parsed `VttCue` structs
    - Implement timestamp truncation to second granularity (strip milliseconds from `VttTimestamp`)
    - Implement consecutive duplicate line deduplication
    - Handle multi-line cues (use last line only from `cue.payload`)
    - Copy `cW3tzRzTHKI.en.vtt` fixture to `tests/fixtures/`
    - Add unit test verifying byte-for-byte match with Python output from `t02_parse_vtt_file.py`
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_

  - [ ]* 2.4 Write property tests for VTT parser
    - **Property 3: VTT Parser Output Format** — Every output line matches `HH:MM:SS caption_text\n` with no milliseconds
    - **Property 4: VTT Parser Deduplication** — No consecutive lines have identical caption text
    - **Validates: Requirements 2.1, 2.2, 2.3**

  - [x] 2.5 Implement markdown to YouTube format converter
    - Create `src/utils/markdown_converter.rs` with `convert_markdown_to_youtube_format(text: &str) -> String`
    - Convert `**word**` to `*word*`, `## Heading` to `*Heading*`
    - Replace dots in URLs with `-dot-`
    - Reposition punctuation adjacent to bold markers
    - Add unit test from `t03_convert_markdown_to_youtube_format.py`
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

  - [ ]* 2.6 Write property tests for markdown converter
    - **Property 5: Markdown Converter Eliminates Bold Markers** — Output never contains `**`
    - **Property 6: Markdown Converter URL Dot Replacement** — URLs with dots have them replaced with `-dot-`
    - **Validates: Requirements 3.1, 3.2, 3.3, 3.5**

  - [x] 2.7 Implement HTML timestamp to YouTube link converter
    - Create `src/utils/timestamp_linker.rs` with `replace_timestamps_in_html(html: &str, youtube_url: &str) -> String`
    - Parse MM:SS and HH:MM:SS timestamps, compute seconds offset
    - Generate anchor tags with canonical `watch?v=ID&t=Ns` form
    - Return HTML unchanged if URL is invalid
    - Add unit tests from `t04_convert_html_timestamps_to_youtube_links.py` (4 test cases)
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

  - [ ]* 2.8 Write property tests for timestamp linker
    - **Property 7: Timestamp Linker Correct Time Offset** — Anchor `t` param equals H*3600 + M*60 + S
    - **Property 8: Timestamp Linker Invalid URL Passthrough** — Invalid YouTube URL returns HTML unchanged
    - **Property 9: Timestamp Linker Canonical URL Form** — Generated links use `watch?v=ID` without extra params
    - **Validates: Requirements 4.1, 4.2, 4.3, 4.4, 4.5**

  - [x] 2.9 Create `src/utils/mod.rs` to export all utility modules
    - Wire up url_validator, vtt_parser, markdown_converter, timestamp_linker as public modules
    - _Requirements: 1.1–4.5_

- [x] 3. Checkpoint - Verify utility modules
  - Ensure all tests pass (`cargo test`), ask the user if questions arise.

- [ ] 4. Database layer
  - [x] 4.1 Implement database initialization and connection pool
    - Create `src/db.rs` with pool initialization function
    - Configure WAL mode, connection pool (default 5 connections)
    - Run migrations at startup via `sqlx::migrate!()`
    - _Requirements: 11.1, 11.2, 11.5_

  - [x] 4.2 Implement CRUD operations for summaries
    - Add `insert_new_summary()` function with parameterized bindings
    - Add `fetch_summary()` by identifier
    - Add `update_transcript()`, `update_summary_chunk()`, `mark_summary_done()`, `mark_timestamps_done()`
    - Add `store_embedding()` function for blob storage
    - Add `fetch_all_embeddings()` for similarity search
    - Add `fetch_browse_page()` with pagination (20 per page, ordered by id DESC)
    - _Requirements: 11.3, 11.4, 10.5, 7.2_

  - [ ]* 4.3 Write unit tests for database operations
    - Test insert and fetch round-trip
    - Test pagination returns correct page size and ordering
    - Test embedding blob storage size matches dimensions × 4
    - **Property 13: Embedding Storage Size Invariant**
    - **Validates: Requirements 7.2, 10.5, 11.3**

- [ ] 5. Service layer
  - [x] 5.1 Implement deduplication service
    - Create `src/services/deduplication.rs` with `DeduplicationService` struct
    - Implement `check_duplicate()` by URL + model within 5-minute window
    - Implement `check_duplicate_by_transcript()` by transcript + model within window
    - Use parameterized queries with the composite index
    - _Requirements: 8.1, 8.2, 8.3, 8.4_

  - [ ]* 5.2 Write property tests for deduplication
    - **Property 15: Deduplication Within Window** — Matching entries within window return Some, outside return None
    - **Validates: Requirements 8.1, 8.2, 8.3**

  - [x] 5.3 Implement transcript service
    - Create `src/services/transcript.rs` with `TranscriptService` struct
    - Implement `download_transcript()` using yt-dlp crate
    - Implement `pick_best_language()` with priority ordering
    - Integrate VTT parser for subtitle parsing
    - Store temp files in `/dev/shm`, clean up on all paths (including errors)
    - Define `TranscriptError` enum
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6_

  - [ ]* 5.4 Write unit tests for language selection
    - **Property 10: Language Selection Priority** — Test priority ordering with various subtitle listings
    - **Validates: Requirement 5.2**

  - [x] 5.5 Implement summary service
    - Create `src/services/summary.rs` with `SummaryService` struct
    - Implement `generate_summary()` with Gemini streaming, persisting chunks to DB progressively
    - Implement `build_prompt()` with transcript input
    - Implement `compute_cost()` for token-based pricing
    - Handle rate limiting (ResourceExhausted) by appending error without setting summary_done
    - Validate transcript length (30–280,000 words)
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7_

  - [ ]* 5.6 Write unit tests for summary service
    - **Property 17: Transcript Length Validation** — Transcripts < 30 words are rejected
    - Test cost computation formula
    - **Validates: Requirements 6.5, 6.6**

  - [x] 5.7 Implement embedding service
    - Create `src/services/embedding.rs` with `EmbeddingService` struct
    - Implement `embed_text()` via Gemini embedding model
    - Implement `cosine_similarity()` with Matryoshka truncation
    - Implement `find_similar()` returning top-k results by descending similarity
    - Handle zero-magnitude vectors (return 0.0)
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7_

  - [ ]* 5.8 Write property tests for embedding service
    - **Property 11: Cosine Similarity Bounded Output** — Result always in [-1.0, 1.0], zero-magnitude returns 0.0
    - **Property 12: Cosine Similarity Matryoshka Truncation** — Different-length vectors truncated to min length
    - **Property 14: Similarity Search Ranking** — Results ordered by descending similarity, top-k respected
    - **Validates: Requirements 7.3, 7.4, 7.5, 7.6**

  - [x] 5.9 Implement rate limiting and daily counter reset
    - Create `src/services/rate_limiter.rs` with per-model request counters
    - Implement daily reset logic using America/Los_Angeles timezone
    - Integrate with AppState's `model_counts` and `last_reset_day`
    - _Requirements: 13.1, 13.2, 13.3_

  - [x] 5.10 Create `src/services/mod.rs` to export all service modules
    - Wire up deduplication, transcript, summary, embedding, rate_limiter as public modules
    - _Requirements: 5.1–8.4, 13.1–13.3_

- [x] 6. Checkpoint - Verify service layer
  - Ensure all tests pass (`cargo test`), ask the user if questions arise.

- [ ] 7. Background task processing
  - [x] 7.1 Implement the background task orchestrator
    - Create `src/tasks.rs` with `process_summary()` async function
    - Implement `wait_until_row_exists()` with retry/backoff (100ms interval, 400 max attempts)
    - Wire together: transcript download → validation → summary generation → YouTube format conversion → embedding
    - Handle errors by storing message in summary field and setting summary_done=true
    - Ensure embedding failure doesn't fail the overall task
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5_

- [ ] 8. Web layer and templates
  - [x] 8.1 Create HTML templates with askama
    - Create `src/templates/` directory with askama template files
    - `index.html` — main page with submission form (URL input, model selector)
    - `generation_partial.html` — HTMX polling div for progressive summary display
    - `browse.html` — paginated browse page (20 per page)
    - `search_results.html` — similarity search results
    - Include pico.css and HTMX from static assets
    - Enable auto-escaping for XSS prevention
    - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5, 14.3_

  - [x] 8.2 Implement route handlers
    - Create `src/routes/mod.rs` with all route handler functions
    - `GET /` → index page with form
    - `POST /process_transcript` → accept submission, check dedup, spawn background task, return HTMX polling partial
    - `POST /generations/{identifier}` → polling endpoint returning current partial summary or final result
    - `GET /browse` → paginated browse page from metadata cache
    - `POST /search` → similarity search endpoint
    - Integrate rate limiting check before spawning tasks
    - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 14.1, 14.2, 14.4_

  - [x] 8.3 Implement in-memory metadata cache
    - Create `src/cache.rs` with metadata cache struct
    - Load all summary metadata at startup
    - Refresh cache when new summaries complete
    - Implement duplicate grouping for consecutive identical summaries in browse view
    - _Requirements: 12.1, 12.2, 12.3, 12.4_

  - [ ]* 8.4 Write property tests for metadata cache
    - **Property 19: Metadata Cache Duplicate Grouping** — Consecutive entries with identical summaries are collapsed into single groups
    - **Validates: Requirement 12.4**

  - [x] 8.5 Wire up the axum router and main entry point
    - Create `src/main.rs` with tokio main function
    - Initialize database pool with WAL mode
    - Run migrations
    - Build AppState with all services
    - Configure axum Router with all routes and static file serving (tower-http)
    - Bind to `0.0.0.0:5001` and start server
    - Load Gemini API key from environment variable
    - _Requirements: 11.1, 11.2, 11.5, 14.1_

  - [x] 8.6 Add static assets
    - Add `static/pico.min.css` for styling
    - Add `static/htmx.min.js` for HTMX functionality
    - Configure tower-http to serve the `static/` directory
    - _Requirements: 10.1, 10.4_

- [x] 9. Checkpoint - Verify compilation and basic routes
  - Ensure `cargo build` succeeds and all tests pass, ask the user if questions arise.

- [ ] 10. Integration and remaining property tests
  - [ ]* 10.1 Write property test for streaming monotonicity
    - **Property 16: Streaming Monotonicity** — After persisting chunk K, DB summary equals concatenation of chunks 1..K
    - **Validates: Requirement 6.2**

  - [ ]* 10.2 Write property test for browse pagination
    - **Property 18: Browse Pagination** — Returns at most 20 summaries at correct offset, ordered by id DESC
    - **Validates: Requirement 10.5**

  - [ ]* 10.3 Write property test for daily counter reset
    - **Property 20: Daily Counter Reset** — Counter resets exactly once when calendar day changes (America/Los_Angeles)
    - **Validates: Requirement 13.2**

  - [ ]* 10.4 Write integration tests for full pipeline
    - Test end-to-end flow with mocked yt-dlp and Gemini API
    - Test deduplication prevents duplicate processing
    - Test HTMX polling returns progressive updates
    - Test error handling stores error in summary field
    - _Requirements: 9.1–9.5, 10.1–10.3_

- [x] 11. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass (`cargo test`), ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties from the design document
- Unit tests use ground-truth assertions ported from Python test files (t01–t04)
- The VTT test fixture `cW3tzRzTHKI.en.vtt` must be copied from the source04 project into `tests/fixtures/`
- All code uses Rust with the crates specified in the design document's dependency table
- **Python dependency management**: Use `uv` (not pip) to install any Python dependencies needed during development or testing (e.g., for running Python ground-truth test scripts). Use `uv pip install` or `uvx` for one-off tool execution.
- **VTT parsing**: Use the `vtt` crate (v1.0, https://github.com/Govcraft/vtt) for parsing WebVTT files instead of manual regex-based parsing. Parse via `WebVtt::from_str()`, iterate `cues` for `VttCue` structs with `start: VttTimestamp`, `end: VttTimestamp`, and `payload: String`.
