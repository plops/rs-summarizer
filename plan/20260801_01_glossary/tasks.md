# Serial Task List: Glossary & German Output Feature

Each task must be completed and validated sequentially before moving to the next.

---

### Task 1: Create Database Migration
- **Action**: Create `migrations/005_add_glossary_and_language_options.sql` to add index `idx_summaries_lang_glossary`.
- **Validation**: Run `cargo check` and verify sqlx migrations run cleanly on startup.

---

### Task 2: Update Rust Data Models
- **Action**: Update `SubmitForm` in `src/models.rs` to include `include_glossary: bool` and `output_language: String`.
- **Validation**: Run `cargo test models` or `cargo check`.

---

### Task 3: Update Database Persistence Layer
- **Action**: Update `insert_new_summary` in `src/db.rs` to bind `form.include_glossary` and `form.output_language`.
- **Validation**: Write and run unit test in `src/db.rs` verifying insertion of `include_glossary` and `output_language`.

---

### Task 4: Update HTTP Route Handlers
- **Action**: Update `process_transcript` in `src/routes/mod.rs` to pass `include_glossary` and `output_language` during batch URL processing.
- **Validation**: Run `cargo test routes`.

---

### Task 5: Update Background Task Pipeline
- **Action**: Update `process_summary_inner` and `run_model_pipeline` in `src/tasks.rs` to pass `summary.include_glossary` and `summary.output_language` to `SummaryService::generate_summary`.
- **Validation**: Run `cargo test tasks`.

---

### Task 6: Prompt Engineering for Glossary & German Output
- **Action**: Update `generate_summary`, `build_prompt`, `build_hn_prompt`, `build_prompt_for_gemma`, and `generate_summary_hetzner` in `src/services/summary.rs` to incorporate prompt instructions for Glossary and German language directives.
- **Validation**: Write and run unit tests in `src/services/summary.rs` checking prompt output for Glossary and German mode.

---

### Task 7: Update Frontend Interface
- **Action**: Add Glossary checkbox and Output Language selector to `templates/index.html` under Advanced Options.
- **Validation**: Run `cargo check` (Askama template compile-time check).

---

### Task 8: Code Quality & Full Test Suite Execution
- **Action**: Run `cargo fmt -- --check`, `cargo clippy -- -W clippy::all`, and `cargo test`.
- **Validation**: Zero clippy warnings, clean formatting, all 118+ unit/integration tests pass.

---

### Task 9: Git Commits
- **Action**: Commit changes following Conventional Commits format with detailed body.
- **Validation**: Run `git status` and `git log -n 1`.

---

### Task 10: Generate Walkthrough Document
- **Action**: Create `plan/20260801_01_glossary/walkthrough.md` summarizing changes, test results, learnings, future improvements, and Docker environment notes.
- **Validation**: Verify file exists and contains complete details.
