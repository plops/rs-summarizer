# Implementation Plan: Hetzner Inference Model Reduction & Qwen 3.8 Integration

**Client**: Wol Pumba (`wolpumba@gmail.com`)  
**Project**: `plops/rs-summarizer`  
**Plan Directory**: `file:///workspace/src/rs-summarizer/plan/20260822_01_hetzner_reduce/`  
**Target Release**: `v1.6.0`  

---

## 1. Overview & Context

Hetzner has updated its experimental OpenAI-compatible inference API (`https://inference.hetzner.com/api/v1`). The model catalog has been reduced from four models down to two:

```json
{
  "object": "list",
  "data": [
    {
      "id": "Qwen/Qwen3.6-35B-A3B-FP8",
      "object": "model",
      "created": 1787406513,
      "owned_by": "hetzner",
      "root": "/model",
      "parent": null,
      "max_model_len": 262144
    },
    {
      "id": "Qwen3.8-27B",
      "object": "model",
      "created": 1787406513,
      "owned_by": "hetzner",
      "root": "/model",
      "parent": null,
      "max_model_len": 262144
    }
  ]
}
```

### Changes in Model Availability:
1. **Retained Model**: `Qwen/Qwen3.6-35B-A3B-FP8` (internal: `hetzner-qwen-3.6-35b`) — 262,144 context window.
2. **Newly Added Model**: `Qwen3.8-27B` (internal: `hetzner-qwen-3.8-27b`) — 262,144 context window, free tier ($0.00 / $0.00), 60 RPM / 14,400 RPD limits.
3. **Decommissioned Models**:
   - `hetzner-deepseek-v4-flash` (`DeepSeek-V4-Flash-0731`)
   - `hetzner-glm-5.2` (`GLM-5.2-NVFP4`)
   - `hetzner-kimi-k2.7-code` (`Kimi-K2.7-Code`)

---

## 2. Requirements Assessment & Proposals

### User Requirements Checklist
- [x] Adapt code and configuration to Hetzner's current model offering (`Qwen/Qwen3.6-35B-A3B-FP8` and `Qwen3.8-27B`).
- [x] Remove deprecated models that Hetzner no longer provides.
- [x] Maintain clean, idiomatic code formatting and Rust tooling (`cargo fmt`, `cargo clippy`).
- [x] Document dependencies in `deps.md` with GitHub organization.
- [x] Provide an autonomous AI agent context map.
- [x] Specify comprehensive Conventional Commit message rules.
- [x] Define a serial task plan in `task.md`.
- [x] Introduce and execute unit and integration tests.
- [x] Prepare a new release (`v1.6.0`) including version bump, tag, and package metadata.
- [x] Write post-implementation walkthrough in `walkthrough.md`.

### Recommended Architecture Proposals & Enhancements
1. **Seamless Backward & Forward Model Mapping**:
   In `resolve_hetzner_model_name()` (`src/services/summary.rs`), map `hetzner-qwen-3.8-27b` to `"Qwen3.8-27B"` and `hetzner-qwen-3.6-35b` to `"Qwen/Qwen3.6-35B-A3B-FP8"`. Maintain the catch-all `other => other` to permit direct pass-through of upstream model IDs without breaking.
2. **Synchronized Default Models & Config**:
   Keep `config/models.json` and `get_default_models()` in `src/state.rs` in exact parity to ensure zero discrepancy between static fallbacks and runtime file configurations.
3. **Streamlined Fallback Chains**:
   Update `get_fallback_chain()` in `src/tasks.rs`:
   - `hetzner-qwen-3.6-35b` falls back to `hetzner-qwen-3.8-27b`, then Gemini models.
   - `hetzner-qwen-3.8-27b` falls back to `hetzner-qwen-3.6-35b`, then Gemini models.
   - All Gemini chains fall back to `hetzner-qwen-3.8-27b` and `hetzner-qwen-3.6-35b` as free-tier options.

---

## 3. Autonomous AI Agent File Context Map

An autonomous agent working on this codebase should review the following core files:

