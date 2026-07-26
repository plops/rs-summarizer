# Implementation Plan: Hetzner AI API & Extensible LLM Provider Integration

**Client**: Wol Pumba (`wolpumba@gmail.com`)  
**Project**: `plops/rs-summarizer`  
**Plan Directory**: `file:///workspace/src/rs-summarizer/plan/20260726_01_hetzner/`  

---

## 1. Overview & Architecture Design

Hetzner has launched an experimental, OpenAI-compatible LLM inference API (`https://inference.hetzner.com/api/v1`) hosting `Qwen/Qwen3.6-35B-A3B-FP8`. This model features 35 billion total parameters (3B active MoE parameters), a 262,144 token context window, FP8 quantization, and fast time-to-first-token benchmarks (~153 ms).

### Maintainable & Extensible Provider Architecture
Currently, `SummaryService` in `src/services/summary.rs` is tightly coupled with the `gemini-rust` crate for Google Gemini API calls. To make the architecture maintainable and extensible for Hetzner and future OpenAI-compatible or custom providers (e.g. Scaleway, DeepSeek, local Ollama):

1. **`ModelArchitecture` Enum Extension**:
   Expand `ModelArchitecture` in `src/state.rs` with a `Hetzner` variant (and `OpenAI` as appropriate) to represent OpenAI-compatible inference backends.
2. **Provider Dispatcher / Dispatch Logic**:
   Modularize `SummaryService::generate_summary` so it dispatches summary requests based on `model.architecture`:
   - `ModelArchitecture::Gemini` / `ModelArchitecture::Gemma` -> Calls Gemini API via `gemini-rust`.
   - `ModelArchitecture::Hetzner` -> Calls Hetzner API via `async-openai` (OpenAI-compatible REST client).
3. **Environment & Model Configuration**:
   - Environment variables: `HETZNER_API_KEY` (defaulting to key `2jwqK0zWB54O0ipIzRtmv9jHme7jSazg`) and `HETZNER_BASE_URL` (defaulting to `https://inference.hetzner.com/api/v1`).
   - JSON Model Entry: Add `hetzner-qwen-3.6-35b` to `config/models.json` and `src/state.rs` default models.

---

## 2. Requirements Assessment & Recommendations

### User Requirements Checklist
- [x] Read `/workspace/src/hetzner_config.md` and integrate Hetzner model.
- [x] Maintainable, extensible architecture.
- [x] Clean code formatting & rust tooling.
- [x] Track dependencies in `deps.md` with GitHub organization (`645068db/async-openai`).
- [x] Ubuntu Docker environment compatibility.
- [x] File context map for autonomous AI agent context building.
- [x] Conventional Commit message rules with comprehensive descriptions.
- [x] Serial task breakdown (`task.md`).
- [x] Unit tests and Integration tests.
- [x] Post-implementation Walkthrough (`walkthrough.md`).

### Recommended Additional Enhancements
1. **Thinking / Reasoning Control Parameter**:
   Hetzner's Qwen model supports an `enable_thinking` flag in requests. For standard summarization tasks, setting `enable_thinking: false` preserves output token budgets and reduces latency. We implement prompt formatting tailored to Qwen/Hetzner models.
2. **UI Feature Visibility Scoping**:
   Ensure `google_search_grounding` and `url_context` checkboxes in `templates/index.html` are hidden when a user selects Hetzner models, as these features are Google Gemini-specific.
3. **Fallback Chain Integration**:
   Include `hetzner-qwen-3.6-35b` in the rate-limiter fallback chain so that if Gemini quotas are exhausted, requests can seamlessly failover to Hetzner.
4. **Token Usage Estimation & Cost Calculation**:
   Calculate exact or estimated input/output token counts and report cost (0.0 USD for unbilled experimental platform).

---

## 3. Autonomous AI Agent File Context Map

An autonomous AI agent tasked with working on this implementation should inspect the following key codebase files:

