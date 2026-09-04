# Gemini 3 Thinking Levels: Implementation Walkthrough

## Delivered behavior

Users can select `auto`, `minimal`, `low`, `medium`, or `high` thinking effort
in the submission UI. Named values are available only for `gemini-3.*`; the
browser resets unsupported models to `auto` and the route rejects forged
unsupported model/value pairs before they can be stored.

`ThinkingPreference` is a typed, lowercase value in `SubmitForm` and `Summary`.
Migration `006_add_thinking_level.sql` adds a non-null checked SQLite column
with the backward-compatible `high` default. This matches the prior hard-coded
Gemini 3 behavior for existing rows and omitted form fields. It is separate
from the existing `thinking` and `thinking_tokens` response fields.

The background task reads the saved row and forwards that value on every normal
and fallback generation attempt. Gemini 3 named choices map one-to-one to
`gemini_rust::ThinkingLevel`; `auto` omits the provider level. Gemini 2.5 keeps
its numeric budget path, so the two mutually exclusive SDK builder methods are
never combined. A fallback to an unsupported model logs a controlled downgrade
and omits the named level while retaining the original persisted choice.

## Commits

1. `239cca7 feat(thinking): persist validated Gemini 3 thinking preferences`
2. `0f90a22 feat(ui): expose Gemini 3.* thinking levels`
3. `b3eb3c1 feat(gemini): apply persisted Gemini 3 thinking levels`
4. `14aa83a ci(release): verify thinking-level migration is packaged`
5. `29cb1ec chore: satisfy strict workspace lint gate`
6. This document: `docs(thinking): add implementation walkthrough`

## Verification actually run

- `cargo fmt --check` — passed.
- `cargo test models::` — 2 passed.
- `cargo test db::` — 8 passed, including fresh migration, legacy schema,
  default/constraint, and every-value persistence checks.
- `cargo test routes::` — 7 passed, including template, invalid preference,
  and multi-URL fan-out checks.
- `cargo test services::summary::tests::thinking` — 1 passed.
- `cargo test tasks::` — 13 passed.
- `cargo test` — 140 unit tests and 4 rating integration tests passed; 41
  externally dependent tests remained ignored.
- `cargo clippy --all-targets --all-features -- -D warnings` — passed. The
  vendored `fast-umap` dependency still emits seven future-incompatible
  `float_literal_f32_fallback` warnings, but they are dependency warnings and
  do not fail this gate.
- `cargo build --release` — passed.

The Firefox attempt could not start because `geckodriver` is not installed.
The harness subsequently gained Chromium support and was run with the
container's headless Chrome for Testing plus matching ChromeDriver: 16 of 25
browser tests passed. The remaining nine are existing assertion/fixture issues
(including an obsolete title expectation, an invalid YouTube ID, and keyboard
tab order changed by the new selector), not WebDriver startup failures. No live
Gemini request was made: it requires credentials and incurs cost, while the
provider mapping and fallback behavior are covered without a network call. The
full suite still leaves 13 live pipeline/transcript tests and three live
transcript tests ignored for their external-service requirements.

## Migration and release proof

Migration order is `001_initial.sql` through
`006_add_thinking_level.sql`; no prior migration was edited. The release
workflow preserves its generic `migrations/` copy, then asserts both
`staging/migrations/006_add_thinking_level.sql` and its presence after tar
creation. A local release build/package reproduction ran:

```text
tar tzf rs-summarizer-linux-amd64.tar.gz | grep -Fx 'migrations/006_add_thinking_level.sql'
migrations/006_add_thinking_level.sql
```

The generated archive and staging directory were removed afterward and were not
committed.

## Learnings and follow-ups

- Persisting a provider-neutral enum makes queued work reproducible through
  restarts and future SDK naming changes.
- Model capabilities are currently inferred from the model name. A future
  `ModelOption` capability field would make UI and route policy data-driven.
- An administrator-configurable default and browse-page display of selected
  effort/actual thought-token usage would improve observability.
- A Gemini 2.5 numeric-budget UI should be designed separately once supported
  bounds are explicitly approved.

## Container additions

None were installed permanently. Rust/Cargo, SQLite, `tar`, and the supplied
headless Chrome were sufficient; matching ChromeDriver was downloaded only to
`/tmp` for the Chromium browser-test run.

## Chromium integration-test repairs

The initially recorded Chromium run exposed nine stale browser-test
assertions/fixtures rather than an application or WebDriver failure. The
browser suite now follows the current UI and response contracts:

- It expects the YouTube & Hacker News heading, the identifier-scoped
  `generation-<id>` containers, and the Thinking effort selector in keyboard
  tab order.
- It reads a generation identifier from its container, which remains available
  after a completed duplicate correctly drops `hx-post` polling.
- It uses syntactically valid YouTube IDs and zero-padded timestamp fixtures
  accepted by the current validators/linker.
- Its rate-limit case preloads the counter, making the browser assertion
  deterministic rather than dependent on an asynchronous background task.

Verification after these repairs:

```text
TEST_BROWSER=chromium CHROMEDRIVER=<matching-driver> CHROME_BINARY=<chrome> \
  cargo test --test integration_browser -- --ignored --test-threads=4

25 passed; 0 failed
```
