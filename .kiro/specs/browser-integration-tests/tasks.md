# Implementation Plan: Browser Integration Tests

## Overview

Add 16 browser integration tests to the existing `tests/integration_browser.rs` file, covering HTMX behavior, browse page pagination, search functionality, concurrency/resilience, and accessibility. Tests use the established fantoccini + geckodriver + headless Firefox harness with in-memory SQLite.

## Tasks

- [x] 1. Add helper functions and dependencies
  - [x] 1.1 Add `tokio-util` dev-dependency to `Cargo.toml` for `CancellationToken` support (used by server restart test)
    - Add `tokio-util = { version = "0.7", features = ["rt"] }` under `[dev-dependencies]`
    - Add `chrono` under `[dev-dependencies]` if not already available for test seeding
    - _Requirements: 5.1, 7.1, 11.1_

  - [x] 1.2 Implement `start_test_server_with_state()` helper
    - Accepts a pre-configured `AppState` and starts the axum server on a random port
    - Returns the base URL string
    - Place in `tests/integration_browser.rs` alongside existing helpers
    - _Requirements: 2.1, 5.1, 6.1, 7.1, 8.1_

  - [x] 1.3 Implement `start_test_server_controllable()` helper
    - Returns `(base_url, SocketAddr, tokio::sync::watch::Sender<bool>)` for graceful shutdown
    - Uses `axum::serve(...).with_graceful_shutdown(...)` pattern
    - _Requirements: 11.1, 11.2_

  - [x] 1.4 Implement `seed_summaries()` helper
    - Inserts `count` summary records with `summary_done=true` and markdown content
    - Returns `Vec<i64>` of inserted identifiers
    - Each record has unique YouTube URL, markdown with bold/lists/headings
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 6.1, 6.2, 6.3_

  - [x] 1.5 Implement `seed_summary_with_timestamps()` helper
    - Inserts a single summary with `timestamps_done=true` and timestamped content
    - Content includes timestamps like `0:00`, `1:30` that should become clickable links
    - _Requirements: 7.1, 7.2_

  - [x] 1.6 Implement `test_app_state_with_low_limit()` helper
    - Creates `AppState` with a model having `rpd_limit=1`
    - Uses model name `"test-limited-model"` to avoid conflicts
    - _Requirements: 2.1, 2.2_

- [x] 2. Implement HTMX behavior tests
  - [x] 2.1 Implement `test_deduplication_returns_same_id` (port 4452)
    - Submit a URL via the form, extract identifier from `#generation` div's `hx-post` attribute
    - Submit the same URL again, verify the second response contains the same identifier
    - Verify no new background task is spawned (same generation partial returned)
    - _Requirements: 1.1, 1.2_

  - [x] 2.2 Implement `test_rate_limit_error_display` (port 4453)
    - Use `test_app_state_with_low_limit()` and `start_test_server_with_state()`
    - Submit once (succeeds, exhausts the `rpd_limit=1`)
    - Submit again, verify response contains "Rate limit exceeded"
    - Verify no `hx-post` attribute present in the error response
    - _Requirements: 2.1, 2.2_

  - [x] 2.3 Implement `test_polling_stops_on_error` (port 4454)
    - Inject invalid model via JavaScript (like existing `test_invalid_model_shows_error`)
    - Submit form, wait for HTMX response
    - Verify `#result` contains error message
    - Verify `#generation` element does NOT have `hx-trigger` attribute
    - _Requirements: 3.1, 3.2_

  - [x] 2.4 Implement `test_form_required_validation` (port 4455)
    - Verify URL input has `required` attribute in DOM
    - Click submit without entering a URL
    - Verify page remains on index (no HTMX request fired)
    - Verify `#result` div is still empty
    - _Requirements: 4.1, 4.2_

