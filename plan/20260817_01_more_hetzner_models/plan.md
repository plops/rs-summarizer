# Implementation Plan: Additional Hetzner Inference Models

**Client**: Wol Pumba (`wolpumba@gmail.com`)
**Project**: `plops/rs-summarizer`
**Plan Directory**: `file:///workspace/src/rs-summarizer/plan/20260817_01_more_hetzner_models/`

---

## 1. Overview & Motivation

Hetzner has expanded their experimental inference API from 1 model to 4 models. The current codebase only supports `Qwen/Qwen3.6-35B-A3B-FP8` (registered as `hetzner-qwen-3.6-35b`). We need to add the 3 new models while maintaining the clean, extensible architecture established in the previous Hetzner integration (`plan/20260726_01_hetzner/`).

The key architectural issue to fix: `generate_summary_hetzner()` in `src/services/summary.rs` has a **hardcoded fallback** that always maps unknown model names to `Qwen/Qwen3.6-35B-A3B-FP8`, which would break when selecting any of the new models.

---

## 2. New Models Specification

All models use the same Hetzner OpenAI-compatible API endpoint.

| Internal Name | Hetzner API Model ID | Type | Context Window | Modalities | Price |
|:---|:---|:---|:---|:---|:---|
| `hetzner-qwen-3.6-35b` *(existing)* | `Qwen/Qwen3.6-35B-A3B-FP8` | MoE, 35B total / 3B active | 262,144 tokens | Text, Image | Free |
| `hetzner-deepseek-v4-flash` *(new)* | `DeepSeek-V4-Flash-0731` | MoE, 304B total / 13B active | 512,000 tokens | Text | Free |
| `hetzner-glm-5.2` *(new)* | `GLM-5.2-NVFP4` | MoE, 744B total / 40B active | 512,000 tokens | Text | Free |
| `hetzner-kimi-k2.7-code` *(new)* | `Kimi-K2.7-Code` | MoE, 1T total / 32B active | 262,144 tokens | Text, Image | Free |

### Hetzner API Configuration
- **Base URL**: `https://inference.hetzner.com/api/v1`
- **API Key**: `2jwqK0zWB54O0ipIzRtmv9jHme7jSazg` (env: `HETZNER_API_KEY`)
- **Rate Limits** (per API key, shared across all models):
  - 60s window: 3M input tokens, 60k output tokens
  - 24h window: 500M input tokens, 5M output tokens

---

## 3. Architecture Changes

### 3.1 Model Name Resolution (Critical Fix)

**Current state** (`src/services/summary.rs` L450-454):
```rust
let actual_model_name = if model.name.contains('/') {
    model.name.clone()
} else {
    "Qwen/Qwen3.6-35B-A3B-FP8".to_string()  // BUG: hardcoded fallback
};
```

**Target state**: Replace with a dedicated mapping function:
```rust
fn resolve_hetzner_model_name(internal_name: &str) -> &str {
    match internal_name {
        "hetzner-qwen-3.6-35b" => "Qwen/Qwen3.6-35B-A3B-FP8",
        "hetzner-deepseek-v4-flash" => "DeepSeek-V4-Flash-0731",
        "hetzner-glm-5.2" => "GLM-5.2-NVFP4",
        "hetzner-kimi-k2.7-code" => "Kimi-K2.7-Code",
        other => other, // pass-through for names containing '/' or unknown
    }
}
```

### 3.2 Model Registry Updates

Add 3 new `ModelOption` entries to:
1. `src/state.rs` → `get_default_models()` function
2. `config/models.json` → JSON array

All new models use:
- `architecture: Hetzner`
- `input_price_per_mtoken: 0.0` / `output_price_per_mtoken: 0.0` (free experimental)
- `rpm_limit: 60` / `rpd_limit: 14400` (matching existing Hetzner model)
- Context windows as per Hetzner spec

### 3.3 Fallback Chain Updates

New fallback chains in `get_fallback_chain()` in `src/tasks.rs`:

- **Hetzner models** should fall back to other Hetzner models first (same-provider preference), then Gemini models
- **Existing Gemini chains** should include new Hetzner models as additional last-resort fallbacks

Example new chain:
```
hetzner-deepseek-v4-flash → hetzner-glm-5.2 → hetzner-kimi-k2.7-code → hetzner-qwen-3.6-35b → gemini-3.7-flash → gemini-3.6-flash → gemini-3.5-flash-lite
```

### 3.4 Shared Rate Limits Consideration

Hetzner enforces rate limits **per API key** (not per model). The current per-model RPM/RPD counters in `AppState::model_counts` don't reflect this shared budget. However, since:
- The experimental API is free and generous (3M input tokens/60s)
- Individual model RPD limits of 14400 are unlikely to cause issues
- Adding a shared rate limiter would be a larger refactor

**Recommendation**: Keep individual per-model RPD/RPM tracking as-is. Document this as a known limitation and potential future enhancement.

---

## 4. Autonomous AI Agent File Context Map

An agent implementing this plan should inspect these files:

