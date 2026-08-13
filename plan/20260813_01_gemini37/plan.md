# Implementation Plan: Gemini 3.7 Flash Integration

**Client**: Wol Pumba (`wolpumba@gmail.com`)  
**Project**: `plops/rs-summarizer`  
**Plan Directory**: `file:///workspace/src/rs-summarizer/plan/20260813_01_gemini37/`  

---

## 1. Overview & Architectural Design

Google has released **Gemini 3.7 Flash** (`gemini-3.7-flash`), bringing enhanced reasoning capabilities, safety evaluations, and frontier agentic coding capabilities.

### Model Specifications & Parameters (from `models202608` & `gemini37.md`)
- **API Model Identifier**: `gemini-3.7-flash` (Custom model string `models/gemini-3.7-flash`)
- **Architecture**: `ModelArchitecture::Gemini`
- **Context Window**: 1,000,000 tokens
- **Pricing**:
  - Input: $0.10 per million tokens (Introductory pricing through December 31, 2026)
  - Output: $0.40 per million tokens
- **Rate Limits (Free/Tier Limits from `models202608`)**:
  - RPM (Requests Per Minute): 5
  - TPM (Tokens Per Minute): 250,000
  - RPD (Requests Per Day): 20
- **Thinking / Reasoning Mode**: Gemini 3.x thinking architecture (`ThinkingLevel::High` with thoughts included).

### Core Changes in `rs-summarizer`
1. **Model Configuration (`config/models.json` & `src/state.rs`)**:
   - Add `gemini-3.7-flash` at the top of the model selection list (immediately after `auto` and before `gemini-3.6-flash`).
   - Register default model options with exact context, pricing, RPM, and RPD limits.
2. **Dynamic Heuristic Auto-Selection (`src/tasks.rs:run_model_pipeline`)**:
   - Upgrade the long-content heuristic target from `gemini-3.6-flash` to `gemini-3.7-flash`:
     - **Hacker News**: Content >= 3,000 words -> `gemini-3.7-flash` (Content < 3,000 words -> `gemini-3.5-flash-lite`).
     - **YouTube / Transcripts**: Video duration >= 1,800 seconds (30 minutes) -> `gemini-3.7-flash` (Duration < 1,800 seconds -> `gemini-3.5-flash-lite`).
3. **Robust Fallback Chains (`src/tasks.rs:get_fallback_chain`)**:
   - Add comprehensive fallback chain for `gemini-3.7-flash`:
     `gemini-3.7-flash` -> `gemini-3.6-flash` -> `gemini-3.5-flash` -> `gemini-3-flash-preview` -> `gemini-2.5-flash` -> `gemini-3.5-flash-lite` -> `gemini-3.1-flash-lite` -> `gemini-2.5-flash-lite` -> `hetzner-qwen-3.6-35b`.
   - Update fallback chains for other models (`hetzner-qwen-3.6-35b`, `gemini-3.6-flash`, `gemini-3.5-flash-lite`, `gemini-3.1-flash-lite`, etc.) to include `gemini-3.7-flash`.
4. **Thinking & Tool Integration (`src/services/summary.rs`)**:
   - Ensure Gemini 3.x thinking configuration (`ThinkingLevel::High` and `with_thoughts_included(true)`) automatically applies to `gemini-3.7-flash` (matched via `.contains("gemini-3")`).
   - Retain full compatibility with Google Search Grounding and URL Context.

---

## 2. Requirements Assessment & Proposals

### User Requirements Checklist
- [x] Check `models202608` table and integrate `gemini-3.7-flash`.
- [x] Update `config/models.json` and `src/state.rs` default model configurations.
- [x] Update heuristics where Gemini 3.6 was previously used.
- [x] Maintain clean code formatting and linting via Cargo workflow skill.
- [x] Use DeepWiki MCP for `plops/rs-summarizer` and record dependencies in `deps.md`.
- [x] Provide autonomous AI agent context map and Conventional Commit guidelines.
- [x] Create step-by-step sequential `task.md`.
- [x] Add and execute new unit tests and integration tests.
- [x] Generate post-implementation `walkthrough.md`.
- [x] Create release and commit.

### Proposals & Recommendations
1. **Fallback Resilience Across Flash Generations**:
   - *Proposal*: Ensure all existing Gemini fallback chains include `gemini-3.7-flash` near the top of the chain so that if a user manually targets an older model (like `gemini-3.6-flash` or `hetzner-qwen-3.6-35b`) and hits a quota error, the system seamlessly tries the latest 3.7 model.
2. **Flash-Lite Model Preservation**:
   - *Proposal*: Do not attempt to add `gemini-3.7-flash-lite` as Google has not released a 3.7 variant of Flash-Lite yet (as verified in `models202608`). Keep `gemini-3.5-flash-lite` as the primary lightweight model for short transcripts (<30 min) to preserve high daily quota (500 RPD vs 20 RPD).
