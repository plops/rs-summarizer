# Model Update Integration Plan

Integrating the new models and structured limits by centralizing model configuration in `src/state.rs` and updating prompt dispatching in `src/services/summary.rs`.

## User Review Required

> [!IMPORTANT]
> The model updates include new limits (RPM and RPD) and architectures (Gemini, Gemma, and Other) which will affect cost calculation, rate-limiting, and prompt-routing.
> 
> Key configurations updated:
> - **Gemma 4 26B**: RPM = 15, RPD = 1500, Architecture = Gemma, Input price = 0.07, Output price = 0.34, Context = 256K.
> - **Gemini 3.5 Flash**: RPM = 5, RPD = 20, Architecture = Gemini, Input price = 0.10, Output price = 0.40, Context = 1M.
> - **text-embedding-004** has been removed per request as embeddings are handled separately.
> - 15 other models as defined in `.kiro/specs/model-update/models_20250522.md` (excluding embeddings).

## Proposed Changes

### Configuration & State Component

#### [MODIFY] [state.rs](file:///home/kiel/stage/rs-summarizer/src/state.rs)
- Introduce the `ModelArchitecture` enum with variants `Gemini`, `Gemma`, and `Other`.
- Add `architecture: ModelArchitecture` field to `ModelOption`.
- Add `get_default_models() -> Vec<ModelOption>` function returning the centralized list of 16 configured models (excluding `text-embedding-004`).
- Add unit tests verifying uniqueness of model names, valid pricing range, and specific model configuration attributes (e.g., limits for `gemini-3.5-flash` and `gemma-4-26b-a4b-it`).

#### [MODIFY] [main.rs](file:///home/kiel/stage/rs-summarizer/src/main.rs)
- Replace the hardcoded `model_options` list setup with a call to `rs_summarizer::state::get_default_models()`.

---

### Services Component

#### [MODIFY] [summary.rs](file:///home/kiel/stage/rs-summarizer/src/services/summary.rs)
- Update prompt dispatch logic in `generate_summary` to route based on `model.architecture` instead of string prefix checks.
- Update `test_model()` in the unit tests to construct `ModelOption` with `ModelArchitecture::Gemini`.

#### [MODIFY] [rate_limiter.rs](file:///home/kiel/stage/rs-summarizer/src/services/rate_limiter.rs)
- Update mock `ModelOption` constructions in unit tests to include the `architecture` field.

---

## Verification Plan

### Automated Tests
- Run `cargo test` to execute all existing unit tests and integration tests, as well as the new configuration/limits validation test cases.
- Run `cargo check` to ensure clean compilation across all modules.

### Manual Verification
- Not applicable (no UI modifications are required).
