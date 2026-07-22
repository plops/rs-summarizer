# Walkthrough: Code-Review Fixes (Phase 1 & Phase 2)

All planned code review findings (Phase 1 Quick Wins & Phase 2 Targeted Improvements) have been implemented and verified.

## 1. Summary of Implemented Fixes

| Issue # | Description | Target File(s) | Summary of Changes |
|:---|:---|:---|:---|
| **#2** | Remove `assert!` panic in `cosine_similarity` | `src/services/embedding.rs` | Replaced `assert!` with an `if a.is_empty() || b.is_empty() { return 0.0; }` guard. Updated unit tests to verify `0.0` is returned without panicking. |
| **#3** | Fix blocking I/O in async function `load_viz_data` | `src/main.rs` | Replaced `std::fs::read_to_string` with `tokio::fs::read_to_string` for loading cluster titles. |
| **#4** | Cache compiled Regex instances | `src/services/hacker_news.rs`, `src/utils/url_validator.rs`, `src/utils/markdown_converter.rs`, `src/utils/timestamp_linker.rs` | Converted all inline `Regex::new(...)` pattern compilations to static `OnceLock<Regex>` / `OnceLock<Vec<Regex>>` instances. |
| **#6** | Configurable host & port via environment variables | `src/main.rs` | Used `HOST` (default `127.0.0.1`) and `PORT` (default `5001`) env variables instead of hardcoded `"127.0.0.1:5001"`. |
| **#10** | Standardize error & log messages to English | `src/routes/mod.rs`, `src/errors.rs`, `src/main.rs` | Replaced German log/error strings in `NnMapperError`, `main.rs` tracing calls, and `process_transcript` error response with English phrasing. |
| **#11** | Log template rendering errors | `src/routes/mod.rs` | Replaced `template.render().unwrap_or_default()` with a helper `render_template` that logs errors via `tracing::error!`. |
| **#12** | Tighten timestamp Regex matching | `src/utils/timestamp_linker.rs` | Added negative lookbehind/lookahead `(?<![\w\-\.\/])...(?![\w\-\.\/])` to avoid false matches on CSS properties or ratio formats like `16:9` or `16:09px`. |
| **#13** | Enable multiline flag for heading Regex | `src/utils/markdown_converter.rs` | Updated heading Regex to `(?m)^##\s*(.*)` and used `replace_all` to convert headings across all lines. Updated unit tests accordingly. |
| **#14** | Store `DeduplicationService` in `AppState` | `src/state.rs`, `src/services/deduplication.rs`, `src/routes/mod.rs`, `src/main.rs`, test files | Added `#[derive(Clone, Debug)]` to `DeduplicationService`, added `dedup_service` field to `AppState`, and removed per-request instantiation in routes. |
| **#15** | Parameterize page size in `fetch_browse_page` | `src/db.rs`, `src/routes/mod.rs` | Added `page_size: u32` parameter to `fetch_browse_page` and bound it to SQL `LIMIT ? OFFSET ?`. |

---

## 2. Intentionally Deferred Fixes (Phase 3 Strategic Tasks)

The following items are strategic refactorings explicitly scheduled for separate feature tasks as outlined in `02_implementation_plan.md`:
- **Fix #1** (`src/tasks.rs`): Decomposing `process_summary_inner` monolith into sub-functions (`fetch_youtube_content`, `fetch_hn_content`, `run_model_pipeline`, etc.).
- **Fix #5** (`src/state.rs`): Externalizing model configuration to JSON/YAML config file.
- **Fix #8** (`src/services/nn_mapper.rs`): Detailed safety audit / mutex wrapping of `FittedUmap` internals for `unsafe impl Send/Sync`.

---

## 3. Verification & Test Results

- **Unit Tests (`cargo test`)**: 101/101 unit tests passed successfully with 0 failures.
- **Compiler / Linting (`cargo clippy`)**: Passed without new warnings.
