# Dependencies & Crate Documentation

This document records any new dependencies introduced for the Hetzner AI API integration along with their GitHub organization, exact version, purpose, and usage examples. This enables autonomous agents to easily formulate DeepWiki or documentation queries for external crates.

---

## 1. `async-openai`

- **Crate Name**: `async-openai`
- **Version**: `0.41.1` (latest version)
- **GitHub Repository**: [645068db/async-openai](https://github.com/645068db/async-openai)
- **GitHub Organization / Owner**: `645068db`
- **License**: Apache-2.0
- **Purpose**: Provides an idiomatic, async Rust client for OpenAI-compatible REST APIs. Used to interact with Hetzner's experimental inference platform (`https://inference.hetzner.com/api/v1`) using standard `/v1/chat/completions` endpoints and SSE (Server-Sent Events) streaming.

### Usage Example: Custom Base URL & Streaming Chat Completions

```rust
use async_openai::{
    config::OpenAIConfig,
    types::{
        CreateChatCompletionRequestArgs,
        ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs,
    },
    Client,
};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Configure client for Hetzner experimental OpenAI-compatible endpoint
    let api_key = std::env::var("HETZNER_API_KEY")
        .unwrap_or_else(|_| "2jwqK0zWB54O0ipIzRtmv9jHme7jSazg".to_string());
    let base_url = std::env::var("HETZNER_BASE_URL")
        .unwrap_or_else(|_| "https://inference.hetzner.com/api/v1".to_string());

    let config = OpenAIConfig::new()
        .with_api_base(base_url)
        .with_api_key(api_key);

    let client = Client::with_config(config);

    // 2. Build chat completion request
    let request = CreateChatCompletionRequestArgs::default()
        .model("Qwen/Qwen3.6-35B-A3B-FP8")
        .messages([
            ChatCompletionRequestSystemMessageArgs::default()
                .content("You are an adaptive knowledge synthesis engine.")
                .build()?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content("Summarize the main principles of modern systems architecture.")
                .build()?
                .into(),
        ])
        .stream(true)
        .build()?;

    // 3. Execute streaming request
    let mut stream = client.chat().create_stream(request).await?;

    while let Some(result) = stream.next().await {
        match result {
            Ok(response) => {
                for choice in response.choices {
                    if let Some(content) = choice.delta.content {
                        print!("{}", content);
                    }
                }
            }
            Err(err) => {
                eprintln!("Error in stream: {}", err);
                break;
            }
        }
    }

    Ok(())
}
```

### DeepWiki Query Reference Format
To query information regarding this crate in DeepWiki or external docs, use:
- **Repo Target**: `645068db/async-openai`
- **Query Pattern**: `"How to configure custom api_base and handle streaming in async-openai?"`

---

## 2. Existing Workspace Dependencies Utilized

The following pre-existing workspace dependencies are also leveraged in this feature:

- **`reqwest`** (`0.12`/`0.13`, Org: `seanmonstar/reqwest`): Underlying HTTP transport for API requests and health probes.
- **`serde` / `serde_json`** (`1.0`, Org: `serde-rs/json`): Deserialization of Hetzner models, fallback parameters, and pricing metadata.
- **`tokio`** (`1.52`, Org: `tokio-rs/tokio`): Async runtime and task orchestration.
- **`tracing`** (`0.1`, Org: `tokio-rs/tracing`): Structured logging across API calls and rate-limiting loops.
