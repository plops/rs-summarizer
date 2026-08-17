# Dependencies & Crate Documentation

This document records dependencies for the Hetzner multi-model expansion. **No new crates are introduced** — the existing `async-openai` dependency already supports all OpenAI-compatible endpoints used by the new Hetzner models.

---

## 1. `async-openai` (Existing — No Version Change)

- **Crate Name**: `async-openai`
- **Version**: `0.41.1` (unchanged from previous Hetzner integration)
- **GitHub Repository**: [645068db/async-openai](https://github.com/645068db/async-openai)
- **GitHub Organization / Owner**: `645068db`
- **License**: Apache-2.0
- **Purpose**: Idiomatic async Rust client for OpenAI-compatible REST APIs. Used for all 4 Hetzner models via the same `https://inference.hetzner.com/api/v1` endpoint.
- **Cargo.toml Entry**: `async-openai = { version = "0.41.1", features = ["chat-completion"] }`

### DeepWiki Query Reference
- **Repo Target**: `645068db/async-openai`
- **Query Pattern**: `"How to configure custom api_base and handle streaming chat completions in async-openai?"`

### Usage (unchanged from previous integration)
All 4 Hetzner models use the same `async-openai` client configuration — only the `model` parameter in the chat completion request differs:

```rust
use async_openai::config::OpenAIConfig;
use async_openai::Client;

let config = OpenAIConfig::new()
    .with_api_base("https://inference.hetzner.com/api/v1")
    .with_api_key("2jwqK0zWB54O0ipIzRtmv9jHme7jSazg");
let client = Client::with_config(config);

// Model name varies per selection:
// - "Qwen/Qwen3.6-35B-A3B-FP8"
// - "DeepSeek-V4-Flash-0731"
// - "GLM-5.2-NVFP4"
// - "Kimi-K2.7-Code"
```

---

## 2. Existing Workspace Dependencies Utilized (No Changes)

- **`reqwest`** (`0.12`, Org: `seanmonstar/reqwest`): HTTP transport
- **`serde` / `serde_json`** (`1.0`, Org: `serde-rs/json`): Model config serialization
- **`tokio`** (`1.52`, Org: `tokio-rs/tokio`): Async runtime
- **`tracing`** (`0.1`, Org: `tokio-rs/tracing`): Structured logging
- **`futures-util`** (`0.3`, Org: `rust-lang/futures-rs`): Stream processing for SSE chunks

---

## 3. System-Level Dependencies

No new system packages or tools are required for this feature. The Docker container does not need any additional programs installed.