3. **Clippy & Linter Hygiene**:
   - *Proposal*: Clean up any outstanding clippy warnings in `src/` (e.g. `Default` implementations, type complexity annotations, unnecessary return statements) to maintain 100% clean CI/CD builds.
4. **Release Versioning**:
   - *Proposal*: Bump version from `1.2.1` to `1.3.0` in `Cargo.toml` as adding a new flagship AI model and updating heuristics represents a feature update (`minor` release in SemVer).

---

## 3. Autonomous AI Agent Context File Map

An autonomous AI agent tasked with working on this implementation should inspect the following key codebase files:

| File Path | Description & Relevance |
| :--- | :--- |
| [prompt.txt](file:///workspace/src/rs-summarizer/plan/20260813_01_gemini37/prompt.txt) | Original user prompt and specifications for the Gemini 3.7 Flash task. |
| [models202608](file:///workspace/src/rs-summarizer/plan/20260813_01_gemini37/models202608) | Official quota, RPM, and RPD table for Google AI models (August 2026). |
| [gemini37.md](file:///workspace/src/rs-summarizer/plan/20260813_01_gemini37/gemini37.md) | Release notes, pricing details, and migration checklist for Gemini 3.7 Flash. |
| [config/models.json](file:///workspace/src/rs-summarizer/config/models.json) | External JSON file loaded at runtime containing model options and rate limits. |
| [src/state.rs](file:///workspace/src/rs-summarizer/src/state.rs) | Model option struct definitions, baseline model list (`get_default_models`), and config loading unit tests. |
| [src/tasks.rs](file:///workspace/src/rs-summarizer/src/tasks.rs) | Background task pipeline (`run_model_pipeline`), duration heuristics, and fallback chain mappings (`get_fallback_chain`). |
| [src/services/summary.rs](file:///workspace/src/rs-summarizer/src/services/summary.rs) | Core Gemini summary generation service, streaming handler, thinking level configuration, and cost computation. |
| [templates/index.html](file:///workspace/src/rs-summarizer/templates/index.html) | Main web interface dynamically rendering model dropdown options and UI feature toggles. |
| [.kiro/skills/gemini-api-models/SKILL.md](file:///workspace/src/rs-summarizer/.kiro/skills/gemini-api-models/SKILL.md) | Project skill reference documenting Gemini model identifiers, quota limits, and API expectations. |
| [Cargo.toml](file:///workspace/src/rs-summarizer/Cargo.toml) | Package manifest and dependencies. |

---

## 4. Git Commit Message Guidelines

All git commits **must** adhere strictly to the **Conventional Commit** specification:
1. **Title**: `<type>(<scope>): <short imperative summary>` (max 72 chars).
2. **Body**: Detailed multi-line explanation of changes, rationale, design decisions, and testing verification.
3. **Types**:
   - `feat`: A new feature (e.g. adding Gemini 3.7 Flash model).
   - `fix`: A bug fix.
   - `refactor`: Code refactoring without behavioral change.
   - `docs`: Documentation changes (`plan.md`, `task.md`, `walkthrough.md`, `deps.md`).
   - `test`: Adding or updating tests.
   - `chore`: Version bumps, dependency updates.

### Example Commit Format
```gitcommit
feat(models): add Gemini 3.7 Flash and update auto-selection heuristics

- Add `gemini-3.7-flash` model configuration to `config/models.json` and `src/state.rs`.
- Update `run_model_pipeline` in `src/tasks.rs` to select `gemini-3.7-flash` for long transcripts and Hacker News articles in auto mode.
- Add `gemini-3.7-flash` fallback chain and include it in existing model chains.
- Add unit tests verifying `gemini-3.7-flash` limits, pricing, and fallback behavior.
```

---

## 5. Testing & Verification Strategy

1. **Unit Tests**:
   - Verify `gemini-3.7-flash` pricing, context window, RPM (5), and RPD (20) in `src/state.rs`.
   - Verify `config/models.json` parsing matches `get_default_models()`.
   - Verify `get_fallback_chain("gemini-3.7-flash")` returns the complete fallback list in `src/tasks.rs`.
   - Verify `auto` model selection selects `gemini-3.7-flash` for long transcripts (>= 1800s) and long HN submissions (>= 3000 words).
2. **Formatting & Linter Checks**:
   - Run `cargo fmt -- --check` to ensure standard formatting.
   - Run `cargo clippy -- -W clippy::all` to ensure zero clippy warnings.
3. **Full Test Suite**:
   - Run `cargo test` to execute all unit and integration tests.
