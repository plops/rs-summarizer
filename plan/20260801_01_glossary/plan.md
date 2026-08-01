# Implementation Plan: Glossary Generation & German Output Options

## 1. Overview & Goal

User **Wol Pumba** (`wolpumba@gmail.com`) requested an extension to `rs-summarizer`. When processing videos or articles with complex medical/technical content (e.g., filovirus strains, Nirsevimab RSV prophylactics, striatal VMAT2 PET imaging in Long COVID, acute renal allograft loss), standard high-density abstracts can be difficult for non-specialists to understand.

To solve this, we are adding two user-configurable features:
1. **Technical & Medical Glossary Generation (`include_glossary`)**: Appends a dedicated `## Glossary` (or `## Glossar`) section to the summary, explaining technical jargon, acronyms, and domain-specific medical/scientific terms in plain language.
2. **German Output Support (`output_language`)**: Allows the user to receive the entire summary (abstract, bullet points, timestamps, discussion, and glossary) in German (`de`).
3. **GUI Controls**: Adds clean HTML inputs to `templates/index.html` allowing users to toggle glossary inclusion and select their preferred output language.
4. **Database Persistence & Migration**: Stores `include_glossary` and `output_language` per summary in SQLite and adds database migration `005_add_glossary_and_language_options.sql` to create index `idx_summaries_lang_glossary`.

---

## 2. Codebase Context for Autonomous Agents

An AI agent working on this codebase should inspect the following files before making changes:

