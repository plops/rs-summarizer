# Implementation Plan: Gemini 3.7/3.8 Thinking-Level Selection

**Project:** `plops/rs-summarizer`  
**Primary scope:** Gemini 3.7 Flash and Gemini 3.8 Flash. Both models must let a user choose thinking effort in the web UI; that exact choice must be stored with the summary and used in the Gemini request.

## Current-state evidence

`src/services/summary.rs` already unconditionally sends `ThinkingLevel::High` for every Gemini model whose name contains `gemini-3`. Gemini 3.7 and 3.8 therefore use a provider-supported API, but users cannot control it. Gemini 2.5 instead uses numeric `with_thinking_budget`; Gemma, Hetzner/Qwen, and Other receive neither setting.

The full path is `templates/index.html` → `SubmitForm` → `routes::process_transcript` → `db::insert_new_summary` → `summaries` row → `tasks::process_summary_inner` → `run_model_pipeline` → `SummaryService::generate_summary` → Gemini builder.

`db::init_db` embeds/applies `migrations/` using `sqlx::migrate!`. `.github/workflows/release.yml` recursively copies that directory to `staging/` before tar creation. Add explicit staging and archive checks for the new migration so this contract is proved, not assumed.

## Requirements and recommended decisions

1. Add a labelled web selector for `auto`, `minimal`, `low`, `medium`, and `high`. `auto` is defaulted to use the default Gemini 3.7/3.8 behavior.
2. Enable named values only for `gemini-3.*`. Other models show an explanation and submit `auto`; server-side validation is authoritative.
3. Create immutable `migrations/006_add_thinking_level.sql`; never edit earlier SQLx migrations because their checksums are immutable.
4. Persist a normalized value with every `summaries` row, including every multi-URL row. Existing `thinking` and `thinking_tokens` store returned thought text/usage, not configuration.
5. The task reads the saved setting from `Summary`, then passes it to generation. This survives delays, restarts, and fallback attempts.
6. Map 3.* named values one-to-one to `gemini_rust::ThinkingLevel`. `auto` omits `with_thinking_level`.
7. Do not invent mappings for Gemini 2.5 budgets or Qwen/Gemma in this feature. Preserve 2.5 behavior and send no new setting to unsupported providers.
8. Prove the migration applies and ships in the release tarball.

The recommended migration is: `ALTER TABLE summaries ADD COLUMN thinking_level TEXT NOT NULL DEFAULT 'high' CHECK (thinking_level IN ('auto', 'minimal', 'low', 'medium', 'high'));`.

Create a `ThinkingPreference` type in `src/models.rs`; persist stable lowercase values, not SDK enum/provider spellings. Reject malformed values at the route boundary before insertion.

## File context for the implementing agent

| File | Reason |
| --- | --- |
| `prompt.txt`, `deps.md` | Original objective and checked API evidence. |
| `src/models.rs` | `Summary`, `SubmitForm`, validated preference type. |
| `templates/index.html` | Form and current model visibility JavaScript. |
| `src/routes/mod.rs` | Validation, multi-URL `single_input`, insert/task spawn. |
| `src/db.rs` | Migrations, insert/fetch, in-memory test helpers. |
| `migrations/001_initial.sql`, `002_add_grounding_and_url_context.sql` | Schema/default conventions. |
| `src/tasks.rs` | Persisted-row fetch, fallback, generation call. |
| `src/services/summary.rs` | Existing hard-coded high/budget policy. |
| `src/state.rs`, `config/models.json` | Exact model names and architectures. |
| `tests/integration_pipeline.rs`, `integration_ratings.rs`, `integration_browser.rs` | Test patterns. |
| `.github/workflows/release.yml`, `RELEASE_README.md` | Package and migration contract. |

## Provider design

Create a pure helper in `src/services/summary.rs`, e.g. `gemini_3_thinking_level(model_name, preference) -> Option<ThinkingLevel>`. It returns a named level only for 3.* and returns `None` for `auto` or every other model. Unit-test this helper without an API key.

Replace the broad hard-coded Gemini-3 high branch with this logic: a named 3.* level configures `with_thinking_level(level).with_thoughts_included(true)`; `auto` configures neither; Gemini 2.5 retains its existing budget branch and uses `with_thinking_budget(...).with_thoughts_included(true)`. The two methods must never be combined.

If fallback changes to an unsupported model, preserve the originally selected value in SQLite, avoid an invalid provider field, and log the controlled downgrade.

## Acceptance tests

| Coverage | Required proof |
| --- | --- |
| Parsing/default | Five valid values parse; invalid values fail; omitted form defaults to high. |
| Migration | Fresh and legacy in-memory DBs apply 006; column is non-null, constrained, and defaults to high. |
| Persistence | Insert/fetch round-trips all valid values. |
| Form/fan-out | 3.* medium survives every multi-URL row; unsupported/malformed inputs are rejected/normalized. |
| Template | Label, five values, help text, high default, and model-aware capability behavior render. |
| Mapping | 3.* named values map correctly; auto/unsupported map None; 2.5 budget path remains separate. |
| Release | Workflow checks staging and `tar tzf` for `migrations/006_add_thinking_level.sql`. |

Mandatory commands: `cargo fmt --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test`; `cargo build --release`; and `tar tzf rs-summarizer-linux-amd64.tar.gz | grep -Fx 'migrations/006_add_thinking_level.sql'` after locally reproducing the package block.

No live Gemini request is required: it costs money, needs credentials, and is weaker than deterministic mapping tests. An authorized smoke test may be recorded separately.

## Commit messages

Use Conventional Commits with detailed rationale, compatibility, files changed, and test evidence:

1. `feat(thinking): persist validated Gemini 3 thinking preferences`
2. `feat(ui): expose Gemini 3.* thinking levels`
3. `feat(gemini): apply persisted Gemini 3 thinking levels`
4. `ci(release): verify thinking-level migration is packaged`
5. `docs(thinking): add implementation walkthrough`

Example body: “Map the saved per-request preference to gemini-rust ThinkingLevel for Gemini 3.* only. Auto omits the provider setting; Gemini 2.5 retains its separate budget behavior. Tests: cargo test services::summary::tests::thinking.”

## Suggested later work

Display selection and actual thought tokens in Browse; permit an administrator default; model capabilities in `ModelOption` instead of name checks; and a separately documented Gemini-2.5 numeric-budget UI after provider bounds are approved.
