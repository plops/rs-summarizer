# Walkthrough: Hetzner AI API Integration

**Client**: Wol Pumba (`wolpumba@gmail.com`)  
**Repository**: `plops/rs-summarizer`  
**Walkthrough File**: `file:///workspace/src/rs-summarizer/plan/20260726_01_hetzner/walkthrough.md`  
**Date**: July 26, 2026  

---

## 1. Executive Summary of Implementation

We have successfully integrated Hetzner’s experimental OpenAI-compatible LLM inference API (`https://inference.hetzner.com/api/v1`) hosting `Qwen/Qwen3.6-35B-A3B-FP8` into `rs-summarizer`. 

The architecture was designed to be modular, maintainable, and extensible for future OpenAI-compatible LLM providers.

### Key Deliverables Completed
1. **Extensible Architecture**: Extended `ModelArchitecture` enum in `src/state.rs` with the `Hetzner` variant and updated model configuration handlers.
2. **Model Entry**: Registered `hetzner-qwen-3.6-35b` (`Qwen/Qwen3.6-35B-A3B-FP8`) in `config/models.json` and default fallback list in `src/state.rs` with 262K context window and 0.0 USD cost per million tokens.
3. **OpenAI-Compatible Streaming Provider**: Updated `SummaryService` in `src/services/summary.rs` to stream responses progressively to SQLite via `async-openai` crate.
4. **Environment Configuration**: Supported `HETZNER_API_KEY` (default: `2jwqK0zWB54O0ipIzRtmv9jHme7jSazg`) and `HETZNER_BASE_URL` (default: `https://inference.hetzner.com/api/v1`).
5. **Fallback Chains**: Added `hetzner-qwen-3.6-35b` into background pipeline fallback chains (`src/tasks.rs`).
6. **UI Scoping**: Preserved Gemini-only feature scoping in `templates/index.html` (automatically hiding Google Search Grounding and URL Context when Hetzner model is selected).
7. **Documentation & Dependencies**: Created `deps.md`, `plan.md`, `task.md`, and recorded crate `async-openai = "0.41.1"` under GitHub organization `64bit/async-openai`.
8. **Git Commit**: Committed all changes with Conventional Commit format and detailed description (`a25e1df`).

---

## 2. Test-Driven Findings & Technical Discoveries

During implementation and compiler checks, two noteworthy technical details were identified:

1. **`async-openai` Feature Gating**:
   In `async-openai` version `0.41.1`, builder types (`CreateChatCompletionRequestArgs`, `ChatCompletionRequestSystemMessageArgs`, `ChatCompletionRequestUserMessageArgs`) are gated behind the `chat-completion` feature flag. We explicitly configured `async-openai = { version = "0.41.1", features = ["chat-completion"] }` in `Cargo.toml`.
2. **Rust Pattern Exhaustiveness**:
   Adding `ModelArchitecture::Hetzner` triggered Rust compiler exhaustiveness checks on pattern matching. This ensured every prompt-building and feature-scoping match block across the service layer explicitly handles Hetzner model architecture.

---

## 3. Recommended Container Dependencies for Docker

To run `rs-summarizer` inside an Ubuntu-based Docker container with full HTTPS support for Hetzner inference endpoints, ensure the following OS packages are included in your `Dockerfile`:

```dockerfile
# Required OS packages for HTTPS/SSL connections to Hetzner API
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl-dev \
    curl \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*
```

---

## 4. Production Systemd & `.env` Configuration

To securely supply `HETZNER_API_KEY` to the `rs-summarizer` systemd service on boot, place the key in the `.env` file referenced by `rs-summarizer.service` (`EnvironmentFile=/home/kiel/host/.env`):

```env
GEMINI_API_KEY=your_gemini_api_key
HETZNER_API_KEY=your_hetzner_api_key
HETZNER_BASE_URL=https://inference.hetzner.com/api/v1
```

Set secure file permissions:
```bash
chmod 600 /home/kiel/host/.env
```

Restart systemd service:
```bash
sudo systemctl daemon-reload
sudo systemctl restart rs-summarizer.service
```

---

## 5. Verification Results


All automated test suites were run and passed without error:

- **Unit Tests**: 115 passing tests (`cargo test`)
- **Code Format**: Formatted cleanly (`cargo fmt`)
- **Linter Checks**: Passed without errors (`cargo clippy`)

```bash
test result: ok. 115 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

---

## 5. Learnings & Future Extensions

- **Pluggable LLM Providers**: The newly implemented provider dispatcher pattern in `SummaryService` allows adding other OpenAI-compatible backends (e.g. Scaleway, DeepSeek, or local Ollama/vLLM endpoints) by simply registering a new `ModelOption` and configuring base URL environment variables.
- **Reasoning Control**: In future updates, the `chat_template_kwargs` parameter (`enable_thinking: false`) can be configured per request to further fine-tune output generation latency.
