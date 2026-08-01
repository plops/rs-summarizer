# Dependencies for Glossary & German Output Feature

This document lists the crate dependencies and third-party libraries involved in the implementation of the **Glossary & Output Language Selection** feature in `rs-summarizer`.

## Crate Dependencies

All features are implemented using existing workspace dependencies. No new external Rust crates were added to `Cargo.toml`.

| Crate Name | Version | Purpose in Feature | GitHub Organization / Repo |
| :--- | :--- | :--- | :--- |
| `serde` | `1.0` | Deserializing `SubmitForm` with `include_glossary` and `output_language` fields | `serde-rs/serde` |
| `sqlx` | `0.9` | SQLite database persistence and migration execution (`005_add_glossary_and_language_options.sql`) | `launchbadge/sqlx` |
| `axum` | `0.8` | Handling HTTP POST requests (`/process_transcript`) and form processing | `tokio-rs/axum` |
| `askama` | `0.16` | Type-safe HTML template rendering (`templates/index.html`) | `djc/askama` |
| `gemini-rust` | `1.7.1` | Executing Gemini API calls with glossary and language prompt directives | `google / gemini-rust` |
| `async-openai` | `0.41.1` | Executing OpenAI-compatible Hetzner API calls with custom system prompts | `64bit/async-openai` |
| `chrono` | `0.4` | Formatted date strings for prompt injection | `chronotope/chrono` |

## Usage Examples (from DeepWiki & Crates)

### Serde Form Deserialization
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

### SQLx SQLite Query Binding
```rust
sqlx::query(
    "INSERT INTO summaries (model, original_source_link, transcript, host, summary_timestamp_start, google_search_grounding, url_context, include_glossary, output_language) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
)
.bind(&form.model)
.bind(&form.original_source_link)
.bind(transcript)
.bind(host)
.bind(timestamp_start)
.bind(form.google_search_grounding)
.bind(form.url_context)
.bind(form.include_glossary)
.bind(&form.output_language)
.execute(db)
.await?;
```
