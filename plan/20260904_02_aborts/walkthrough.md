# Abort-safe Gemini generation walkthrough

## Delivered behavior

- `migrations/007_add_generation_lifecycle.sql` adds an additive lifecycle to
  `summaries`: status, attempt, epoch, timestamps, retry time, safe public
  error fields, and interaction ID, plus a recovery index. Existing completed
  rows are backfilled to `succeeded`; unfinished rows are `queued`.
- `src/generation.rs` owns the public state/error vocabulary, legal
  transitions, finite retry policy, and content-type completeness policy.
- `src/services/summary.rs` now uses `gemini-rust` 2.0.0 Interactions streaming
  rather than deprecated `generate_content`. A response is successful only
  after `interaction.completed` has `completed` status and the required
  headings are present. EOF, provider failure, or truncated text are rejected.
- `src/db.rs` uses CAS status changes and epoch-bound stream appends. A retry
  deliberately starts a new epoch and clears the prior display body, so a late
  event cannot append to a new attempt.
- `src/tasks.rs` persists retry-wait before a bounded delay, restores `running`
  before retrying, and retains incomplete chunks as `partial_failed`. A failed
  child in a multi-source submission makes the aggregate non-successful.
- Startup recovers stale `running` jobs to `queued` and claims only rows
  explicitly marked as interrupted. `008_quarantine_legacy_queued_generations`
  prevents a historical queued backlog from being replayed at startup; users
  can explicitly,
  idempotently retry terminal failures through
  `POST /generations/{identifier}/retry`.
- `generation_partial.html` polls only queued/running/retry-wait generations.
  Terminal errors render an accessible alert and retry action; partial drafts
  are explicitly marked incomplete.

## Verification

Executed successfully on 2026-09-04:

```text
cargo search gemini-rust --limit 5
cargo info gemini-rust
cargo tree -i gemini-rust
cargo fmt -- --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
git diff --check
```

The all-feature lint/test commands pass. Their only output is pre-existing
future-incompatibility warnings in the vendored `third_party/fast-umap`; no
project lint warnings are emitted. Deterministic tests cover state rules,
output rejection, migration/backfill, CAS/epoch ownership, retry reset, and
the terminal UI polling/retry contract.

`GEMINI_LIVE_TEST` was not set, so no API key was loaded and no billable live
request was made. The WebDriver suite passed serially with
`TEST_BROWSER=chromium`, the matching ChromeDriver path documented below, and
`--ignored --test-threads=1`; serial execution avoids its fixed-port
collisions. The `sqlx` CLI is not installed in this container;
migration behavior is instead exercised directly against empty and pre-007
SQLite fixtures by the database tests.

## Rollout and rollback

### 1.7.2 corrective rollout

The 1.7.0 startup path inadvertently spawned a worker for every historical
unfinished row backfilled to `queued`. On an existing database, that can issue
many concurrent Gemini calls, quickly exhaust quota, and contend for SQLite's
small connection pool. Version 1.7.1 adds migration 008. It preserves partial
drafts but marks untouched legacy queued rows as terminal `failed` with a safe
retry message; it resumes only rows explicitly marked `network_interrupted` by
crash/retry recovery.

1.7.2 also maps the persisted `minimal` thinking preference to Gemini Flash's
lowest currently accepted Interactions level, `low`. Gemini returned a 400 for
`minimal` on Gemini 3.8 (`low`, `medium`, and `high` are accepted). Typed
provider stream-error events now enter the same bounded retry policy as
temporary connection interruption.

Deploy migration 008 before application binaries. It is additive and retains
`summary_done` for browse/export compatibility. Monitor `partial_failed`,
`retry_wait`, provider terminal status, and the public error code fields. On
rollback, deploy the previous binary: it ignores the new columns. Do not drop
the new columns until all deployed binaries no longer rely on them.

## Learnings and follow-ups

Interactions supplies a meaningful lifecycle terminal event, unlike treating
stream EOF as implicit success. A deterministic provider-event accumulator
seam now covers text/usage/completed, failed terminal, and error events. The
WebDriver abort/retry scenario remains to complete browser coverage. On
restart, retry-wait work is recovered to queued work so that a
crash cannot strand a non-terminal row; this favors recovery over preserving a
stale in-memory delay.
Thinking summaries are captured when Interactions emits typed thought-summary
content; Gemini 2.5 retains its provider default because this API exposes named
thinking levels for Gemini 3.

## Container programs added

ChromeDriver 152.0.7977.75 was installed under
`/opt/archify-browser/chromedriver` to match the existing Chrome-for-Testing
binary. Run browser checks with `TEST_BROWSER=chromium` and
`CHROMEDRIVER=/opt/archify-browser/chromedriver/chromedriver-linux64/chromedriver`.
