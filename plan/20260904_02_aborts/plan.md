# Implementation plan: reliable Gemini abort handling

**Client:** Wol Pumba (`wolpumba@gmail.com`)  
**Scope:** prevent truncated Gemini 3.7/3.8 summaries from being published as
successful; make every terminal failure visible in the web UI; retire the
deprecated Gemini generation API without losing existing features.

## Findings and target outcome

The reported `#18505` is a separate, reproducible class of failure from the
logged 503s: it completed after 49 output tokens and was logged as successful.
`src/services/summary.rs` currently treats EOF plus non-empty output as
success, does not inspect the candidate finish reason, and does not validate
the required Abstract/section structure.  A short, provider-aborted response
can therefore pass.  The 503 path waits 10 minutes then four hours in a Tokio
task; while it waits, the sole `summary_done` boolean keeps HTMX polling with a
generic spinner.  The current `mark_error` does end polling, but overwrites
partial output and exposes only unstructured provider text.

The required end state is a durable, observable generation state machine:

```text
queued -> running -> retry_wait -> running -> succeeded
                 \-> failed                 \-> partial_failed
```

Every branch is terminal or has a persisted next retry time.  The polling
response renders a meaningful state, stops polling for either terminal state,
and never labels an incomplete response `succeeded`.  Preserve partial text
for diagnosis/retry; do not make it look like a complete summary.

## Requirements and recommended additions

| Requirement | Implementation decision / acceptance evidence |
|---|---|
| Gemini 3.7/3.8 output may stop prematurely | Require an explicit successful provider terminal event/reason plus a product output-completeness check before `succeeded`; classify all other endings as retryable or `partial_failed`. |
| 503 high demand | Persist retry count, attempt, model, error class, and `next_retry_at`; bounded exponential backoff with jitter and a clear UI countdown/message. Exhaustion is terminal failed, never an endless spinner. |
| Errors reach the user | Add a structured public error code/message and display it in the generation partial. Keep diagnostic detail in logs/database, not raw secrets/provider payloads in HTML. |
| Deprecated Gemini API | Migrate Gemini summarization to `gemini-rust` 2.0.0 Interactions streaming. Retain system instruction, Gemini-3 thinking preference, Google Search, URL Context, streaming chunks, token/cost accounting, and the Hetzner path. |
| Existing data/API compatibility | Use an additive SQLite migration and backfill legacy rows deterministically (`summary_done=1` => `succeeded` unless they carry the historical error marker, otherwise `queued`/`running` policy documented and tested). Keep `summary_done` synchronized during rollout for current browse/export code, then remove only in a later deliberate migration. |
| Safe retries | Never append a second attempt to a prior partial body. Store an attempt number / generation epoch; clear or archive the display body only at a deliberate retry boundary, and make chunk writes conditional on that epoch. |
| Operational safety (recommended) | Persist timestamps/heartbeats and add a startup recovery pass for stale `running` rows. Add a retry endpoint with an explicit user action, and make it idempotent while a task is already active. |
| Product quality (recommended) | Make the completion policy configurable per content type: HN needs `Abstract`, `Key Points`, `Discussion Highlights`; transcripts need `Abstract` and `Key Highlights & Timestamps`. Log rejection reason and observed output/usage metrics. |

Out of scope unless separately approved: silently switching providers after a
non-rate-limit partial response, automatically spending a new Gemini request
after a terminal failure, or exposing thinking text / raw API errors to users.

## Design

1. Add an additive migration (next sequence after `006`) with `generation_status`
   constrained by application validation, `generation_attempt`, `generation_epoch`,
   `generation_started_at`, `generation_updated_at`, `next_retry_at`,
   `generation_error_code`, `generation_error_message`, and optional
   `provider_interaction_id`.  Add an index supporting stale/retry recovery.
2. Introduce a small domain module (for example `src/generation.rs`) containing
   the status enum, public error-code enum, transition validation, retry policy,
   completion validator, and provider-neutral `GenerationOutcome`.  Database
   methods perform compare-and-set updates using identifier + epoch/status.
3. Replace the `#[allow(deprecated)]` Gemini `generate_content` path with the
   Interactions stream.  Process `StepDelta::Text` as chunks, final usage from
   `StepStop`, thoughts from typed thought steps, and completion only after
   `InteractionCompleted` reports success.  Treat stream EOF without that event,
   stream errors, failed/cancelled status, safety refusal, and malformed/too-short
   product output as named outcomes.  Verify the actual Interactions API supports
   every selected Gemini 3.7/3.8 feature against the supplied key in a narrowly
   scoped, non-production test; fall back to a documented compatibility adapter
   only if a provider capability is absent.
4. Refactor `run_model_pipeline` so it records each candidate attempt and uses
   a finite retry budget.  503/UNAVAILABLE and network interruption are
   retryable; 429 retains fallback-chain behavior; invalid requests, policy
   refusals, and exhausted retries are terminal.  A retry reuses source text but
   starts a fresh epoch and cannot concatenate output from attempts.
