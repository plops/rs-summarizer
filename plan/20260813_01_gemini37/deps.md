# Dependencies & Crate Documentation

This document records the dependencies utilized for Gemini 3.7 Flash and existing LLM model interactions along with their GitHub organization, exact version, purpose, and usage examples. This enables autonomous agents to easily formulate DeepWiki or documentation queries for external crates.

---

## 1. `gemini-rust`

- **Crate Name**: `gemini-rust`
- **Version**: `1.7.1` (workspace locked)
- **GitHub Repository**: [flachesis/gemini-rust](https://github.com/flachesis/gemini-rust)
- **GitHub Organization / Owner**: `flachesis`
- **License**: MIT / Apache-2.0
- **Purpose**: Provides an asynchronous client for Google's Gemini REST/RPC API. Used to interact with Gemini models (`gemini-3.7-flash`, `gemini-3.6-flash`, `gemini-3.5-flash-lite`, etc.) supporting streaming responses, thinking mode (`ThinkingLevel::High`), Google Search Grounding (`Tool::google_search()`), and URL Context (`Tool::url_context()`).

### Usage Example: Gemini 3.7 Flash with High Thinking Level & Streaming

```rust
use gemini_rust::{Gemini, Model, ThinkingLevel, Tool};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY must be set");

    // Configure Gemini 3.7 Flash custom model identifier
    let model = Model::Custom("models/gemini-3.7-flash".to_string());
    let client = Gemini::with_model(&api_key, model)?;

    // Build streaming request with thinking level and tools
    let mut builder = client.generate_content();
    builder = builder
        .with_system_prompt("You are an adaptive knowledge synthesis engine.")
        .with_thinking_level(ThinkingLevel::High)
        .with_thoughts_included(true)
        .with_tool(Tool::google_search())
        .with_tool(Tool::url_context());

    let mut stream = builder.stream("Summarize recent developments in AI hardware.").await?;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                if let Some(candidate) = chunk.candidates.first() {
                    for part in &candidate.content.parts {
                        if let Some(text) = &part.text {
                            print!("{}", text);
                        }
                    }
                }
            }
            Err(err) => {
                eprintln!("Streaming error: {}", err);
                break;
            }
        }
    }

    Ok(())
}
```

### DeepWiki Query Reference Format
To query information regarding this crate in DeepWiki or external docs, use:
- **Repo Target**: `flachesis/gemini-rust`
- **Query Pattern**: `"How to configure custom model identifiers and streaming in gemini-rust?"`

---

## 2. Existing Workspace Dependencies Utilized

The following pre-existing workspace dependencies are also leveraged in this feature:

- **`async-openai`** (`0.41.1`, Org: `645068db/async-openai`): OpenAI-compatible client used for alternative/Hetzner inference backends.
- **`reqwest`** (`0.12`, Org: `seanmonstar/reqwest`): Underlying HTTP transport for API requests and health probes.
- **`serde` / `serde_json`** (`1.0`, Org: `serde-rs/json`): Deserialization of model configurations (`config/models.json`) and API payloads.
- **`sqlx`** (`0.9`, Org: `launchbadge/sqlx`): Asynchronous SQLite storage for persistence of summaries and model tokens.
- **`tokio`** (`1.52`, Org: `tokio-rs/tokio`): Async runtime, mutexes, and background task orchestration.
- **`tracing`** (`0.1`, Org: `tokio-rs/tracing`): Structured logging across API calls, rate-limiting loops, and fallback steps.