| File Path | Description & Relevance |
|:---|:---|
| [prompt.txt](file:///workspace/src/rs-summarizer/plan/20260817_01_more_hetzner_models/prompt.txt) | Original requirements from the client. |
| [models.md](file:///workspace/src/rs-summarizer/plan/20260817_01_more_hetzner_models/models.md) | New Hetzner model specifications and API examples. |
| [hetzner_config.md](file:///workspace/src/hetzner_config.md) | Hetzner API credentials, rate limits, and endpoint documentation. |
| [state.rs](file:///workspace/src/rs-summarizer/src/state.rs) | `ModelArchitecture` enum, `ModelOption` struct, `get_default_models()`, `load_models_config()`. Core file for registering new models. |
| [config/models.json](file:///workspace/src/rs-summarizer/config/models.json) | External JSON model configuration loaded at startup. |
| [services/summary.rs](file:///workspace/src/rs-summarizer/src/services/summary.rs) | `generate_summary_hetzner()` with hardcoded model name resolution that needs fixing. Prompt building and streaming logic. |
| [tasks.rs](file:///workspace/src/rs-summarizer/src/tasks.rs) | `get_fallback_chain()` definitions (L74-161), `run_model_pipeline`, RPM/RPD enforcement, auto-routing heuristics. |
| [routes/mod.rs](file:///workspace/src/rs-summarizer/src/routes/mod.rs) | Web route handlers. Model dropdown population logic. |
| [templates/index.html](file:///workspace/src/rs-summarizer/templates/index.html) | Frontend model dropdown with `data-architecture` attributes. New models auto-appear via template loop. |
| [Cargo.toml](file:///workspace/src/rs-summarizer/Cargo.toml) | Dependencies. No new crates needed — `async-openai` v0.41.1 already supports all OpenAI-compatible endpoints. |
| [plan/20260726_01_hetzner/](file:///workspace/src/rs-summarizer/plan/20260726_01_hetzner/) | Previous Hetzner integration plan, walkthrough, and deps reference. |
| [tests/](file:///workspace/src/rs-summarizer/tests/) | Integration test patterns (`integration_pipeline.rs`, `integration_browser.rs`). |

---

## 5. Requirements Assessment & Recommendations

### Fulfilled Requirements
- [x] Add new Hetzner models from `models.md`
- [x] Maintainable, extensible architecture (reuse existing `ModelArchitecture::Hetzner` dispatch)
- [x] Track dependencies in `deps.md`
- [x] Conventional Commit messages
- [x] Serial task breakdown (`task.md`)
- [x] Unit tests and integration tests
- [x] Post-implementation walkthrough (`walkthrough.md`)

### Recommended Additional Enhancements
1. **Context-Window-Aware Auto-Routing**: DeepSeek-V4-Flash and GLM-5.2 have 512k context windows (vs 262k for Qwen). For very long transcripts (>200k words), consider auto-routing to these larger-context models.
2. **Code-Specific Routing**: Kimi-K2.7-Code is a code-specialized model. If content is detected as code-heavy (e.g., GitHub README transcripts), it could be preferred.
3. **Quality Tiering**: GLM-5.2 has the highest total parameter count (744B) and may produce the highest quality summaries. Consider it as the premium Hetzner option.
4. **Shared Hetzner Rate Limiter**: Future enhancement to enforce per-API-key token budgets across all Hetzner models collectively.

---

## 6. Git Commit Message Guidelines

All commits **must** follow **Conventional Commit** format:

```
<type>(<scope>): <short description>

<detailed body explaining rationale, design decisions, and testing>
```

### Types
- `feat`: New feature (model registration, name resolution, fallback chains)
- `test`: New or updated tests
- `chore`: Formatting, linting, dependency updates
- `docs`: Documentation (plan.md, task.md, deps.md, walkthrough.md)

### Example
```
feat(llm): add Hetzner model name resolution map for multi-model support

Replace hardcoded Qwen fallback in generate_summary_hetzner() with a
dedicated resolve_hetzner_model_name() function that maps internal model
names to their Hetzner API identifiers.

Supports: hetzner-qwen-3.6-35b, hetzner-deepseek-v4-flash,
hetzner-glm-5.2, hetzner-kimi-k2.7-code.

Includes unit tests for all mappings and unknown-name pass-through.
```

---

## 7. Testing & Verification Strategy

### Unit Tests to Add
| Test Name | Location | Description |
|:---|:---|:---|
| `test_resolve_hetzner_model_name` | `src/services/summary.rs` | Verify all 4 model name mappings + unknown pass-through |
| `test_hetzner_models_registered` | `src/state.rs` | Verify all 4 Hetzner models exist in `get_default_models()` |
| `test_hetzner_model_cost_zero` | `src/services/summary.rs` | Verify all new Hetzner models have $0.00 pricing |
| `test_fallback_chain_hetzner_*` | `src/tasks.rs` | Verify fallback chains for each new Hetzner model |

### Existing Tests to Verify (No Regression)
- `test_unique_model_names` — must pass with new model names
- `test_model_pricing_is_valid` — must pass with 0.0 pricing
- `test_updated_model_limits` — existing Hetzner model checks still valid
- `test_load_models_config` — JSON config must match defaults
- All fallback chain tests — existing chains must not break

### Validation Commands
```bash
cargo fmt        # Format code
cargo clippy     # Lint check
cargo build      # Compile check
cargo test       # Full test suite
```