| File Path | Description & Relevance |
| :--- | :--- |
| [config/models.json](file:///workspace/src/rs-summarizer/config/models.json) | External model definitions loaded at startup. Contains model limits, context windows, pricing, and architecture. |
| [src/state.rs](file:///workspace/src/rs-summarizer/src/state.rs) | Hardcoded default model configurations (`get_default_models()`), `ModelArchitecture` enum, and registry tests. |
| [src/services/summary.rs](file:///workspace/src/rs-summarizer/src/services/summary.rs) | `resolve_hetzner_model_name()` identifier mapping and Hetzner OpenAI client streaming handler. |
| [src/tasks.rs](file:///workspace/src/rs-summarizer/src/tasks.rs) | Background task orchestration, rate limit fallback chains (`get_fallback_chain()`), and rate limiting tests. |
| [Cargo.toml](file:///workspace/src/rs-summarizer/Cargo.toml) | Package manifest version (`1.5.0` → `1.6.0`) and dependencies. |
| [Cargo.lock](file:///workspace/src/rs-summarizer/Cargo.lock) | Lockfile tracking exact package versions. |
| [RELEASE_README.md](file:///workspace/src/rs-summarizer/RELEASE_README.md) | User-facing installation and deployment guide bundled with release packages. |

---

## 4. Git Commit Message Guidelines

All commits must follow the **Conventional Commit** format with structured multi-line descriptions:

### Format
```text
<type>(<scope>): <short description>

<detailed description explaining why the change was made, what files were affected, and how it was verified>
```

### Allowed Types
- `feat`: Introduction of new features or models (e.g. `feat(models): ...`, `feat(routing): ...`).
- `fix`: Bug fixes or adjustments to existing logic.
- `chore`: Version bumps, dependency updates, lint/format fixes.
- `docs`: Documentation updates (`deps.md`, `plan.md`, `task.md`, `walkthrough.md`).
- `test`: Adding or adjusting unit and integration tests.

### Example Commit Format
```text
feat(models): register Hetzner Qwen 3.8 27B and retire deprecated models

Update Hetzner model catalog in config/models.json and src/state.rs to reflect
the reduced upstream availability:
- Register `hetzner-qwen-3.8-27b` (Qwen3.8-27B) with 262,144 token context window
- Remove decommissioned models: hetzner-deepseek-v4-flash, hetzner-glm-5.2, hetzner-kimi-k2.7-code
- Update unit tests in src/state.rs for model registration and context limits

Tested with cargo test state::model_checks.
```

---

## 5. Sequential Implementation Strategy

1. **Model Registry Updates**:
   - Update `config/models.json` with `hetzner-qwen-3.8-27b` and remove decommissioned models.
   - Update `src/state.rs` `get_default_models()` and associated model unit tests.
2. **Model Name Resolution**:
   - Update `resolve_hetzner_model_name()` in `src/services/summary.rs` to map `"hetzner-qwen-3.8-27b"` to `"Qwen3.8-27B"`.
   - Update unit tests in `src/services/summary.rs`.
3. **Fallback Chains**:
   - Update `get_fallback_chain()` in `src/tasks.rs` to cross-fallback between Qwen 3.8 and Qwen 3.6, and update Gemini fallback chains.
   - Update fallback chain unit tests in `src/tasks.rs`.
4. **Tooling & Validation**:
   - Run `cargo fmt --check` and `cargo clippy -- -D warnings`.
   - Run full test suite with `cargo test`.
5. **Release Preparation (v1.6.0)**:
   - Bump version to `1.6.0` in `Cargo.toml` and run `cargo check` to update `Cargo.lock`.
   - Create release commit and git tag `v1.6.0`.
6. **Documentation & Walkthrough**:
   - Write `walkthrough.md` summarizing changes, testing, learnings, and container requirements.

---

## 6. Testing Strategy

- **State Tests**: `test_all_hetzner_models_registered`, `test_hetzner_context_windows`, `test_load_models_config`.
- **Summary Tests**: `test_resolve_hetzner_model_name`, `test_hetzner_model_cost_calculation`, `test_hetzner_model_architecture_as_str`.
- **Task Fallback Tests**: `test_get_fallback_chain_new_models`, `test_fallback_chain_hetzner_qwen38`, `test_gemini_chains_include_all_hetzner_models`.
- **Full Suite**: Execute all unit and integration tests across the crate (`cargo test`).
