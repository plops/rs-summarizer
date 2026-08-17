# Task Breakdown: Additional Hetzner Inference Models

**Project**: `plops/rs-summarizer`
**Reference**: [plan.md](file:///workspace/src/rs-summarizer/plan/20260817_01_more_hetzner_models/plan.md)

Each task is self-contained: implement → test → validate → commit. Execute serially.

---

## Task 1: Model Name Resolution Map

**Goal**: Replace the hardcoded `Qwen/Qwen3.6-35B-A3B-FP8` fallback with a proper model name resolution function.

### Files to Modify
- [src/services/summary.rs](file:///workspace/src/rs-summarizer/src/services/summary.rs)

### Steps
1. Add a new function `resolve_hetzner_model_name(internal_name: &str) -> &str` in `src/services/summary.rs` (before `generate_summary_hetzner`):
   ```rust
   /// Maps internal Hetzner model names to their API-side model identifiers.
   fn resolve_hetzner_model_name(internal_name: &str) -> &str {
       match internal_name {
           "hetzner-qwen-3.6-35b" => "Qwen/Qwen3.6-35B-A3B-FP8",
           "hetzner-deepseek-v4-flash" => "DeepSeek-V4-Flash-0731",
           "hetzner-glm-5.2" => "GLM-5.2-NVFP4",
           "hetzner-kimi-k2.7-code" => "Kimi-K2.7-Code",
           other => other, // pass-through for direct API names (e.g. containing '/')
       }
   }
   ```
2. Replace the model name resolution block in `generate_summary_hetzner()` (lines ~450-454):
   ```rust
   // BEFORE:
   let actual_model_name = if model.name.contains('/') {
       model.name.clone()
   } else {
       "Qwen/Qwen3.6-35B-A3B-FP8".to_string()
   };

   // AFTER:
   let actual_model_name = resolve_hetzner_model_name(&model.name).to_string();
   ```
3. Add unit tests in the `#[cfg(test)] mod tests` block:
   ```rust
   #[test]
   fn test_resolve_hetzner_model_name() {
       assert_eq!(resolve_hetzner_model_name("hetzner-qwen-3.6-35b"), "Qwen/Qwen3.6-35B-A3B-FP8");
       assert_eq!(resolve_hetzner_model_name("hetzner-deepseek-v4-flash"), "DeepSeek-V4-Flash-0731");
       assert_eq!(resolve_hetzner_model_name("hetzner-glm-5.2"), "GLM-5.2-NVFP4");
       assert_eq!(resolve_hetzner_model_name("hetzner-kimi-k2.7-code"), "Kimi-K2.7-Code");
       // Pass-through for direct API names
       assert_eq!(resolve_hetzner_model_name("Qwen/Qwen3.6-35B-A3B-FP8"), "Qwen/Qwen3.6-35B-A3B-FP8");
       // Unknown names pass through unchanged
       assert_eq!(resolve_hetzner_model_name("unknown-model"), "unknown-model");
   }
   ```

### Validate
```bash
cargo test -- test_resolve_hetzner_model_name
cargo test
```

### Commit
```
feat(llm): add Hetzner model name resolution map for multi-model support

Replace hardcoded Qwen/Qwen3.6-35B-A3B-FP8 fallback in
generate_summary_hetzner() with a dedicated resolve_hetzner_model_name()
function that maps internal model names to Hetzner API identifiers.

Mappings:
- hetzner-qwen-3.6-35b → Qwen/Qwen3.6-35B-A3B-FP8
- hetzner-deepseek-v4-flash → DeepSeek-V4-Flash-0731
- hetzner-glm-5.2 → GLM-5.2-NVFP4
- hetzner-kimi-k2.7-code → Kimi-K2.7-Code

Unknown names pass through unchanged for forward compatibility.
Includes unit tests for all mappings and pass-through behavior.
```

---

## Task 2: Register New Models in State & Config

**Goal**: Add the 3 new Hetzner models to the model registry.

### Files to Modify
- [src/state.rs](file:///workspace/src/rs-summarizer/src/state.rs) — `get_default_models()` function
- [config/models.json](file:///workspace/src/rs-summarizer/config/models.json) — JSON model array

### Steps
1. Add 3 new `ModelOption` entries to `get_default_models()` in `src/state.rs`, after the existing Hetzner model (line ~195):
   ```rust
   // 11. Hetzner DeepSeek V4 Flash (OpenAI-compatible experimental inference API)
   ModelOption {
       name: "hetzner-deepseek-v4-flash".to_string(),
       input_price_per_mtoken: 0.0,
       output_price_per_mtoken: 0.0,
       context_window: 512_000,
       rpm_limit: 60,
       rpd_limit: 14400,
       architecture: ModelArchitecture::Hetzner,
   },
   // 12. Hetzner GLM 5.2 NVFP4 (OpenAI-compatible experimental inference API)
   ModelOption {
       name: "hetzner-glm-5.2".to_string(),
       input_price_per_mtoken: 0.0,
       output_price_per_mtoken: 0.0,
       context_window: 512_000,
       rpm_limit: 60,
       rpd_limit: 14400,
       architecture: ModelArchitecture::Hetzner,
   },
   // 13. Hetzner Kimi K2.7 Code (OpenAI-compatible experimental inference API)
   ModelOption {
       name: "hetzner-kimi-k2.7-code".to_string(),
       input_price_per_mtoken: 0.0,
       output_price_per_mtoken: 0.0,
       context_window: 262_144,
       rpm_limit: 60,
       rpd_limit: 14400,
       architecture: ModelArchitecture::Hetzner,
   },
   ```
2. Add corresponding entries to `config/models.json`:
   ```json
   {
     "name": "hetzner-deepseek-v4-flash",
     "input_price_per_mtoken": 0.0,
     "output_price_per_mtoken": 0.0,
     "context_window": 512000,
     "rpm_limit": 60,
     "rpd_limit": 14400,
     "architecture": "Hetzner"
   },
   {
     "name": "hetzner-glm-5.2",
     "input_price_per_mtoken": 0.0,
     "output_price_per_mtoken": 0.0,
     "context_window": 512000,
     "rpm_limit": 60,
     "rpd_limit": 14400,
     "architecture": "Hetzner"
   },
   {
     "name": "hetzner-kimi-k2.7-code",
     "input_price_per_mtoken": 0.0,
     "output_price_per_mtoken": 0.0,
     "context_window": 262144,
     "rpm_limit": 60,
     "rpd_limit": 14400,
     "architecture": "Hetzner"
   }
   ```
3. Add a test in `src/state.rs` `mod model_checks`:
   ```rust
   #[test]
   fn test_all_hetzner_models_registered() {
       let models = get_default_models();
       let hetzner_names: Vec<&str> = vec![
           "hetzner-qwen-3.6-35b",
           "hetzner-deepseek-v4-flash",
           "hetzner-glm-5.2",
           "hetzner-kimi-k2.7-code",
       ];
       for name in &hetzner_names {
           let model = models.iter().find(|m| m.name == *name);
           assert!(model.is_some(), "Hetzner model '{}' not found in defaults", name);
           let m = model.unwrap();
           assert_eq!(m.architecture, ModelArchitecture::Hetzner);
           assert_eq!(m.input_price_per_mtoken, 0.0);
           assert_eq!(m.output_price_per_mtoken, 0.0);
       }
   }

   #[test]
   fn test_hetzner_context_windows() {
       let models = get_default_models();
       let qwen = models.iter().find(|m| m.name == "hetzner-qwen-3.6-35b").unwrap();
       assert_eq!(qwen.context_window, 262_144);
       let deepseek = models.iter().find(|m| m.name == "hetzner-deepseek-v4-flash").unwrap();
       assert_eq!(deepseek.context_window, 512_000);
       let glm = models.iter().find(|m| m.name == "hetzner-glm-5.2").unwrap();
       assert_eq!(glm.context_window, 512_000);
       let kimi = models.iter().find(|m| m.name == "hetzner-kimi-k2.7-code").unwrap();
       assert_eq!(kimi.context_window, 262_144);
   }
   ```
4. Update the `test_load_models_config` test: since `config/models.json` now has more models, the test comparing `models == default_models` should still pass if both were updated in sync.

### Validate
```bash
cargo test -- test_all_hetzner_models_registered test_hetzner_context_windows test_unique_model_names test_model_pricing_is_valid test_load_models_config
cargo test
```

### Commit
```
feat(models): register three new Hetzner inference models

Add DeepSeek-V4-Flash-0731 (512k ctx), GLM-5.2-NVFP4 (512k ctx), and
Kimi-K2.7-Code (262k ctx) to both get_default_models() in state.rs and
config/models.json.

All models use ModelArchitecture::Hetzner with $0.00 experimental
pricing and 60 RPM / 14400 RPD limits matching the existing Qwen model.

Includes unit tests verifying all 4 Hetzner models are registered with
correct architecture, pricing, and context window values.
```

---

## Task 3: Update Fallback Chains

**Goal**: Add fallback chains for new Hetzner models and integrate them into existing chains.

### Files to Modify
- [src/tasks.rs](file:///workspace/src/rs-summarizer/src/tasks.rs) — `get_fallback_chain()` function (L74-161)

### Steps
1. Update `get_fallback_chain()` to add new match arms for each new Hetzner model. Place before the `other => vec![other]` catch-all. Each Hetzner model falls back to other Hetzner models first, then Gemini:
   ```rust
   "hetzner-deepseek-v4-flash" => vec![
       "hetzner-deepseek-v4-flash",
       "hetzner-glm-5.2",
       "hetzner-kimi-k2.7-code",
       "hetzner-qwen-3.6-35b",
       "gemini-3.7-flash",
       "gemini-3.6-flash",
       "gemini-3.5-flash",
       "gemini-3.5-flash-lite",
   ],
   "hetzner-glm-5.2" => vec![
       "hetzner-glm-5.2",
       "hetzner-deepseek-v4-flash",
       "hetzner-kimi-k2.7-code",
       "hetzner-qwen-3.6-35b",
       "gemini-3.7-flash",
       "gemini-3.6-flash",
       "gemini-3.5-flash",
       "gemini-3.5-flash-lite",
   ],
   "hetzner-kimi-k2.7-code" => vec![
       "hetzner-kimi-k2.7-code",
       "hetzner-deepseek-v4-flash",
       "hetzner-glm-5.2",
       "hetzner-qwen-3.6-35b",
       "gemini-3.7-flash",
       "gemini-3.6-flash",
       "gemini-3.5-flash",
       "gemini-3.5-flash-lite",
   ],
   ```
2. Update the existing `hetzner-qwen-3.6-35b` chain to include the new Hetzner models:
   ```rust
   "hetzner-qwen-3.6-35b" => vec![
       "hetzner-qwen-3.6-35b",
       "hetzner-deepseek-v4-flash",
       "hetzner-glm-5.2",
       "hetzner-kimi-k2.7-code",
       "gemini-3.7-flash",
       "gemini-3.6-flash",
       "gemini-3.5-flash",
       "gemini-3.5-flash-lite",
   ],
   ```
3. Update existing Gemini fallback chains: add the new Hetzner models alongside the existing `hetzner-qwen-3.6-35b` at the tail end. Example for `gemini-3.7-flash`:
   ```rust
   "gemini-3.7-flash" => vec![
       "gemini-3.7-flash",
       "gemini-3.6-flash",
       "gemini-3.5-flash",
       "gemini-3-flash-preview",
       "gemini-2.5-flash",
       "gemini-3.5-flash-lite",
       "gemini-3.1-flash-lite",
       "gemini-2.5-flash-lite",
       "hetzner-deepseek-v4-flash",
       "hetzner-glm-5.2",
       "hetzner-kimi-k2.7-code",
       "hetzner-qwen-3.6-35b",
   ],
   ```
   Apply the same pattern to all other Gemini chains (replacing the single `hetzner-qwen-3.6-35b` tail entry with all 4 Hetzner models).
4. Add unit tests in the existing `mod tests` block of `tasks.rs`:
   ```rust
   #[test]
   fn test_fallback_chain_hetzner_deepseek() {
       let chain = get_fallback_chain("hetzner-deepseek-v4-flash");
       assert_eq!(chain[0], "hetzner-deepseek-v4-flash");
       assert!(chain.contains(&"hetzner-glm-5.2"));
       assert!(chain.contains(&"hetzner-kimi-k2.7-code"));
       assert!(chain.contains(&"hetzner-qwen-3.6-35b"));
       assert!(chain.contains(&"gemini-3.7-flash"));
   }

   #[test]
   fn test_fallback_chain_hetzner_glm() {
       let chain = get_fallback_chain("hetzner-glm-5.2");
       assert_eq!(chain[0], "hetzner-glm-5.2");
       assert!(chain.contains(&"hetzner-deepseek-v4-flash"));
       assert!(chain.contains(&"hetzner-qwen-3.6-35b"));
       assert!(chain.contains(&"gemini-3.7-flash"));
   }

   #[test]
   fn test_fallback_chain_hetzner_kimi() {
       let chain = get_fallback_chain("hetzner-kimi-k2.7-code");
       assert_eq!(chain[0], "hetzner-kimi-k2.7-code");
       assert!(chain.contains(&"hetzner-deepseek-v4-flash"));
       assert!(chain.contains(&"hetzner-glm-5.2"));
       assert!(chain.contains(&"hetzner-qwen-3.6-35b"));
       assert!(chain.contains(&"gemini-3.7-flash"));
   }

   #[test]
   fn test_gemini_chains_include_all_hetzner_models() {
       let hetzner_models = vec![
           "hetzner-deepseek-v4-flash",
           "hetzner-glm-5.2",
           "hetzner-kimi-k2.7-code",
           "hetzner-qwen-3.6-35b",
       ];
       for gemini_model in &["gemini-3.7-flash", "gemini-3.6-flash", "gemini-3.5-flash"] {
           let chain = get_fallback_chain(gemini_model);
           for hetzner in &hetzner_models {
               assert!(chain.contains(hetzner),
                   "Fallback chain for {} should contain {}", gemini_model, hetzner);
           }
       }
   }
   ```
5. Update the existing `test_fallback_chain_hetzner` test (currently named `test_fallback_chain_hetzner` testing the Qwen chain) to verify the new Hetzner models are included.

### Validate
```bash
cargo test -- test_fallback_chain
cargo test
```

### Commit
```
feat(routing): extend fallback chains with new Hetzner models

Add dedicated fallback chains for hetzner-deepseek-v4-flash,
hetzner-glm-5.2, and hetzner-kimi-k2.7-code. Each Hetzner model
falls back to other Hetzner models first, then to Gemini models.

Update existing hetzner-qwen-3.6-35b chain to include new Hetzner
models. Update all Gemini fallback chains to include all 4 Hetzner
models as last-resort fallbacks.

Includes unit tests for each new chain and a cross-check verifying
Gemini chains contain all Hetzner models.
```

---

## Task 4: Code Quality Validation

**Goal**: Ensure consistent code formatting and no lint warnings.

### Steps
1. Run `cargo fmt` to auto-format all modified files.
2. Run `cargo clippy -- -D warnings` to check for lint warnings.
3. Fix any issues found by clippy.

### Validate
```bash
cargo fmt --check
cargo clippy -- -D warnings
```

### Commit (only if changes needed)
```
chore(lint): apply formatting and fix clippy warnings
```

---

## Task 5: Full Test Suite & Compilation Check

**Goal**: Verify zero regressions across the entire test suite.

### Steps
1. Run `cargo build` to verify clean compilation.
2. Run `cargo test` to execute all unit and integration tests.
3. Verify the test count has increased (was ~130+ tests, should now have ~140+).
4. Check output for any `FAILED` or `error` entries.

### Validate
```bash
cargo build 2>&1
cargo test 2>&1
```

### Commit (only if test adjustments were needed)
```
test(hetzner): verify full test suite passes with new models
```

---

## Task 6: Documentation & Walkthrough

**Goal**: Document what was implemented, design decisions, and learnings.

### Files to Create
- [walkthrough.md](file:///workspace/src/rs-summarizer/plan/20260817_01_more_hetzner_models/walkthrough.md)

### Content Structure
1. **Summary of Changes** — What was implemented
2. **Files Modified** — List of changed files with brief descriptions
3. **Design Decisions** — Why certain approaches were chosen
4. **Test Results** — Summary of test execution
5. **Learnings** — What was discovered during implementation
6. **Future Extensions** — Possible next steps (shared rate limiter, auto-routing to large-context Hetzner models, code-content routing to Kimi)
7. **Docker Container Requirements** — Any new programs/tools needed (likely none — no new system dependencies)

### Validate
- Verify walkthrough.md is complete and well-formatted

### Commit
```
docs(walkthrough): document Hetzner multi-model implementation

Summarize implemented changes, design decisions, test outcomes,
learnings, and potential future extensions for the Hetzner multi-model
integration.
```