- [x] 3. Implement browse page tests
  - [x] 3.1 Implement `test_browse_pagination_page_0` (port 4456)
    - Seed 25 summaries via `seed_summaries()`
    - Start server with seeded state via `start_test_server_with_state()`
    - Navigate to `/browse`, count `<article>` elements (expect 20)
    - Verify "Next →" link exists pointing to `/browse?page=1`
    - _Requirements: 5.1, 5.2_

  - [x] 3.2 Implement `test_browse_pagination_page_1` (port 4457)
    - Seed 25 summaries, start server with seeded state
    - Navigate to `/browse?page=1`, count `<article>` elements (expect 5)
    - Verify "← Previous" link exists pointing to `/browse?page=0`
    - _Requirements: 5.3, 5.4_

  - [x] 3.3 Implement `test_browse_no_next_on_last_page` (port 4458)
    - Seed 25 summaries, start server with seeded state
    - Navigate to `/browse?page=1` (the last page)
    - Verify "Next →" link is NOT present
    - _Requirements: 5.5_

  - [x] 3.4 Implement `test_summary_markdown_rendering` (port 4459)
    - Seed 1 summary with known markdown (`**bold**`, `- list item`, `## Heading`)
    - Navigate to the generation partial endpoint or browse page
    - Verify HTML contains `<strong>`, `<ul>` or `<li>`, and `<h2>` elements
    - _Requirements: 6.1, 6.2, 6.3_

  - [x] 3.5 Implement `test_timestamp_links_rendered` (port 4460)
    - Seed 1 summary with `timestamps_done=true` and timestamped content via `seed_summary_with_timestamps()`
    - Navigate to the generation partial for that summary
    - Verify `<a>` elements are present with `href` containing `&t=` parameter
    - _Requirements: 7.1, 7.2_

- [x] 4. Implement search tests
  - [x] 4.1 Implement `test_search_returns_results` (port 4461)
    - Seed summaries with synthetic embedding blobs (random f32 vectors serialized to bytes)
    - Skip test if `GEMINI_API_KEY` is not set (search requires embedding the query)
    - Submit search query, verify `#search-results` contains result entries
    - _Requirements: 8.1, 8.2_

  - [x] 4.2 Implement `test_search_empty_results` (port 4462)
    - Use default empty database (no embeddings)
    - Skip test if `GEMINI_API_KEY` is not set
    - Submit a nonsensical search query
    - Verify `#search-results` renders without error and contains no result entries
    - _Requirements: 9.1, 9.2_

- [x] 5. Implement concurrency and resilience tests
  - [x] 5.1 Implement `test_concurrent_submissions` (ports 4463–4464)
    - Start one geckodriver on port 4463, connect two browser clients (or use two geckodrivers on 4463 and 4464)
    - Each client submits a different URL to the same test server
    - Verify each receives a `#generation` div with a distinct identifier
    - _Requirements: 10.1, 10.2_

  - [x] 5.2 Implement `test_server_restart_recovery` (port 4465)
    - Use `start_test_server_controllable()` to get shutdown handle
    - Submit a form, verify polling starts (generation div with `hx-trigger`)
    - Send shutdown signal, wait briefly
    - Start a new server on a fixed port (rebind the address)
    - Verify browser eventually receives a valid response ("Summary not found" since in-memory DB is lost)
    - _Requirements: 11.1, 11.2_

- [x] 6. Implement accessibility tests
  - [x] 6.1 Implement `test_aria_busy_during_generation` (port 4466)
    - Submit a form, wait for generation partial to appear
    - Verify `aria-busy="true"` is present on an element within `#generation`
    - For completion state: seed a completed summary, navigate to its generation partial
    - Verify `aria-busy="true"` is NOT present when summary is done
    - _Requirements: 12.1, 12.2_

  - [x] 6.2 Implement `test_form_input_labels` (port 4467)
    - Navigate to index page
    - Verify `<label for="url">` exists
    - Verify `<label for="model">` exists
    - Verify search input has accessible name (check for `placeholder`, `aria-label`, or associated `<label>`)
    - _Requirements: 13.1, 13.2, 13.3_

  - [x] 6.3 Implement `test_keyboard_navigation` (port 4468)
    - Navigate to index page
    - Send Tab key presses and verify focus moves through URL input → model select → submit button
    - Fill in URL, press Enter, verify form submits via HTMX (check `#result` gets content)
    - _Requirements: 14.1, 14.2_

- [x] 7. Checkpoint - Verify all tests compile and run
  - Ensure `cargo test --test integration_browser -- --ignored` compiles without errors
  - Run a subset of tests that don't require geckodriver to verify compilation: `cargo build --tests`
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- All tests are `#[tokio::test]` and `#[ignore]` (require geckodriver + Firefox)
- Port range 4452–4468 is used for new tests to avoid conflicts with existing tests (4444–4451)
- Tests requiring `GEMINI_API_KEY` (search tests) should gracefully skip if the key is not set
- The server restart test uses `tokio::sync::watch` for graceful shutdown signaling
- No property-based tests — browser integration tests are scenario-based by nature
