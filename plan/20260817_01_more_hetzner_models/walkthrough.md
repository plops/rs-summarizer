# Walkthrough: Additional Hetzner Inference Models Integration

**Client**: Wol Pumba (`wolpumba@gmail.com`)  
**Repository**: `plops/rs-summarizer`  
**Plan Directory**: `file:///workspace/src/rs-summarizer/plan/20260817_01_more_hetzner_models/`  
**Date**: August 17, 2026  

---

## 1. Summary of Changes

We expanded Hetzner model support in `rs-summarizer` from 1 model (`hetzner-qwen-3.6-35b`) to all 4 experimental models currently available on the Hetzner OpenAI-compatible inference API (`https://inference.hetzner.com/api/v1`):

| Internal Name | API Model Identifier | Architecture | Context Window | Price (in/out) | RPM / RPD |
|:---|:---|:---|:---|:---|:---|
| `hetzner-qwen-3.6-35b` | `Qwen/Qwen3.6-35B-A3B-FP8` | MoE (35B / 3B active) | 262,144 tokens | $0.00 / $0.00 | 60 / 14,400 |
| `hetzner-deepseek-v4-flash` | `DeepSeek-V4-Flash-0731` | MoE (304B / 13B active) | 512,000 tokens | $0.00 / $0.00 | 60 / 14,400 |
| `hetzner-glm-5.2` | `GLM-5.2-NVFP4` | MoE (744B / 40B active) | 512,000 tokens | $0.00 / $0.00 | 60 / 14,400 |
| `hetzner-kimi-k2.7-code` | `Kimi-K2.7-Code` | MoE (1T / 32B active) | 262,144 tokens | $0.00 / $0.00 | 60 / 14,400 |

