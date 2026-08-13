# Task Checklist: Gemini 3.7 Flash Integration

- [x] **Task 1: Update Model Configuration in `config/models.json` & `src/state.rs`**
  - [x] Add `gemini-3.7-flash` (RPM 5, RPD 20, context 1,000,000, prices $0.10/$0.40) to `config/models.json`.
  - [x] Add `gemini-3.7-flash` to `get_default_models()` in `src/state.rs` right after `auto` and before `gemini-3.6-flash`.
  - [x] Update `test_updated_model_limits` in `src/state.rs` to verify `gemini-3.7-flash`.
  - [x] Validate: Run `cargo test state::model_checks`.

- [x] **Task 2: Update Heuristics and Fallback Chains in `src/tasks.rs`**
  - [x] Update `run_model_pipeline` in `src/tasks.rs` to select `gemini-3.7-flash` for long transcripts (>= 1800s) and long HN submissions (>= 3000 words).
  - [x] Add `gemini-3.7-flash` fallback chain in `get_fallback_chain`.
  - [x] Update fallback chains for `hetzner-qwen-3.6-35b`, `gemini-3.6-flash`, `gemini-3.5-flash-lite`, `gemini-3.1-flash-lite`, etc., to incorporate `gemini-3.7-flash`.
  - [x] Update and expand unit tests `test_get_fallback_chain_new_models` and `test_hn_model_auto_selection_thresholds` in `src/tasks.rs`.
  - [x] Validate: Run `cargo test tasks::tests`.

- [x] **Task 3: Update Skill & Project Documentation**
  - [x] Update `.kiro/skills/gemini-api-models/SKILL.md` to list `gemini-3.7-flash` and its quota parameters.
  - [x] Validate: Check markdown consistency.

- [x] **Task 4: Clean Up Clippy Warnings and Format Codebase**
  - [x] Implement `Default` for `MetadataCache` and `HackerNewsService`.
  - [x] Simplify `map_or` to `is_some_and` and remove unnecessary return in rate limiter.
  - [x] Run `cargo fmt` across the workspace.
  - [x] Validate: Run `cargo fmt -- --check` and `cargo clippy -- -W clippy::all`.

- [x] **Task 5: Execute Full Verification and Test Suite**
  - [x] Run all unit tests and integration tests via `cargo test`.
  - [x] Verify 100% test pass rate with zero warnings.

- [x] **Task 6: Generate Post-Implementation Walkthrough**
  - [x] Write `plan/20260813_01_gemini37/walkthrough.md` summarizing what was implemented, test results, learnings, and docker container tool recommendations.

- [x] **Task 7: Commit and Release**
  - [x] Git commit all changes following Conventional Commit guidelines.
  - [x] Execute release bump (`scripts/release.sh 1.3.0` or cargo version bump & tag).