- [Cargo.toml](file:///workspace/src/rs-summarizer/Cargo.toml): Core dependencies, feature flags, and member crates.
- [migrations/001_initial.sql](file:///workspace/src/rs-summarizer/migrations/001_initial.sql): Base SQLite schema containing existing `include_glossary` and `output_language` columns in `summaries`.
- [src/models.rs](file:///workspace/src/rs-summarizer/src/models.rs): Data transfer structs (`SubmitForm`) and DB model (`Summary`).
- [src/db.rs](file:///workspace/src/rs-summarizer/src/db.rs): Database initialization and CRUD queries (`insert_new_summary`, `fetch_summary`).
- [src/routes/mod.rs](file:///workspace/src/rs-summarizer/src/routes/mod.rs): HTTP route handlers (`process_transcript`, `index`).
- [src/tasks.rs](file:///workspace/src/rs-summarizer/src/tasks.rs): Background task orchestration (`process_summary`, `run_model_pipeline`).
- [src/services/summary.rs](file:///workspace/src/rs-summarizer/src/services/summary.rs): Gemini AI prompt construction (`build_prompt`, `build_hn_prompt`, `generate_summary`).
- [prompts/system_instruction.txt](file:///workspace/src/rs-summarizer/prompts/system_instruction.txt): System instruction for domain-specific persona adoption and structural repeatability.
- [templates/index.html](file:///workspace/src/rs-summarizer/templates/index.html): HTML form interface for submit options.
- [tests/integration_pipeline.rs](file:///workspace/src/rs-summarizer/tests/integration_pipeline.rs): Integration test suite.

---

## 3. Commit Message Guidelines

All commits must strictly adhere to the **Conventional Commits** specification:

Format:
```
<type>(<scope>): <short description>

<detailed body explaining why and what changed>
```

Types:
- `feat`: New user-facing feature.
- `fix`: Bug fix.
- `test`: Adding or modifying tests.
- `docs`: Documentation updates.
- `refactor`: Code restructuring without behavior changes.

Example:
```
feat(summary): add glossary generation and German output options

- Update SubmitForm to deserialize include_glossary and output_language
- Modify insert_new_summary in db.rs to persist options
- Update SummaryService prompt builder to include Glossary and German language directives
- Add GUI controls to index.html
- Add database migration 005_add_glossary_and_language_options.sql
```

---

## 4. Requirements Audit & Design Proposals

### Audited Requirements:
1. **Glossary Directive**: When `include_glossary` is true, prompt the LLM to extract and define technical/medical terms (acronyms, clinical jargon, specialized metrics) in an accessible `## Glossary` / `## Glossar` section.
2. **Language Directive**: When `output_language` is `"de"` or `"de-DE"`, generate all section headers and bullet points in German. Default (`"en"`) outputs in English.
3. **GUI Controls**: Add a checkbox for Glossary and a dropdown for Language under Advanced Options in `templates/index.html`.
4. **DB Schema & Persistence**: Ensure `insert_new_summary` binds `include_glossary` and `output_language`. Create migration `005_add_glossary_and_language_options.sql` to optimize query performance via index `idx_summaries_lang_glossary`.

### Additional Value-Add Proposals:
1. **Auto-Glossary Heuristic for Medical / Highly Technical Domains**: If a transcript contains high density of medical/scientific jargon (e.g. >10 medical terms or clinical metrics), the backend can automatically enable glossary if user hasn't explicitly disabled it.
2. **Custom CSS Styling for Glossary**: Render `## Glossary` sections with distinct visual callout styling in `markdown_renderer` or CSS for better readability.
3. **Comprehensive Unit & Integration Test Coverage**: Test prompt generation for English vs. German, Glossary enabled vs. disabled, form deserialization, and DB insertion.

---

## 5. Detailed Implementation Strategy

### Step 1: Database Migration
Create `migrations/005_add_glossary_and_language_options.sql`:
```sql
CREATE INDEX IF NOT EXISTS idx_summaries_lang_glossary
    ON summaries (output_language, include_glossary);
```

### Step 2: Update Rust Data Models
Update `SubmitForm` in `src/models.rs`:
```rust
#[derive(Debug, serde::Deserialize)]
pub struct SubmitForm {
    pub original_source_link: String,
    pub transcript: Option<String>,
    pub model: String,
    #[serde(default)]
    pub google_search_grounding: bool,
    #[serde(default)]
    pub url_context: bool,
    #[serde(default)]
    pub include_glossary: bool,
    #[serde(default = "default_output_language")]
    pub output_language: String,
}

fn default_output_language() -> String {
    "en".to_string()
}
```

### Step 3: Update Database Functions
In `src/db.rs`, update `insert_new_summary`:
```rust
pub async fn insert_new_summary(
    db: &SqlitePool,
    form: &SubmitForm,
    host: &str,
    timestamp_start: &str,
) -> Result<i64, sqlx::Error> {
    let transcript = form.transcript.as_deref().unwrap_or("");
    let lang = if form.output_language.is_empty() { "en" } else { &form.output_language };

    let result = sqlx::query(
        "INSERT INTO summaries (model, original_source_link, transcript, host, summary_timestamp_start, google_search_grounding, url_context, include_glossary, output_language) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&form.model)
    .bind(&form.original_source_link)
    .bind(transcript)
    .bind(host)
    .bind(timestamp_start)
    .bind(form.google_search_grounding)
    .bind(form.url_context)
    .bind(form.include_glossary)
    .bind(lang)
    .execute(db)
    .await?;

    Ok(result.last_insert_rowid())
}
```

### Step 4: Update Route Handlers & Task Pipeline
In `src/routes/mod.rs`: Ensure `SubmitForm` clones `include_glossary` and `output_language` during batch URL processing.
In `src/tasks.rs`: Update `process_summary_inner` and `run_model_pipeline` to retrieve `summary.include_glossary` and `summary.output_language` and pass them to `SummaryService::generate_summary`.

### Step 5: Prompt Engineering in `src/services/summary.rs`
Update `SummaryService::generate_summary`, `build_prompt`, `build_hn_prompt`, and `build_prompt_for_gemma` to accept `include_glossary: bool` and `output_language: &str`:
- If `output_language` is `"de"`, append explicit German translation directives and localized section headers (`## Zusammenfassung` / `## Abstract`, `## Wichtigste Punkte & Zeitstempel`, `## Glossar`).
- If `include_glossary` is `true`, append explicit instructions to generate a `## Glossary` (or `## Glossar`) section defining technical/medical jargon, acronyms, and specialized concepts.

### Step 6: Frontend GUI (`templates/index.html`)
Add form inputs under Advanced Options:
```html
<div style="margin-bottom: 0.5rem;">
    <label for="include_glossary">
        <input type="checkbox" id="include_glossary" name="include_glossary" value="true" role="switch">
        Glossar für Fachbegriffe / Jargon einschließen
    </label>
</div>

<div style="margin-bottom: 0.5rem;">
    <label for="output_language">Ausgabesprache</label>
    <select id="output_language" name="output_language">
        <option value="en" selected>Original / Englisch</option>
        <option value="de">Deutsch</option>
    </select>
</div>
```

### Step 7: Testing & Verification
- Unit tests for `build_prompt` with language and glossary options.
- DB migration and insert unit tests.
- Full compilation with `cargo check`, `cargo fmt -- --check`, `cargo clippy`, and `cargo test`.
