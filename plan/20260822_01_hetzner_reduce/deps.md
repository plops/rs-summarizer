# Dependencies & Crate Documentation

This document records dependencies for the Hetzner model catalog update (model reduction and Qwen 3.8 27B introduction). **No new crates are introduced** — the existing `async-openai` dependency already supports all OpenAI-compatible endpoints used by Hetzner models.

---

## 1. `async-openai` (Existing — No Version Change)

- **Crate Name**: `async-openai`
- **Version**: `0.41.1`
- **GitHub Repository**: [645068db/async-openai](https://github.com/645068db/async-openai)
- **GitHub Organization / Owner**: `645068db`
- **License**: Apache-2.0
- **Purpose**: Idiomatic async Rust client for OpenAI-compatible REST APIs. Used for Hetzner models (`Qwen/Qwen3.6-35B-A3B-FP8` and `Qwen3.8-27B`) via the `https://inference.hetzner.com/api/v1` endpoint.
- **Cargo.toml Entry**: `async-openai = { version = "0.41.1", features = ["chat-completion"] }`

### DeepWiki Query Reference
- **Repo Target**: `645068db/async-openai`
- **Query Pattern**: `"How to configure custom api_base and handle streaming chat completions in async-openai?"`

### Usage Example
Both Hetzner models utilize the same `async-openai` client configuration — only the `model` identifier in the chat completion request differs:

```rust
use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};
use async_openai::Client;

let config = OpenAIConfig::new()
    .with_api_base(std::env::var("HETZNER_BASE_URL").unwrap_or_else(|_| "https://inference.hetzner.com/api/v1".to_string()))
    .with_api_key(std::env::var("HETZNER_API_KEY").unwrap_or_default());
let client = Client::with_config(config);

// Available Hetzner models:
// 1. "Qwen/Qwen3.6-35B-A3B-FP8" (mapped from internal "hetzner-qwen-3.6-35b")
// 2. "Qwen3.8-27B" (mapped from internal "hetzner-qwen-3.8-27b")

let request = CreateChatCompletionRequestArgs::default()
    .model("Qwen3.8-27B")
    .messages([
        ChatCompletionRequestSystemMessageArgs::default()
            .content("You are a concise video summarizer.")
            .build()?,
        ChatCompletionRequestUserMessageArgs::default()
            .content("Summarize this text: ...")
            .build()?,
    ])
    .stream(true)
    .build()?;
```

---

## 2. Existing Workspace Dependencies Utilized (No Changes)

- **`reqwest`** (`0.12`, Org: `seanmonstar/reqwest`): HTTP transport
- **`serde` / `serde_json`** (`1.0`, Org: `serde-rs/json`): Model config serialization and validation
- **`tokio`** (`1.52`, Org: `tokio-rs/tokio`): Async runtime & concurrency primitives
- **`tracing`** (`0.1`, Org: `tokio-rs/tracing`): Structured logging & telemetry
- **`futures-util`** (`0.3`, Org: `rust-lang/futures-rs`): Stream processing for SSE chunks

---

## 3. System-Level Dependencies

No new system packages, background services, or Docker tools are required for this update. The container's existing OpenSSL and CA certificate infrastructure handles all HTTPS requests to Hetzner's API.