| File Path | Description & Relevance |
| :--- | :--- |
| [hetzner_config.md](file:///workspace/src/hetzner_config.md) | Technical specifications of Hetzner API, token, base URL, model name `Qwen/Qwen3.6-35B-A3B-FP8`, and sample request payloads. |
| [Cargo.toml](file:///workspace/src/rs-summarizer/Cargo.toml) | Package manifest containing dependencies (`async-openai`, `gemini-rust`, `reqwest`, `tokio`). |
| [src/state.rs](file:///workspace/src/rs-summarizer/src/state.rs#L8-L34) | Defines `ModelArchitecture`, `ModelOption`, `AppState`, default model list, and config loader. |
| [config/models.json](file:///workspace/src/rs-summarizer/config/models.json) | External JSON file containing default model configurations loaded at startup. |
| [src/services/summary.rs](file:///workspace/src/rs-summarizer/src/services/summary.rs) | Core summarization service executing API calls, streaming chunks, prompt construction, and cost calculation. |
| [src/tasks.rs](file:///workspace/src/rs-summarizer/src/tasks.rs#L275-L360) | Background task orchestration (`run_model_pipeline`), rate limit checks, RPM delays, and fallback chains. |
| [src/routes/mod.rs](file:///workspace/src/rs-summarizer/src/routes/mod.rs#L50-L120) | Web route handlers (`index`, `process_transcript`) and form validation logic. |
| [templates/index.html](file:///workspace/src/rs-summarizer/templates/index.html#L19-L50) | Frontend template rendering model `<select>` dropdown and feature options toggle JS logic. |
| [src/models.rs](file:///workspace/src/rs-summarizer/src/models.rs#L1-L45) | Data models for summaries, form inputs, and ratings. |

---

## 4. Git Commit Message Guidelines

All git commits **must** adhere strictly to the **Conventional Commit** specification. Every commit must have:
1. A concise, structured title in the format `<type>(<scope>): <short description>`.
2. A blank line after the title.
3. A detailed, multi-line body providing comprehensive context, rationale, design decisions, and testing verification.

### Conventional Commit Types
- `feat`: A new feature or provider implementation.
- `fix`: Bug fix in model processing, API serialization, or UI logic.
- `refactor`: Restructuring code without changing functionality (e.g. abstracting LLM providers).
- `docs`: Documentation updates (`plan.md`, `task.md`, `deps.md`, `walkthrough.md`).
- `test`: Adding or updating unit/integration tests.
- `chore`: Dependency updates (`Cargo.toml`) or build script modifications.

### Example Commit Format

```gitcommit
feat(llm): integrate Hetzner experimental OpenAI-compatible inference API

Add support for Hetzner's hosted Qwen/Qwen3.6-35B-A3B-FP8 model via an OpenAI-compatible REST endpoint.

- Extend `ModelArchitecture` enum with `Hetzner` variant in `src/state.rs`.
- Add `hetzner-qwen-3.6-35b` option to `config/models.json` and default models list.
- Update `SummaryService::generate_summary` to stream chat completions via `async-openai` when a Hetzner model is selected.
- Read `HETZNER_API_KEY` and `HETZNER_BASE_URL` from environment with sensible fallbacks.
- Update index template JavaScript to scope Gemini-only options when Hetzner is selected.

Tested with unit tests for prompt formatting, model parsing, and mock provider streaming.
```

---

## 5. Testing & Verification Strategy

1. **Unit Testing**:
   - `test_hetzner_model_option_parsing`: Verify `hetzner-qwen-3.6-35b` parses correctly from JSON and default models.
   - `test_hetzner_prompt_formatting`: Verify prompt formatting for Hetzner / Qwen models.
   - `test_hetzner_cost_calculation`: Verify 0.0 pricing for experimental Hetzner API.
2. **Integration & Compilation Testing**:
   - Execute `cargo check` and `cargo test`.
   - Verify all existing 113+ unit tests continue to pass without regression.
   - Run `cargo fmt --check` and `cargo clippy`.
