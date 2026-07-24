# Implementation Plan - Multi-Submission Splitting, HN Model Heuristic, & Rate Limit Fallbacks

This document outlines the detailed architecture analysis, source files, step-by-step implementation strategy, and testing requirements for the three requested features in `rs-summarizer`.

---

## 1. Relevant Source Files & Descriptions

- **`src/routes/mod.rs`**: Handles Axum web routes. `process_transcript` receives multi-URL submissions. Must be updated to split URLs into separate database rows, spawn individual tasks, and return HTMX polling partials for all generated IDs.
- **`src/tasks.rs`**: Orchestrates the background summarization pipeline (`process_summary`, `run_model_pipeline`). Must be updated for:
  1. HN article model selection heuristic (`auto` model selection based on word count).
  2. Runtime 429 / Rate limit model fallback loop across the model chain.
- **`src/services/summary.rs`**: Contains `SummaryService` which calls Google Gemini streaming API and detects `SummaryError::RateLimited` when 429 or `RESOURCE_EXHAUSTED` occurs.
- **`src/templates.rs` & `templates/generation_partial.html`**: Defines Askama HTML templates. `generation_partial.html` will be updated to use unique DOM IDs (`id="generation-{{ identifier }}"`) for clean multi-card HTMX polling.
- **`tests/integration_pipeline.rs` & `tests/integration_browser.rs`**: System integration tests.

---

## 2. Technical Requirements & Architectural Analysis

### Requirements Summary & Enhancements
1. **Multi-Submission Splitting**:
   - Each submitted URL in a multi-link input creates an independent row in SQLite `summaries`.
   - Each row gets its own background task, rating, pricing calculation, vector embedding, and timestamp rendering.
   - For HTMX UI compatibility, `process_transcript` returns polling partials for all items so each card updates independently.
2. **Hacker News Model Selection Heuristic**:
   - For `auto` model selection on HN articles, calculate word count:
     - Context < 3,000 words: select `gemini-3.5-flash-lite`.
     - Context >= 3,000 words: select `gemini-3.6-flash`.
3. **Runtime Rate-Limit Fallback Strategy**:
   - When Gemini returns `HTTP 429` / `RESOURCE_EXHAUSTED` or rate limit errors during execution, `run_model_pipeline` must catch the error and attempt the next model in `get_fallback_chain`.
   - For each candidate model in the chain, enforce RPM limits (`enforce_rpm_limit`), check daily RPD, update DB model name, and retry summary generation.

---

## 3. Step-by-Step Implementation Steps

### Step 1: Update Web Handler for Multi-Submission Splitting (`src/routes/mod.rs` & `templates/generation_partial.html`)
- In `templates/generation_partial.html`, change `id="generation"` to `id="generation-{{ identifier }}"`.
- In `src/routes/mod.rs`:
  - Validate all URLs in `split_urls(&input.original_source_link)`. Return error if any URL is unknown.
  - If valid, loop through each URL:
    - Create a distinct `SubmitForm` per URL.
    - Run deduplication (`check_duplicate`).
    - Insert new summary row if not duplicate.
    - Spawn `tasks::process_summary`.
    - Render `GenerationPartialTemplate` for each ID and collect HTML.
  - Return joined HTML response.

### Step 2: Update HN Model Selection Heuristic (`src/tasks.rs`)
- In `run_model_pipeline`:
  - If `initial_model_name == "auto"` and `is_hn` is true:
    - Count words in `input_text`.
    - If `word_count < 3000` -> set `model_name = "gemini-3.5-flash-lite"`.
    - Else -> set `model_name = "gemini-3.6-flash"`.

### Step 3: Implement Runtime Rate Limit Fallback Loop (`src/tasks.rs`)
- Update `run_model_pipeline` to iterate over candidate models returned by `get_fallback_chain(&initial_model)` when `summary_svc.generate_summary` returns `SummaryError::RateLimited` or 429/quota error.
- Enforce RPM delays and RPD limits per fallback candidate.

### Step 4: Tests & Verification
- Unit tests in `src/tasks.rs` for:
  - `auto` model selection for HN short vs long context.
  - Multi-submission handler splitting logic.
  - Runtime fallback chain execution on 429 error.
- Integration tests in `tests/integration_pipeline.rs`.

---

## 4. Conventional Commit Standards

All commits must follow Conventional Commits format with comprehensive descriptions:
- `feat(routes): split multi-URL submissions into individual DB entries`
- `feat(tasks): implement HN word-count model heuristic and runtime API 429 model fallbacks`
- `test(tasks): add unit and integration tests for multi-submission, HN model selection, and fallbacks`

---

## 5. Walkthrough Output Requirement
Upon successful implementation and test execution, create `plan/20260724_02_split_multi/walkthrough.md` summarizing the changes, verification results, learnings, and potential future improvements.
