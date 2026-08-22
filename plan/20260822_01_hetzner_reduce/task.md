# Task Breakdown: Hetzner Inference Model Reduction & Qwen 3.8 Integration

**Project**: `plops/rs-summarizer`  
**Reference**: [plan.md](file:///workspace/src/rs-summarizer/plan/20260822_01_hetzner_reduce/plan.md)  
**Release Target**: `v1.6.0`  

Each task is self-contained: implement → test → validate → commit. Execute serially.

---

## Task 1: Update Model Registry in State & Config

**Goal**: Register the new `hetzner-qwen-3.8-27b` model and remove the decommissioned models (`hetzner-deepseek-v4-flash`, `hetzner-glm-5.2`, `hetzner-kimi-k2.7-code`) in `src/state.rs` and `config/models.json`.

### Files to Modify
- [config/models.json](file:///workspace/src/rs-summarizer/config/models.json)
- [src/state.rs](file:///workspace/src/rs-summarizer/src/state.rs)

### Steps
1. In `config/models.json`:
   - Keep `hetzner-qwen-3.6-35b`
   - Add `hetzner-qwen-3.8-27b`:
     ```json
     {
       "name": "hetzner-qwen-3.8-27b",
       "input_price_per_mtoken": 0.0,
       "output_price_per_mtoken": 0.0,
       "context_window": 262144,
       "rpm_limit": 60,
       "rpd_limit": 14400,
       "architecture": "Hetzner"
     }
     ```
   - Remove `hetzner-deepseek-v4-flash`, `hetzner-glm-5.2`, and `hetzner-kimi-k2.7-code`.
2. In `src/state.rs`:
   - In `get_default_models()`, update the Hetzner model options:
     ```rust
     // 10. Hetzner Qwen 3.6 35B (OpenAI-compatible experimental inference API)
     ModelOption {
         name: "hetzner-qwen-3.6-35b".to_string(),
         input_price_per_mtoken: 0.0,
         output_price_per_mtoken: 0.0,
         context_window: 262_144,
         rpm_limit: 60,
         rpd_limit: 14400,
         architecture: ModelArchitecture::Hetzner,
     },
     // 11. Hetzner Qwen 3.8 27B (OpenAI-compatible experimental inference API)
     ModelOption {
         name: "hetzner-qwen-3.8-27b".to_string(),
         input_price_per_mtoken: 0.0,
         output_price_per_mtoken: 0.0,
         context_window: 262_144,
         rpm_limit: 60,
         rpd_limit: 14400,
         architecture: ModelArchitecture::Hetzner,
     },
     ```
   - Update `mod model_checks` tests:
     - `test_all_hetzner_models_registered`: Check `hetzner-qwen-3.6-35b` and `hetzner-qwen-3.8-27b`.
     - `test_hetzner_context_windows`: Check `hetzner-qwen-3.6-35b` (262_144) and `hetzner-qwen-3.8-27b` (262_144).

### Validate
```bash
cargo test -- state::model_checks
```

### Commit
```text
feat(models): register Hetzner Qwen 3.8 27B and remove retired models

Update default models in src/state.rs and config/models.json to reflect
Hetzner's updated model availability:
- Register `hetzner-qwen-3.8-27b` with a 262,144 token context window and free experimental pricing
- Retain `hetzner-qwen-3.6-35b`
- Remove decommissioned models: hetzner-deepseek-v4-flash, hetzner-glm-5.2, hetzner-kimi-k2.7-code
- Update model registration and context window unit tests
```

---

## Task 2: Update Model Name Resolution Map

**Goal**: Update `resolve_hetzner_model_name()` in `src/services/summary.rs` to map `"hetzner-qwen-3.8-27b"` to `"Qwen3.8-27B"`.

### Files to Modify
- [src/services/summary.rs](file:///workspace/src/rs-summarizer/src/services/summary.rs)

### Steps
1. In `src/services/summary.rs`, update `resolve_hetzner_model_name()`:
   ```rust
   /// Maps internal Hetzner model names to their API-side model identifiers.
   fn resolve_hetzner_model_name(internal_name: &str) -> &str {
       match internal_name {
           "hetzner-qwen-3.6-35b" => "Qwen/Qwen3.6-35B-A3B-FP8",
           "hetzner-qwen-3.8-27b" => "Qwen3.8-27B",
           other => other, // pass-through for direct API names (e.g. containing '/')
       }
   }
   ```
2. Update unit tests in `src/services/summary.rs`:
   ```rust
   #[test]
   fn test_resolve_hetzner_model_name() {
       assert_eq!(
           resolve_hetzner_model_name("hetzner-qwen-3.6-35b"),
           "Qwen/Qwen3.6-35B-A3B-FP8"
       );
       assert_eq!(
           resolve_hetzner_model_name("hetzner-qwen-3.8-27b"),
           "Qwen3.8-27B"
       );
       // Direct API model names pass through unchanged
       assert_eq!(
           resolve_hetzner_model_name("Qwen/Qwen3.6-35B-A3B-FP8"),
           "Qwen/Qwen3.6-35B-A3B-FP8"
       );
       assert_eq!(
           resolve_hetzner_model_name("Qwen3.8-27B"),
           "Qwen3.8-27B"
       );
       assert_eq!(resolve_hetzner_model_name("unknown-model"), "unknown-model");
   }
   ```

### Validate
```bash
cargo test -- services::summary::tests
```

### Commit
```text
feat(llm): map Hetzner Qwen 3.8 27B model identifier

Update resolve_hetzner_model_name() in src/services/summary.rs to map
hetzner-qwen-3.8-27b to Qwen3.8-27B. Remove obsolete mapping entries while
preserving pass-through functionality for direct upstream identifiers.
Update unit tests to verify resolution and pass-through.
```

---

## Task 3: Update Fallback Chains & Routing

**Goal**: Update `get_fallback_chain()` in `src/tasks.rs` so that Hetzner models cross-fallback between each other and Gemini chains reference only active Hetzner models.

### Files to Modify
- [src/tasks.rs](file:///workspace/src/rs-summarizer/src/tasks.rs)

### Steps
1. Update `get_fallback_chain()`:
   - `hetzner-qwen-3.6-35b` -> `["hetzner-qwen-3.6-35b", "hetzner-qwen-3.8-27b", "gemini-3.7-flash", "gemini-3.6-flash", "gemini-3.5-flash", "gemini-3.5-flash-lite"]`
   - `hetzner-qwen-3.8-27b` -> `["hetzner-qwen-3.8-27b", "hetzner-qwen-3.6-35b", "gemini-3.7-flash", "gemini-3.6-flash", "gemini-3.5-flash", "gemini-3.5-flash-lite"]`
   - Remove deprecated `hetzner-deepseek-v4-flash`, `hetzner-glm-5.2`, and `hetzner-kimi-k2.7-code` match arms.
   - Update all Gemini model fallback chains (`gemini-3.7-flash`, `gemini-3.6-flash`, `gemini-3.5-flash-lite`, `gemini-3.5-flash`, `gemini-3-flash-preview`, `gemini-2.5-flash`, `gemini-3.1-flash-lite`, `gemini-2.5-flash-lite`) to append `hetzner-qwen-3.8-27b` and `hetzner-qwen-3.6-35b` as tail fallbacks.
2. Update unit tests in `src/tasks.rs`:
   - Update `test_get_fallback_chain_new_models`.
   - Replace old Hetzner chain tests with `test_fallback_chain_hetzner_qwen38`.
   - Update `test_gemini_chains_include_all_hetzner_models`.

### Validate
```bash
cargo test -- tasks::tests
```

### Commit
```text
feat(routing): update fallback chains for Hetzner Qwen 3.8 and Qwen 3.6

Update get_fallback_chain() in src/tasks.rs:
- Establish mutual fallback between hetzner-qwen-3.6-35b and hetzner-qwen-3.8-27b
- Remove decommissioned Hetzner model fallback arms
- Update all Gemini fallback chains to list active Hetzner models as tail fallbacks
- Update fallback chain unit tests
```

---

## Task 4: Code Quality Validation & Full Test Suite

**Goal**: Ensure code formatting, clippy compliance, and full test suite passing.

### Steps
1. Run `cargo fmt --check` (or `cargo fmt` if needed).
2. Run `cargo clippy -- -D warnings`.
3. Run `cargo test`.

### Validate
```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

### Commit (if formatting or clippy adjustments were needed)
```text
chore(lint): apply formatting and fix clippy warnings
```

---

## Task 5: Bump Version & Prepare Release v1.6.0

**Goal**: Prepare and tag release `v1.6.0`.

### Files to Modify
- [Cargo.toml](file:///workspace/src/rs-summarizer/Cargo.toml)
- [Cargo.lock](file:///workspace/src/rs-summarizer/Cargo.lock)
- [RELEASE_README.md](file:///workspace/src/rs-summarizer/RELEASE_README.md) (if necessary)

### Steps
1. Update `Cargo.toml` version from `1.5.0` to `1.6.0`.
2. Run `cargo check` to update `Cargo.lock`.
3. Commit version bump:
   ```text
   Release v1.6.0
   ```
4. Create git tag `v1.6.0`.

### Validate
```bash
git tag -l v1.6.0
cargo test
```

---

## Task 6: Documentation & Walkthrough

**Goal**: Create walkthrough document in `plan/20260822_01_hetzner_reduce/walkthrough.md`.

### Files to Create
- [walkthrough.md](file:///workspace/src/rs-summarizer/plan/20260822_01_hetzner_reduce/walkthrough.md)

### Commit
```text
docs(walkthrough): document Hetzner model catalog reduction and Qwen 3.8 integration
```
