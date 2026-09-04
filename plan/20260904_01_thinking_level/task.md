# Serial Tasks: Gemini 3.7/3.8 Thinking Levels

Complete every task serially: implement, test, validate, then commit. Do not move ahead with a failing gate.

## 1. Domain, migration, and persistence

Read `src/models.rs`, `src/db.rs`, and all migrations. Add `ThinkingPreference` with `auto`, `minimal`, `low`, `medium`, `high` and a `high` default. Add it to `Summary`/`SubmitForm`; add migration `006_add_thinking_level.sql`; update insert/fetch. Add parser/default, fresh schema, legacy migration, constraint, and DB round-trip tests.

Validate: `cargo fmt --check` and `cargo test models:: db::`.

Commit: `feat(thinking): persist validated Gemini 3 thinking preferences`

Body must explain immutable migration, backward-compatible high defaults, and test evidence.

## 2. UI and server validation

Read `templates/index.html`, `src/routes/mod.rs`, `src/state.rs`, and existing route tests. Add an accessible selector/help text; enable named choices only for 3.*; explain unsupported models and force auto client-side. Validate model/preference pairs server-side, copy the choice to all multi-URL `single_input` values, and add template, invalid form, and fan-out tests.

Validate: `cargo fmt --check`; `cargo test routes::`; `cargo test --test integration_browser -- --ignored`. If Firefox/geckodriver is unavailable, record the skipped ignored test; route/template tests are mandatory.

Commit: `feat(ui): expose Gemini 3.* thinking levels`

## 3. Provider mapping and task propagation

Read `src/tasks.rs`, `src/services/summary.rs`, and `deps.md`. Build a pure 3.* mapping helper; replace hard-coded High with saved preference; omit level for auto; retain mutually exclusive Gemini-2.5 budget behavior; propagate the fetched Summary value through normal/fallback attempts; log controlled unsupported fallback downgrade. Add no-network mapping, auto, 2.5-separation, and fallback tests.

Validate: `cargo fmt --check`; `cargo test services::summary::tests::thinking`; `cargo test tasks::`; `cargo test`.

Commit: `feat(gemini): apply persisted Gemini 3 thinking levels`

## 4. Release archive verification

Read `.github/workflows/release.yml` and `RELEASE_README.md`. Preserve generic migration copy, add post-copy and post-tar assertions for `migrations/006_add_thinking_level.sql`, then build/package locally and inspect the archive.

Validate: `cargo build --release`; then `tar tzf rs-summarizer-linux-amd64.tar.gz | grep -Fx 'migrations/006_add_thinking_level.sql'`.

Commit: `ci(release): verify thinking-level migration is packaged`

## 5. Final gate and walkthrough

Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `cargo build --release`. Inspect migration ordering, tar content, and `git status --short`. Commit all intended work, excluding archives/secrets. Only after actual implementation, create `walkthrough.md` here with actual commits, exact test results, migration/archive proof, skipped browser/live tests and why, compatibility choices, learnings, extensions, and container programs added (expected: none).

Commit: `docs(thinking): add implementation walkthrough`
