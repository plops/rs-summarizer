---
inclusion: manual
---

# Gemini API Model Compatibility

## Overview

rs-summarizer uses the `gemini-rust` crate to interact with Google's Gemini API for text generation and embeddings. Model names must exactly match the API identifiers.

## Model Name Format

Models are referenced via `Model::Custom(format!("models/{}", name))`. The `name` field in `ModelOption` is the part after `models/`.

```rust
let gemini_model = Model::Custom(format!("models/{}", model.name));
let client = Gemini::with_model(&self.api_key, gemini_model)?;
```

## Available Text Generation Models (as of April 2026)

| Config Name | API ID | RPM | RPD | Notes |
|-------------|--------|-----|-----|-------|
| `gemini-3-flash-preview` | `models/gemini-3-flash-preview` | 5 | 20 | Best quality |
| `gemini-3.1-flash-lite-preview` | `models/gemini-3.1-flash-lite-preview` | 15 | 500 | Best quota |
| `gemini-2.5-flash` | `models/gemini-2.5-flash` | 5 | 20 | Solid all-rounder |
| `gemini-2.5-flash-lite` | `models/gemini-2.5-flash-lite` | 10 | 20 | Lightweight |
| `gemma-4-31b-it` | `models/gemma-4-31b-it` | 15 | 1500 | Free, no system prompt |
| `gemma-4-26b-a4b-it` | `models/gemma-4-26b-a4b-it` | 15 | 1500 | Free, no system prompt |
| `gemma-3-27b-it` | `models/gemma-3-27b-it` | 30 | 14400 | Free, massive quota |
| `gemma-3-12b-it` | `models/gemma-3-12b-it` | 30 | 14400 | Free |
| `gemma-3-4b-it` | `models/gemma-3-4b-it` | 30 | 14400 | Free, small |
| `gemma-3-1b-it` | `models/gemma-3-1b-it` | 30 | 14400 | Free, tiny |

## Embedding Model

| Config Name | API ID | RPM | RPD |
|-------------|--------|-----|-----|
| `gemini-embedding-001` | `models/gemini-embedding-001` | 100 | 1000 |

**Important**: Do NOT use `Model::TextEmbedding004` — that maps to `models/text-embedding-004` which doesn't exist. Use `Model::Custom` instead.

## System Prompt Compatibility

**Gemini models** support system prompts via `.with_system_prompt()`.

**Gemma models** do NOT support system prompts ("Developer instruction is not enabled"). The code conditionally skips the system prompt:

```rust
let mut builder = client.generate_content();
if !model.name.starts_with("gemma") {
    builder = builder.with_system_prompt("...");
}
let mut stream = builder.with_user_message(&prompt).execute_stream().await?;
```

## Streaming

Text generation uses `.execute_stream()` which returns an async stream of chunks. Each chunk may contain:
- `response.text()` — the generated text fragment
- `response.usage_metadata` — token counts (typically only in the last chunk)

## Rate Limit Detection

```rust
fn is_rate_limit_error(err_str: &str) -> bool {
    err_str.contains("ResourceExhausted")
        || err_str.contains("429")
        || err_str.contains("RESOURCE_EXHAUSTED")
}
```

## Listing Available Models

To discover correct model IDs:
```bash
curl -s "https://generativelanguage.googleapis.com/v1beta/models?key=$GEMINI_API_KEY" | \
  python3 -c "import sys,json; [print(m['name']) for m in json.load(sys.stdin)['models'] if 'generateContent' in m.get('supportedGenerationMethods',[])]"
```

## Relevant Files

- `src/services/summary.rs` — Text generation with streaming
- `src/services/embedding.rs` — Embedding computation
- `src/main.rs` — Model option configuration
- `src/state.rs` — `ModelOption` struct definition
