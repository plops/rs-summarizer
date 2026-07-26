# Serialized Implementation & Testing Task List (`task.md`)

This task list is designed for serial execution by an autonomous agent or developer. Each task contains precise instructions, target files, and verification steps.

---

- [x] **Task 1: Add Dependency and Document in `deps.md`**
  - **Action**: Add `async-openai = "0.41.1"` to `Cargo.toml` and write `deps.md` with GitHub organization (`645068db/async-openai`) and usage examples.
  - **Target Files**: `Cargo.toml`, `plan/20260726_01_hetzner/deps.md`
  - **Verification**: Run `cargo check` to verify dependency resolves cleanly.

- [ ] **Task 2: Extend Model State & Configuration**
  - **Action**: 
    1. Add `Hetzner` variant to `ModelArchitecture` enum in `src/state.rs`.
    2. Implement `as_str()` returning `"Hetzner"` for `ModelArchitecture::Hetzner`.
    3. Add `hetzner-qwen-3.6-35b` model entry to `get_default_models()` in `src/state.rs` and `config/models.json`.
  - **Target Files**: `src/state.rs`, `config/models.json`
  - **Verification**: Run `cargo test state::model_checks` to confirm unique names, valid pricing, and JSON deserialization.

- [ ] **Task 3: Implement Hetzner Streaming Provider in `SummaryService`**
  - **Action**:
    1. Update `SummaryService` in `src/services/summary.rs` to support Hetzner OpenAI-compatible REST API.
    2. Read `HETZNER_API_KEY` (fallback `2jwqK0zWB54O0ipIzRtmv9jHme7jSazg`) and `HETZNER_BASE_URL` (fallback `https://inference.hetzner.com/api/v1`).
    3. Implement `generate_summary_hetzner` helper or dispatch inside `generate_summary` using `async-openai` client streaming (`chat().create_stream()`).
    4. Progressively persist chunks to SQLite DB using `db::update_summary_chunk`.
    5. Handle error responses and token usage extraction.
  - **Target Files**: `src/services/summary.rs`
  - **Verification**: Run `cargo test services::summary` to ensure existing and new tests compile and run.

- [ ] **Task 4: Update Background Pipeline & Fallback Chain**
  - **Action**:
    1. Update `get_fallback_chain` in `src/tasks.rs` to include `hetzner-qwen-3.6-35b` in fallback paths.
    2. Ensure `run_model_pipeline` passes options correctly for Hetzner architecture.
  - **Target Files**: `src/tasks.rs`
  - **Verification**: Run `cargo test tasks::tests` to verify fallback chains.

- [ ] **Task 5: Update Frontend Options Scoping**
  - **Action**: Update `templates/index.html` to hide Gemini-only feature checkboxes (Grounding and URL Context) when a Hetzner model is selected.
  - **Target Files**: `templates/index.html`
  - **Verification**: Check HTML template syntax and JavaScript conditional logic.

- [ ] **Task 6: Add Unit & Integration Tests for Hetzner Provider**
  - **Action**: Add unit tests in `src/services/summary.rs` and `src/state.rs` for Hetzner model parsing, prompt building, cost calculation, and architecture mapping.
  - **Target Files**: `src/services/summary.rs`, `src/state.rs`, `tests/`
  - **Verification**: Run `cargo test`.

- [ ] **Task 7: Run Full Verification Suite**
  - **Action**: Run `cargo test`, `cargo fmt --check` (or `cargo fmt`), and `cargo clippy`.
  - **Target Files**: Workspace
  - **Verification**: All 113+ tests pass with zero errors.

- [ ] **Task 8: Git Commit with Conventional Format**
  - **Action**: Stage modified files and commit using Conventional Commit format with comprehensive descriptions.
  - **Target Files**: Workspace git repository
  - **Verification**: Run `git log -n 1` to verify commit message structure.

- [ ] **Task 9: Produce Walkthrough Document**
  - **Action**: Create `plan/20260726_01_hetzner/walkthrough.md` documenting what was implemented, test results, learnings, future extensions, and container dependencies.
  - **Target Files**: `plan/20260726_01_hetzner/walkthrough.md`
  - **Verification**: Verify file exists and is complete.