### Key Deliverables Completed:
1. **Dynamic Model Name Resolution Map**: Replaced the hardcoded Qwen fallback in `generate_summary_hetzner()` with `resolve_hetzner_model_name()` in [src/services/summary.rs](file:///workspace/src/rs-summarizer/src/services/summary.rs). Direct/unknown names pass through cleanly for forward compatibility.
2. **Model Registry Registration**: Added all three new models to `get_default_models()` in [src/state.rs](file:///workspace/src/rs-summarizer/src/state.rs) and [config/models.json](file:///workspace/src/rs-summarizer/config/models.json) with accurate context windows and experimental free pricing.
3. **Fallback Chains**: Added dedicated fallback chains in [src/tasks.rs](file:///workspace/src/rs-summarizer/src/tasks.rs) for each Hetzner model (prioritizing same-provider Hetzner models before falling back to Gemini). Extended all existing Gemini fallback chains with all 4 Hetzner models as last-resort fallbacks.
4. **Comprehensive Test Suite**: Added unit tests across `summary.rs`, `state.rs`, and `tasks.rs` validating name resolution, context windows, model registration, fallback order, and cross-chain consistency.

---

## 2. Files Modified

| File | Changes |
|:---|:---|
| [src/services/summary.rs](file:///workspace/src/rs-summarizer/src/services/summary.rs) | Added `resolve_hetzner_model_name()` function, replaced hardcoded fallback in `generate_summary_hetzner()`, and added `test_resolve_hetzner_model_name`. |
| [src/state.rs](file:///workspace/src/rs-summarizer/src/state.rs) | Registered `hetzner-deepseek-v4-flash`, `hetzner-glm-5.2`, and `hetzner-kimi-k2.7-code` in `get_default_models()`, synced baseline pricing with `config/models.json`, and added `test_all_hetzner_models_registered` and `test_hetzner_context_windows`. |
| [config/models.json](file:///workspace/src/rs-summarizer/config/models.json) | Appended JSON definitions for the 3 new Hetzner models. |
| [src/tasks.rs](file:///workspace/src/rs-summarizer/src/tasks.rs) | Added fallback chains for each new Hetzner model, updated existing Hetzner and Gemini fallback chains, and added chain unit tests. |

---

## 3. Design Decisions

1. **Provider-Level Model Name Mapping (`resolve_hetzner_model_name`)**:
   Instead of modifying `ModelOption` structs to hold both internal and external names across the entire codebase, internal identifiers (`hetzner-*`) are resolved cleanly inside `generate_summary_hetzner()`. Unknown or direct model identifiers containing slashes pass through without alteration, ensuring forward compatibility.

2. **Same-Provider Fallback Priority**:
   When a user selects a Hetzner model and it reaches rate limits or encounters transient issues, the fallback sequence tries other Hetzner models first (e.g. DeepSeek → GLM → Kimi → Qwen) before transitioning to Gemini Flash models. This keeps cost ($0.00) and privacy/provider expectations consistent.

3. **Gemini Fallback Integration**:
   Gemini models now include all 4 Hetzner models at the tail of their fallback chain as free last-resort providers when all Gemini daily quotas are exhausted.

4. **Shared Rate Limiting Deferred**:
   Hetzner's rate limit is 3M input tokens per 60s across the API key. Since per-model 14,400 RPD and 60 RPM limits are set, individual counters are currently used without introducing complex global multi-model bucket locks.

---

## 4. Test Results

- **Unit & Integration Tests**: 132 tests passing (`cargo test`)
- **Code Formatting**: 100% compliant (`cargo fmt --check`)
- **Linter Checks**: Passed with zero warnings (`cargo clippy -- -D warnings`)
- **Build Verification**: Clean compilation (`cargo build`)

```text
test state::model_checks::test_all_hetzner_models_registered ... ok
test state::model_checks::test_hetzner_context_windows ... ok
test state::model_checks::test_load_models_config ... ok
test state::model_checks::test_model_pricing_is_valid ... ok
test state::model_checks::test_unique_model_names ... ok
test state::model_checks::test_updated_model_limits ... ok
test services::summary::tests::test_resolve_hetzner_model_name ... ok
test tasks::tests::test_fallback_chain_hetzner_deepseek ... ok
test tasks::tests::test_fallback_chain_hetzner_glm ... ok
test tasks::tests::test_fallback_chain_hetzner_kimi ... ok
test tasks::tests::test_gemini_chains_include_all_hetzner_models ... ok
test tasks::tests::test_get_fallback_chain_new_models ... ok

test result: ok. 132 passed; 0 failed; 41 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

---

## 5. Technical Learnings

1. **Model Price Synchronization**:
   `get_default_models()` in `src/state.rs` must stay strictly synchronized with `config/models.json` to satisfy `test_load_models_config()`. Keeping default definitions and configuration files synchronized prevents divergence during runtime fallback scenarios.
2. **Context Window Scale**:
   Both `DeepSeek-V4-Flash-0731` and `GLM-5.2-NVFP4` support a 512,000 token context window, doubling `Qwen3.6-35B`'s 262k window and making them ideal candidates for extremely long audio/video transcripts.

---

## 6. Future Extensions

1. **Context-Window-Aware Auto-Routing**:
   For transcripts exceeding 200,000 words, `rs-summarizer`'s auto-routing logic in `tasks.rs` could automatically select `hetzner-deepseek-v4-flash` or `hetzner-glm-5.2` to leverage their 512k context windows.
2. **Code & Technical Content Routing**:
   If incoming transcripts or Hacker News submissions are detected as programming-focused (e.g. GitHub URLs or technical documentation), routing could prefer `hetzner-kimi-k2.7-code`.
3. **Shared Provider Token Bucket Rate Limiter**:
   Implement a unified token bucket rate limiter tracking token consumption across all Hetzner models under the single API key.

---

## 7. Docker Container Requirements

No new system libraries, packages, or background binaries are required. The existing Docker configuration with standard OpenSSL/CA-certificates handles all HTTPS connections to `https://inference.hetzner.com/api/v1`.
