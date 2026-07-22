# Subagent Implementation Prompts: Phase 3 Strategic Tasks

This document contains self-contained, numbered prompts formatted for launching subagents to implement the remaining strategic tasks. Each prompt references the relevant sections in [01_review_report.md](file:///workspace/src/rs-summarizer/plan/20260722_02_code_review/01_review_report.md) and [02_implementation_plan.md](file:///workspace/src/rs-summarizer/plan/20260722_02_code_review/02_implementation_plan.md), along with exact file locations, line ranges, and struct/function signatures in the codebase.

---

## Prompt 1: Decompose `process_summary_inner` Monolith (Fix #1)

```text
Role: Rust Refactoring Subagent
Task: Decompose the process_summary_inner monolith in src/tasks.rs into modular sub-functions.

References:
- Review Report Finding #1: plan/20260722_02_code_review/01_review_report.md#L129
- Implementation Plan Section 2 (Fix #1): plan/20260722_02_code_review/02_implementation_plan.md#L28 and L103-L106

Source Code Context:
- Primary File: src/tasks.rs
- Primary Function: `process_summary_inner(db: &SqlitePool, id: i64, app: &AppState) -> Result<(), ProcessError>`
- Associated Types: `AppState` (src/state.rs), `ProcessError` (src/errors.rs), `HackerNewsService` (src/services/hacker_news.rs), `TranscriptService` (src/services/transcript.rs)

Context & Requirements:
As described in `01_review_report.md` (Issue #1), `process_summary_inner` in `src/tasks.rs` is a ~460-line monolithic function that mixes URL parsing, YouTube transcript fetching, Hacker News story/comment downloading, pasted text validation, model pipeline execution with fallbacks/retries, and database finalization.

Instructions:
1. Open and inspect `src/tasks.rs`.
2. Extract the following modular helper functions:
   - `fetch_youtube_content(url: &str) -> Result<String, ProcessError>`: Handles YouTube transcript extraction via `TranscriptService`.
   - `fetch_hn_content(hn_id: u64, user_pasted: Option<&str>, hn_svc: &HackerNewsService) -> Result<String, ProcessError>`: Handles HN submission, comments, and linked article downloading.
   - `process_pasted_transcript(raw_text: &str) -> Result<String, ProcessError>`: Handles text validation and word count bounds (30 to 280,000 words).
   - `run_model_pipeline(app: &AppState, input_text: &str, initial_model: &ModelOption) -> Result<SummaryOutput, ProcessError>`: Encapsulates streaming model execution, error checking, rate limit retry loops, and model fallback chain (`gemini-3.6-flash → gemini-3.5-flash-lite → gemma-4-31b-it`).
   - `finalize_and_embed(app: &AppState, identifier: i64, summary: &SummaryOutput) -> Result<(), ProcessError>`: Handles saving final summary to SQLite (`mark_summary_done`) and generating vector embeddings asynchronously.
3. Refactor `process_summary_inner` to orchestrate these modular functions cleanly.
4. Ensure no changes break existing error-handling behavior (`ProcessError` conversions) or test coverage.

Verification:
- Run `cargo test` to verify all unit and integration tests (`tests/integration_pipeline.rs`, `tests/integration_browser.rs`) pass.
- Run `cargo clippy` to ensure clean compilation.
```

---

## Prompt 2: Externalize Model Configurations to JSON (Fix #5)

```text
Role: Rust Configuration Subagent
Task: Externalize Gemini/Gemma model configurations to a JSON file with fallback logic.

References:
- Review Report Finding #5: plan/20260722_02_code_review/01_review_report.md#L138
- Implementation Plan Section 2 (Fix #5): plan/20260722_02_code_review/02_implementation_plan.md#L107-L109

Source Code Context:
- Primary Files: src/state.rs, src/main.rs
- Structs & Functions: `ModelOption` (src/state.rs#L25-L33), `get_default_models()` (src/state.rs#L64-L167), `main()` (src/main.rs#L38)
- Target Config File: config/models.json (new file)

Context & Requirements:
As noted in `01_review_report.md` (Issue #5), `get_default_models()` in `src/state.rs` hardcodes model definitions, pricing (`input_price_per_mtoken`, `output_price_per_mtoken`), context window size, RPM limits, and RPD limits. Updating model parameters currently requires recompiling the Rust binary.

Instructions:
1. Create `config/models.json` in the root workspace directory containing the JSON array of current `ModelOption` definitions matching `get_default_models()`.
2. In `src/state.rs`:
   - Implement `load_models_config(config_path: Option<&std::path::Path>) -> Vec<ModelOption>`.
   - Read from `MODELS_CONFIG_PATH` env var if set, otherwise try `config/models.json`.
   - Deserialize using `serde_json::from_str`.
   - Log an informative message on success (`tracing::info!`), or log a warning (`tracing::warn!`) and fall back to hardcoded `get_default_models()` if the file is missing or contains invalid JSON.
3. Update `src/main.rs` (around L38) to call `load_models_config(None)` when populating `AppState`.
4. Add unit tests `test_load_models_config()` and `test_load_models_config_fallback()` in `src/state.rs` validating JSON loading and fallback behavior.

Verification:
- Run `cargo test` to verify all tests pass (including `model_checks` suite in `state.rs`).
- Run `cargo clippy`.
```

---

## Prompt 3: Safety Audit of `unsafe impl Send/Sync` for `NnMapper` (Fix #8)

```text
Role: Rust Concurrency & Safety Subagent
Task: Audit and document or replace unsafe impl Send/Sync for NnMapper in src/services/nn_mapper.rs.

References:
- Review Report Finding #8: plan/20260722_02_code_review/01_review_report.md#L141
- Implementation Plan Section 2 (Fix #8): plan/20260722_02_code_review/02_implementation_plan.md#L111-L113

Source Code Context:
- Primary File: src/services/nn_mapper.rs
- Submodule Crate: third_party/fast-umap (path patch in Cargo.toml#L5)
- Struct & Traits: `NnMapper`, `unsafe impl Send for NnMapper`, `unsafe impl Sync for NnMapper`

Context & Requirements:
As reported in `01_review_report.md` (Issue #8), `src/services/nn_mapper.rs` uses `unsafe impl Send for NnMapper` and `unsafe impl Sync for NnMapper` to allow GPU-accelerated `FittedUmap` projection instances to be shared across Tokio worker threads via `AppState`. Rust safety guidelines require explicit safety rationale for `unsafe impl` blocks.

Instructions:
1. Inspect `src/services/nn_mapper.rs` and `third_party/fast-umap`.
2. Inspect `FittedUmap` usage during runtime projections (`NnMapper::project()`). Verify whether `FittedUmap` is read-only after initialization.
3. If `FittedUmap` is thread-safe for read-only concurrent access:
   - Retain `unsafe impl Send` and `unsafe impl Sync` and add a detailed `// SAFETY:` doc comment explaining why `FittedUmap` does not contain unsynchronized interior mutability (e.g. no `Cell`, `RefCell`, or unsynchronized raw pointer mutations during read ops).
4. If `FittedUmap` contains interior mutability or unsafe non-atomic state:
   - Wrap the internal `FittedUmap` field inside `Arc<std::sync::Mutex<FittedUmap>>` or `RwLock`.
   - Remove the `unsafe impl Send / Sync` blocks.
5. Ensure all existing tests in `src/services/nn_mapper.rs` pass cleanly.

Verification:
- Run `cargo test` to verify.
- Run `cargo clippy`.
```
