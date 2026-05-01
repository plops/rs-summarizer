---
name: polling-mechanism
description: Use when modifying HTMX polling behavior, generation status responses, the hx-trigger stop pattern, or debugging infinite spinner issues.
inclusion: manual
---

# HTMX Polling Mechanism

## Overview

rs-summarizer uses HTMX polling to provide real-time progressive updates during summary generation. The frontend polls the backend every second, and the backend signals completion by omitting HTMX attributes from the response.

## How It Works

### 1. Submission (POST /process_transcript)

When a user submits a YouTube URL:
1. A new row is inserted into SQLite with `summary_done = false`
2. A background task is spawned via `tokio::spawn`
3. The route returns an HTML partial with HTMX polling attributes

### 2. Polling Template (generation_partial.html)

The template conditionally includes HTMX attributes based on `summary_done`:

```html
<div id="generation"
     {% if !summary_done %}hx-post="/generations/{{ identifier }}" hx-trigger="every 1s" hx-swap="outerHTML"{% endif %}>
    <article>
        {% if summary_done %}
        <header>Summary Complete</header>
        {% else %}
        <header>Generating summary... <span aria-busy="true"></span></header>
        {% endif %}
        <div>{{ summary|safe }}</div>
        {% if summary_done && !timestamps.is_empty() %}
        <footer>{{ timestamps|safe }}</footer>
        {% endif %}
    </article>
</div>
```

Key behavior:
- When `summary_done = false`: the div has `hx-post`, `hx-trigger="every 1s"`, and `hx-swap="outerHTML"` — HTMX replaces the entire div every second
- When `summary_done = true`: no HTMX attributes are rendered — polling stops automatically
- The spinner (`<span aria-busy="true">`) only shows when `summary_done = false`

### 3. Polling Endpoint (POST /generations/{identifier})

Each poll hits `get_generation()` which:
1. Fetches the current row from SQLite via `db::fetch_summary()`
2. Renders the `GenerationPartialTemplate` with current `summary_done` state
3. Returns the HTML partial — HTMX replaces the old div with this new one

### 4. Background Task Lifecycle (tasks.rs)

The `process_summary()` function orchestrates the pipeline:
1. Wait for row to exist (retry with backoff)
2. Download transcript via yt-dlp
3. Validate transcript length (30–280,000 words)
4. Generate summary via Gemini (streaming, chunks appended to DB progressively)
5. **Mark summary done** — calls `db::mark_summary_done()` setting `summary_done = true`
6. Convert to YouTube format, set `timestamps_done = true`
7. Compute embedding (non-fatal if it fails)

On error: `mark_error()` stores the error message in the summary field AND sets `summary_done = true`, ensuring the frontend always stops polling.

### 5. State Transitions

```
[Insert row]  →  summary_done=false, summary=""
                     ↓
[Streaming]   →  summary_done=false, summary grows chunk by chunk
                     ↓
[Complete]    →  summary_done=true, tokens/cost/timestamp_end recorded
                     ↓
[Timestamps]  →  timestamps_done=true, youtube_format populated
                     ↓
[Embedding]   →  embedding blob stored (non-fatal)
```

On error at any step:
```
[Error]       →  summary_done=true, summary contains error message
```

### 6. Critical Invariant

**`summary_done` must always eventually become `true`** — whether the pipeline succeeds or fails. This is what stops the HTMX polling loop. If `summary_done` never becomes true, the frontend polls forever (the "infinite spinner" bug).

## Database Fields Involved

| Field | Type | Role |
|-------|------|------|
| `summary_done` | BOOLEAN | Controls HTMX polling (false = keep polling, true = stop) |
| `summary` | TEXT | Grows during streaming, contains final text or error message |
| `summary_timestamp_end` | TEXT | ISO 8601 timestamp when summary generation completed |
| `summary_input_tokens` | INTEGER | Token count from Gemini usage metadata |
| `summary_output_tokens` | INTEGER | Token count from Gemini usage metadata |
| `timestamps_done` | BOOLEAN | Set after YouTube format conversion |
| `timestamped_summary_in_youtube_format` | TEXT | YouTube-compatible text (no ** markers) |

## Relevant Files

- `templates/generation_partial.html` — HTMX polling template
- `src/routes/mod.rs` — `get_generation()` polling endpoint, `render_generation_partial()` helper
- `src/tasks.rs` — `process_summary()` background task, `mark_error()` error handler
- `src/db.rs` — `mark_summary_done()`, `update_summary_chunk()`, `mark_timestamps_done()`
- `src/services/summary.rs` — Streaming chunk generation via Gemini

## Browser Tests Covering Polling

The following browser integration tests verify polling behavior end-to-end:

- `test_form_submission_shows_processing` — Verifies `#generation` div appears with `hx-post` after form submission
- `test_polling_stops_on_error` — Verifies no `hx-trigger` attribute when error occurs (polling doesn't start)
- `test_deduplication_returns_same_id` — Verifies duplicate submissions return existing generation partial
- `test_aria_busy_during_generation` — Verifies `aria-busy="true"` present during generation, absent when done
- `test_server_restart_recovery` — Verifies browser recovers when server restarts mid-poll
- `test_full_summarization_e2e` — Verifies polling terminates and `hx-trigger` is removed after completion

Run with: `cargo test --test integration_browser -- --ignored`