5. Make `process_summary` set `running` first, use an RAII/finally-style
   supervisor to guarantee a terminal or scheduled-retry state, and run stale
   recovery at startup.  Keep multi-URL semantics explicit: per-item outcomes
   are rendered as distinct cards and the parent reaches `partial_failed` only
   after all items have terminal outcomes.
6. Extend `Summary`, `db.rs`, route view models, and
   `generation_partial.html`.  Running/retry displays a stage and next retry;
   terminal failure/partial failure displays an accessible error panel,
   preserved partial draft marked incomplete, and a retry control. HTMX polls
   only `queued`, `running`, or `retry_wait`; it stops for success and failures.
7. Add structured tracing/metrics-friendly fields (`identifier`, model,
   attempt, epoch, status transition, provider status/finish reason, error code,
   retry delay, output length). Redact API keys and raw provider response bodies.

## Autonomous-agent context map

| File | Why read it |
|---|---|
| `plan/20260904_02_aborts/prompt.txt` | Original report, logs, constraints, and requested deliverables. |
| `plan/20260904_02_aborts/deps.md` | Dependency ownership, researched API decision, and compile-target usage sketch. |
| `Cargo.toml`, `Cargo.lock` | Current `gemini-rust` 2.0.0 pin and workspace dependency constraints. |
| `src/services/summary.rs` | Deprecated Gemini stream, chunk persistence, prompt/tool/thinking configuration, token accounting, and Hetzner parallel path. |
| `src/tasks.rs` | Supervisor, retries/fallbacks, multi-URL behavior, finalization, and current error overwrite. |
| `src/errors.rs` | Existing error vocabulary to replace/extend with classifiable failures. |
| `src/db.rs`, `migrations/001_initial.sql`, `migrations/002..006` | SQLite schema evolution and persistence helpers. |
| `src/models.rs` | SQLx `Summary` row mapping and submit forms. |
| `src/routes/mod.rs` | Submission, poll route, test state, and generation partial view-model construction. |
| `templates/generation_partial.html`, `templates/index.html` | Current HTMX one-second polling contract and accessible UI surface. |
| `tests/integration_pipeline.rs`, `tests/integration_browser.rs` | Existing task/database and real-browser testing patterns. |
| `src/main.rs`, `src/state.rs` | Startup ordering and app-state construction; required for stale-job recovery. |
| `/root/.cargo/registry/src/.../gemini-rust-2.0.0/README.md` and `examples/interaction_*.rs` | Primary installed dependency source; do not rely on stale DeepWiki claims over it. |

## Tests and verification

Write deterministic fakes around a provider-stream trait; tests must not use
the real API key or make billable network calls.  Cover state transition rules,
retry classification/backoff (with paused Tokio time), epoch-protected chunk
writes, migration/backfill, provider event sequences, all terminal conditions,
output validation, fallback ordering, recovery of stale jobs, route HTML/poll
attributes, retry idempotency, and a browser flow proving that an error card
replaces the spinner.  Retain real provider smoke testing as opt-in
`GEMINI_LIVE_TEST=1`, source the key only from `/workspace/src/.env`, redact it,
and use a minimal request with a hard budget/timeout.

Run: `cargo fmt -- --check`; `cargo check --all-targets`; `cargo clippy
--all-targets --all-features -- -D warnings`; `cargo test --all-targets
--all-features`; the focused browser test with its documented WebDriver; then
the service’s release checks if applicable.  Also inspect `git diff --check`,
run `sqlx migrate run` against a disposable file database, and manually verify
the three rendered states in a browser.

## Commit policy

Use one logical concern per Conventional Commit.  Title format is
`<type>(<scope>): <imperative summary>` (72 characters or fewer); every commit
has a detailed body explaining behavior, rationale, schema compatibility,
failure/retry consequences, and exact tests run.  Suggested sequence:

```text
feat(generation): persist observable generation lifecycle
refactor(gemini): migrate summary streaming to interactions
fix(generation): reject incomplete provider responses
feat(ui): render retry and terminal generation states
test(generation): cover abort recovery and polling termination
docs(generation): document abort-handling implementation
```

Example body:

```gitcommit
fix(generation): reject incomplete provider responses

Require a successful provider terminal event and validate the requested
summary sections before a generation can transition to succeeded. Persist
partial provider output as partial_failed instead of publishing it as complete.

Tests: cargo test --all-targets --all-features; cargo clippy --all-targets \
--all-features -- -D warnings
```

After implementation, commit the walkthrough as `docs(generation): document
abort-handling rollout`; it must state what was actually built, deviations from
this plan, test evidence, operational learnings, extensions, and new container
programs (or explicitly state none were added).
