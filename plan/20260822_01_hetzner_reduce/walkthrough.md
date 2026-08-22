# Walkthrough: Hetzner Inference Model Reduction & Qwen 3.8 Integration

**Client**: Wol Pumba (`wolpumba@gmail.com`)  
**Repository**: `plops/rs-summarizer`  
**Plan Directory**: `file:///workspace/src/rs-summarizer/plan/20260822_01_hetzner_reduce/`  
**Release**: `v1.6.0`  
**Date**: August 22, 2026  

---

## 1. Summary of Changes

Upstream changes at Hetzner's OpenAI-compatible inference API (`https://inference.hetzner.com/api/v1`) reduced the active model offering to two models:

| Internal Name | API Identifier | Architecture | Context Window | Price (in/out) | RPM / RPD |
|:---|:---|:---|:---|:---|:---|
| `hetzner-qwen-3.6-35b` | `Qwen/Qwen3.6-35B-A3B-FP8` | MoE (35B total / 3B active) | 262,144 tokens | $0.00 / $0.00 | 60 / 14,400 |
| `hetzner-qwen-3.8-27b` *(new)* | `Qwen3.8-27B` | Dense / MoE (27B) | 262,144 tokens | $0.00 / $0.00 | 60 / 14,400 |

### Key Deliverables:
1. **Model Catalog Synchronization**:
   - Registered `hetzner-qwen-3.8-27b` in [src/state.rs](file:///workspace/src/rs-summarizer/src/state.rs) (`get_default_models()`) and [config/models.json](file:///workspace/src/rs-summarizer/config/models.json) with 262,144 tokens context length and free experimental pricing tier.
   - Cleanly removed decommissioned models: `hetzner-deepseek-v4-flash`, `hetzner-glm-5.2`, and `hetzner-kimi-k2.7-code`.
2. **Model Name Resolution**:
   - Updated `resolve_hetzner_model_name()` in [src/services/summary.rs](file:///workspace/src/rs-summarizer/src/services/summary.rs) to map `"hetzner-qwen-3.8-27b"` to `"Qwen3.8-27B"`.
   - Retained the `other => other` pass-through for direct API model identifiers (such as `"Qwen/Qwen3.6-35B-A3B-FP8"` and `"Qwen3.8-27B"`).
3. **Fallback Chains & Auto-Failover**:
   - Established mutual fallback between `hetzner-qwen-3.6-35b` and `hetzner-qwen-3.8-27b` in [src/tasks.rs](file:///workspace/src/rs-summarizer/src/tasks.rs) before escalating to Gemini models.
   - Updated all Gemini model fallback chains to append the active Hetzner models as last-resort free tier options.
4. **Code Quality & Linter Compliance**:
   - Fixed clippy lint `chunks-exact-to-as-chunks` using `as_chunks::<4>()` in [src/services/embedding.rs](file:///workspace/src/rs-summarizer/src/services/embedding.rs).
   - Applied `cargo fmt` across all modified files.
5. **Release Preparation & Tagging**:
   - Bumped crate version to `1.6.0` in [Cargo.toml](file:///workspace/src/rs-summarizer/Cargo.toml) and synchronized [Cargo.lock](file:///workspace/src/rs-summarizer/Cargo.lock).
   - Tagged release `v1.6.0`.

---

## 2. Files Modified

| File | Changes |
|:---|:---|
| [config/models.json](file:///workspace/src/rs-summarizer/config/models.json) | Replaced decommissioned models with `hetzner-qwen-3.8-27b`. |
| [src/state.rs](file:///workspace/src/rs-summarizer/src/state.rs) | Registered `hetzner-qwen-3.8-27b` in `get_default_models()`, removed old models, and updated unit tests (`test_all_hetzner_models_registered`, `test_hetzner_context_windows`). |
| [src/services/summary.rs](file:///workspace/src/rs-summarizer/src/services/summary.rs) | Updated `resolve_hetzner_model_name()` and updated unit tests for name resolution and pass-through. |
| [src/tasks.rs](file:///workspace/src/rs-summarizer/src/tasks.rs) | Updated fallback chains in `get_fallback_chain()` and updated chain verification unit tests. |
| [src/services/embedding.rs](file:///workspace/src/rs-summarizer/src/services/embedding.rs) | Updated `bytes_to_embedding` with `as_chunks::<4>()` for clippy compliance. |
| [Cargo.toml](file:///workspace/src/rs-summarizer/Cargo.toml) | Bumped version from `1.5.0` to `1.6.0`. |
| [Cargo.lock](file:///workspace/src/rs-summarizer/Cargo.lock) | Synchronized workspace package version. |
| [plan/20260822_01_hetzner_reduce/](file:///workspace/src/rs-summarizer/plan/20260822_01_hetzner_reduce/) | Added `deps.md`, `plan.md`, `task.md`, and `walkthrough.md`. |

---

## 3. Design Decisions

1. **Explicit Forward & Backward Compatibility in Model Resolution**:
   - `resolve_hetzner_model_name()` cleanly maps internal names (`hetzner-qwen-3.8-27b` and `hetzner-qwen-3.6-35b`) to API-side names.
   - Any unknown model or direct model identifier containing uppercase or slash characters passes through unchanged, allowing experimental models to be queried directly without requiring binary recompilation.
2. **Provider Affinity in Fallback Strategy**:
   - When a user selects a Hetzner model (e.g. `hetzner-qwen-3.8-27b`), rate-limiting or quota exhaustion first falls back to the sibling Hetzner model (`hetzner-qwen-3.6-35b`) before falling back to Google Gemini Flash models.
3. **Parity Between Hardcoded Defaults and JSON Configuration**:
   - `config/models.json` is mirrored 1:1 by `get_default_models()` in `src/state.rs`. This guarantees that unit tests (`test_load_models_config`) and fallback behavior remain deterministic across varied deployment environments.

---

## 4. Test Results & Verification

- **Unit & Integration Tests**: 131 tests passing (`cargo test`)
- **Code Formatting**: 100% compliant (`cargo fmt --check`)
- **Clippy Linting**: Passed with zero warnings under `-D warnings` (`cargo clippy -- -D warnings`)
- **Compilation Check**: Clean build (`cargo check`)

```text
test state::model_checks::test_all_hetzner_models_registered ... ok
test state::model_checks::test_hetzner_context_windows ... ok
test state::model_checks::test_load_models_config ... ok
test state::model_checks::test_load_models_config_fallback ... ok
test state::model_checks::test_model_pricing_is_valid ... ok
test state::model_checks::test_unique_model_names ... ok
test state::model_checks::test_updated_model_limits ... ok
test services::summary::tests::test_resolve_hetzner_model_name ... ok
test services::summary::tests::test_hetzner_model_cost_calculation ... ok
test services::summary::tests::test_hetzner_model_architecture_as_str ... ok
test tasks::tests::test_fallback_chain_hetzner_qwen38 ... ok
test tasks::tests::test_gemini_chains_include_all_hetzner_models ... ok
test tasks::tests::test_get_fallback_chain_new_models ... ok

test result: ok. 131 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
```

---

## 5. Technical Learnings

1. **Upstream Model Lifecycle**:
   Experimental inference endpoints can deprecate or rotate model architectures rapidly. Having an isolated provider translation layer (`resolve_hetzner_model_name`) and unified configuration table minimizes blast radius when upstream offerings change.
2. **Clippy 1.98 Linting Evolution**:
   The `chunks_exact_to_as_chunks` lint encourages using `.as_chunks::<N>()` instead of `.chunks_exact(N)` when slice chunk size is a compile-time constant, improving code clarity and compiler optimization.

---

## 6. Potential Future Extensions

1. **Dynamic Upstream Model Discovery**:
   Implement a background probe querying `GET https://inference.hetzner.com/api/v1/models` on startup to automatically discover newly introduced Hetzner inference models and adjust available dropdown options without code changes.
2. **Qwen 3.8 vs Qwen 3.6 Benchmark Evaluation**:
   Create an evaluation script comparing summarization quality, latency, and reasoning depth between `Qwen3.8-27B` and `Qwen3.6-35B-A3B-FP8`.

---

## 7. Docker Container Requirements

No new system packages, libraries, or background services need to be added to the Docker container. The existing Ubuntu-based Docker environment with `ca-certificates`, `openssl`, and standard build tooling fully supports all HTTPS operations for the updated Hetzner inference models.
