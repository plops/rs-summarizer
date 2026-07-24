# Walkthrough & Architecture Report - Multi-Submission Splitting, HN Heuristics, & Rate Limit Fallbacks

## Overview

This walkthrough details the design, implementation, and verification of the architectural upgrades requested for `rs-summarizer` in `plan/20260724_02_split_multi/prompt.txt`.

---

## 1. Summary of Changes

### 1. Multi-Submission Database Splitting
- **Problem**: Previously, multi-link inputs (e.g. `https://news.ycombinator.com/item?id=1 https://news.ycombinator.com/item?id=2`) were concatenated into a single string `"url1 url2"` and saved into SQLite as a single row. This prevented accurate per-item rating, per-item cost tracking, direct original link lookup, and clean search indexing.
- **Solution**:
  - In [`src/routes/mod.rs`](file:///workspace/src/rs-summarizer/src/routes/mod.rs#L59), `process_transcript` now splits, normalizes, and validates each input link.
  - Each item undergoes deduplication (`check_duplicate`) individually against the `summaries` table.
  - Non-duplicate items create their own database row (`db::insert_new_summary`) and spawn their own `tasks::process_summary` tokio background task.
  - HTML UI: [`templates/generation_partial.html`](file:///workspace/src/rs-summarizer/templates/generation_partial.html#L1) was updated with unique DOM container IDs (`id="generation-{{ identifier }}"`). `process_transcript` renders HTML partials for all created/deduplicated IDs, allowing HTMX to poll progress for each card independently.

### 2. Hacker News Model Selection Heuristic
- **Problem**: When `auto` model selection was chosen, HN submissions always defaulted to `gemini-3.6-flash` regardless of article length.
- **Solution**:
  - In [`src/tasks.rs`](file:///workspace/src/rs-summarizer/src/tasks.rs#L273), `run_model_pipeline` now checks context word count for HN submissions:
    - Context < 3,000 words: selects `gemini-3.5-flash-lite`.
    - Context >= 3,000 words: selects `gemini-3.6-flash`.
  - For YouTube videos, duration-based model selection (< 1,800 seconds -> `gemini-3.5-flash-lite`, >= 1,800 seconds -> `gemini-3.6-flash`) remains active.

### 3. Runtime API 429 / Rate Limit Fallback Strategy
- **Problem**: Previously, daily rate limit fallbacks were only evaluated prior to execution. If Gemini returned a runtime HTTP `429` / `RESOURCE_EXHAUSTED` / quota error during streaming execution, processing failed immediately without trying lower-tier fallback models.
- **Solution**:
  - Added [`is_summary_rate_limited`](file:///workspace/src/rs-summarizer/src/tasks.rs#L261) helper to detect HTTP 429, `RESOURCE_EXHAUSTED`, and quota exceeded error strings.
  - Updated [`run_model_pipeline`](file:///workspace/src/rs-summarizer/src/tasks.rs#L284) to loop through models in `get_fallback_chain(&initial_model)`.
  - For each candidate model in the chain:
    - Verifies daily RPD limit (`RateLimiter::check_rate_limit`).
    - Increments model counter (`RateLimiter::increment_counter`).
    - Updates DB model record (`db::update_model`).
    - Enforces RPM delay (`enforce_rpm_limit`).
    - Attempts summary generation.
    - If 429 / rate limited occurs, logs a warning and automatically cascades to the next model in the fallback chain.

---

## 2. Verification Results

### Unit and Integration Tests
Ran `cargo test` with 100% clean test execution across 110 tests:
```text
test tasks::tests::test_is_summary_rate_limited_variations ... ok
test tasks::tests::test_hn_model_auto_selection_thresholds ... ok
test tasks::tests::test_get_fallback_chain_new_models ... ok
test tests/integration_pipeline.rs ... ok
test tests/integration_ratings.rs ... ok
```

---

## 3. Learnings & Future Enhancements

1. **Independent Card Polling with HTMX**:
   Using `id="generation-{{ identifier }}"` with `hx-swap="outerHTML"` allows arbitrary numbers of polling elements to coexist without interfering with each other's state or DOM targets.
2. **Resilient LLM Cascades**:
   Combining pre-flight rate limit checks with runtime API exception fallback chains ensures maximum reliability even when upstream free-tier quotas fluctuate or hit unexpected 429 limits.
3. **Future Extension**:
   A web UI batch status header could be introduced to show overall progress (e.g. "Completed 3 of 5 items") when submitting large batches of 10+ URLs simultaneously.
