Below is a detailed analysis of the Rust codebase, along with a comprehensive implementation plan to integrate new models and limits. This includes clear file summaries, implementation changes with example code, suggested architectural improvements, and new test cases. The plan required 80k tokens in Gemini 3.5 Flash at a cost of $0.24.

---

### File-by-File Architecture Summary

The following list provides a functional overview of the files in your codebase:

#### Core Execution & Configuration
*   **`src/main.rs`**: The entry point of the system. Initializes tracing, parses command-line flags (e.g., `export-db`), manages the Gemini API key, instantiates the database, loads sidecar visualization configurations (`NnMapper` and `VizData`), defines the supported model configurations, and runs the Axum web server.
*   **`src/lib.rs`**: Declares public library modules and defines the Axum HTTP router structure.
*   **`src/state.rs`**: Holds shared state types (`AppState` and `ModelOption`) shared across routes and background tasks.
*   **`src/models.rs`**: Declares core domain structs and parameters mapping to database schemas (`Summary`, `SubmitForm`, `SearchForm`, `BrowseParams`, `VizData`).
*   **`src/errors.rs`**: Defines standardized, type-safe error definitions via the `thiserror` crate (`TranscriptError`, `SummaryError`, `EmbeddingError`, `ProcessError`, `ExportError`, `NnMapperError`).

#### Services
*   **`src/services/mod.rs`**: Module declarations for all application service modules.
*   **`src/services/transcript.rs`**: Interacts with the local system to execute `yt-dlp` via `uvx`, parses and retrieves subtitle lists, applies prioritization logic to languages, and reads download output.
*   **`src/services/summary.rs`**: Manages generation logic with the `gemini-rust` SDK, formats text templates, streams response chunks, computes token metrics, and calculates generation costs.
*   **`src/services/rate_limiter.rs`**: Manages API usage rules (requests per minute and requests per day limits) with automatic date-reset intervals based on the America/Los_Angeles timezone.
*   **`src/services/embedding.rs`**: Computes text embeddings using the API, calculates cosine similarities, and implements Matryoshka truncation safety.
*   **`src/services/nn_mapper.rs`**: Project/project UMAP visual data points down to 2D coordinates for vector map representation using `fast-umap` and `burn` machine-learning backends.
*   **`src/services/deduplication.rs`**: Runs logic to find identical source links or matching transcripts processed within a 5-minute window to avoid repetitive API spending.

#### Utilities
*   **`src/utils/mod.rs`**: Module declarations for utility functions.
*   **`src/utils/url_validator.rs`**: Runs regular expressions to parse and validate standard, short, live-stream, and mobile YouTube URLs.
*   **`src/utils/vtt_parser.rs`**: Parses WebVTT cue structures into raw timestamped text blocks, sanitizing styling headers and markup.
*   **`src/utils/markdown_converter.rs`**: Converts standard markdown styling into YouTube-specific comment markup (e.g., converting double asterisks to single, and neutralizing links).
*   **`src/utils/timestamp_linker.rs`**: Finds timestamps inside raw content and converts them to clickable anchor tags with parameter links.
*   **`src/utils/markdown_renderer.rs`**: Parses raw markdown to compliant HTML blocks using `pulldown-cmark`.

#### UI Templates & Routes
*   **`src/routes/mod.rs`**: Declares HTTP endpoints matching view requests, including standard page loading, HTMX polling, text searching, and pagination.
*   **`src/templates.rs`**: Links structural Askama visual HTML templates.
*   **`src/cache.rs`**: Manages in-memory metadata summaries to group duplicates and render browse pages.
*   **`src/tasks.rs`**: Orchestrates background workers that combine transcript downloads, summary generations, and embedding processing.
*   **`src/commands/mod.rs`** & **`src/commands/export_db.rs`**: Implements offline CLI utilities to extract and save compact SQL representations of the core data.

---

### Model Configuration Updates (Migration Plan)

To integrate the new models, we should move the hardcoded configurations out of `src/main.rs` into a central constructor in `src/state.rs`. This provides clean separation of concerns and lets us attach structured model attributes directly to `ModelOption`.

#### 1. Update `src/state.rs`
We will define a `ModelArchitecture` enum to structure prompt formulation and mapping logic. The RPM and RPD numbers represent the *denominator* limits parsed from your configuration table:

```rust
// src/state.rs
use chrono::NaiveDate;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ModelArchitecture {
    Gemini,
    Gemma,
    Other, // For embeddings, image generation, grounding, etc.
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ModelOption {
    pub name: String,
    pub input_price_per_mtoken: f64,
    pub output_price_per_mtoken: f64,
    pub context_window: u64,
    pub rpm_limit: u32,
    pub rpd_limit: u32,
    pub architecture: ModelArchitecture,
}

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub model_options: Arc<Vec<ModelOption>>,
    pub model_counts: Arc<RwLock<HashMap<String, u32>>>,
    pub last_reset_day: Arc<RwLock<Option<NaiveDate>>>,
    pub gemini_api_key: String,
    pub nn_mapper: Option<std::sync::Arc<std::sync::Mutex<crate::services::nn_mapper::NnMapper>>>,
    pub viz_data: Option<std::sync::Arc<crate::models::VizData>>,
}

/// Retrieve default baseline configurations reflecting the model list
pub fn get_default_models() -> Vec<ModelOption> {
    vec![
        // 1. Gemini 3 Flash (Text-out models)
        ModelOption {
            name: "gemini-3-flash".to_string(),
            input_price_per_mtoken: 0.10,
            output_price_per_mtoken: 0.40,
            context_window: 1_048_576,
            rpm_limit: 5,
            rpd_limit: 20,
            architecture: ModelArchitecture::Gemini,
        },
        // 2. Gemini 3.1 Flash Lite (Text-out models)
        ModelOption {
            name: "gemini-3.1-flash-lite".to_string(),
            input_price_per_mtoken: 0.075,
            output_price_per_mtoken: 0.30,
            context_window: 1_048_576,
            rpm_limit: 15,
            rpd_limit: 500,
            architecture: ModelArchitecture::Gemini,
        },
        // 3. Gemma 4 26B (Mixture-of-Experts)
        ModelOption {
            name: "gemma-4-26b-a4b-it".to_string(),
            input_price_per_mtoken: 0.07,  // Standard open-weights token pricing model
            output_price_per_mtoken: 0.34,
            context_window: 256_000,
            rpm_limit: 15,
            rpd_limit: 1500, // 1.5K
            architecture: ModelArchitecture::Gemma,
        },
        // 4. Gemini Embedding 1
        ModelOption {
            name: "text-embedding-004".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 2048,
            rpm_limit: 100,
            rpd_limit: 1000, // 1K
            architecture: ModelArchitecture::Other,
        },
        // 5. Gemini 3.5 Flash (Text-out models)
        ModelOption {
            name: "gemini-3.5-flash".to_string(),
            input_price_per_mtoken: 0.10,
            output_price_per_mtoken: 0.40,
            context_window: 1_000_000,
            rpm_limit: 5,
            rpd_limit: 20,
            architecture: ModelArchitecture::Gemini,
        },
        // 6. Imagen 4 Generate (Multi-modal)
        ModelOption {
            name: "imagen-4.0-generate-001".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 0,
            rpm_limit: 5, // Assigned baseline default limits where unspecified
            rpd_limit: 25,
            architecture: ModelArchitecture::Other,
        },
        // 7. Imagen 4 Ultra Generate (Multi-modal)
        ModelOption {
            name: "imagen-4.0-ultra-generate-001".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 0,
            rpm_limit: 5,
            rpd_limit: 25,
            architecture: ModelArchitecture::Other,
        },
        // 8. Imagen 4 Fast Generate (Multi-modal)
        ModelOption {
            name: "imagen-4.0-fast-generate-001".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 0,
            rpm_limit: 5,
            rpd_limit: 25,
            architecture: ModelArchitecture::Other,
        },
        // 9. Gemini 2.5 Flash (Map grounding)
        ModelOption {
            name: "gemini-2.5-flash-map".to_string(),
            input_price_per_mtoken: 0.15,
            output_price_per_mtoken: 0.60,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 500,
            architecture: ModelArchitecture::Other,
        },
        // 10. Gemini 3.1 Flash Lite (Map grounding)
        ModelOption {
            name: "gemini-3.1-flash-lite-map".to_string(),
            input_price_per_mtoken: 0.075,
            output_price_per_mtoken: 0.30,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 500,
            architecture: ModelArchitecture::Other,
        },
        // 11. Gemini 3.1 Flash TTS (Map grounding)
        ModelOption {
            name: "gemini-3.1-flash-tts-map".to_string(),
            input_price_per_mtoken: 0.075,
            output_price_per_mtoken: 0.30,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 500,
            architecture: ModelArchitecture::Other,
        },
        // 12. Gemini Robotics ER 1.6 Preview (Map grounding)
        ModelOption {
            name: "gemini-robotics-er-1.6-preview-map".to_string(),
            input_price_per_mtoken: 0.15,
            output_price_per_mtoken: 0.60,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 500,
            architecture: ModelArchitecture::Other,
        },
        // 13. Computer Use Preview (Map grounding)
        ModelOption {
            name: "computer-use-preview-map".to_string(),
            input_price_per_mtoken: 0.15,
            output_price_per_mtoken: 0.60,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 500,
            architecture: ModelArchitecture::Other,
        },
        // 14. Deep Research Pro Preview (Map grounding)
        ModelOption {
            name: "deep-research-pro-preview-map".to_string(),
            input_price_per_mtoken: 0.15,
            output_price_per_mtoken: 0.60,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 500,
            architecture: ModelArchitecture::Other,
        },
        // 15. Gemini 2.0 (Search grounding)
        ModelOption {
            name: "gemini-2.0".to_string(),
            input_price_per_mtoken: 0.15,
            output_price_per_mtoken: 0.60,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 1500, // 1.5K
            architecture: ModelArchitecture::Gemini,
        },
        // 16. Gemini 2.5 (Search grounding)
        ModelOption {
            name: "gemini-2.5".to_string(),
            input_price_per_mtoken: 0.15,
            output_price_per_mtoken: 0.60,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 1500, // 1.5K
            architecture: ModelArchitecture::Gemini,
        },
        // 17. Default (Search grounding)
        ModelOption {
            name: "default".to_string(),
            input_price_per_mtoken: 0.15,
            output_price_per_mtoken: 0.60,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 1500, // 1.5K
            architecture: ModelArchitecture::Gemini,
        },
    ]
}
```

#### 2. Clean Up `src/main.rs`
Since configurations are now centralized in `src/state.rs`, we can clean up the model setup in `src/main.rs` by referencing the configuration helper function directly:

```rust
// Inside src/main.rs - replace the large model_options setup with:
    
    // Configure model options
    let model_options = rs_summarizer::state::get_default_models();
```

---

### Potential Code Improvements

#### Improvement A: Architecture-Aware Prompt Dispatch
In your original implementation, `src/services/summary.rs` decides whether to prepend system instructions with a string-based name check: `!model.name.starts_with("gemma")`. This approach is fragile when introducing new models.

Using the new `ModelArchitecture` enum in `ModelOption`, we can rewrite prompt routing inside `src/services/summary.rs`:

```rust
// src/services/summary.rs
// Inside generate_summary:

        // Model-aware prompt routing based on structured architecture variants
        let prompt = match model.architecture {
            crate::state::ModelArchitecture::Gemini => {
                builder = builder.with_system_prompt(SYSTEM_INSTRUCTION);
                self.build_prompt(transcript)
            }
            crate::state::ModelArchitecture::Gemma => {
                self.build_prompt_for_gemma(transcript)
            }
            crate::state::ModelArchitecture::Other => {
                // Grounding, embeddings, or other systems use fallback base prompts
                self.build_prompt(transcript)
            }
        };
```

#### Improvement B: Real DST-Aware Timezone Handling in `src/services/rate_limiter.rs`
The existing daily rate limit reset calculates Pacific Time by subtracting a hardcoded 8 hours from UTC:

```rust
    fn today_la() -> NaiveDate {
        let utc_now = chrono::Utc::now();
        let la_time = utc_now - chrono::Duration::hours(8);
        la_time.date_naive()
    }
```

This approximation does not account for Daylight Saving Time (DST) changes. Since America/Los_Angeles switches between PST (UTC-8) and PDT (UTC-7) dynamically throughout the year, we can introduce a pure Rust, zero-dependency DST helper inside `src/services/rate_limiter.rs` to compute the correct offset:

```rust
// src/services/rate_limiter.rs
use chrono::{Datelike, NaiveDate, Weekday};

impl RateLimiter {
    // ... (rest of methods)

    /// Dynamic local time offset lookup for America/Los_Angeles
    fn today_la() -> NaiveDate {
        let utc_now = chrono::Utc::now();
        let year = utc_now.year();

        // DST in US begins second Sunday in March
        let mut march_dst_start = NaiveDate::from_ymd_opt(year, 3, 1).unwrap();
        let mut sundays_found = 0;
        while sundays_found < 2 {
            if march_dst_start.weekday() == Weekday::Sun {
                sundays_found += 1;
                if sundays_found == 2 { break; }
            }
            march_dst_start = march_dst_start.succ_opt().unwrap();
        }
        
        // DST in US ends first Sunday in November
        let mut nov_dst_end = NaiveDate::from_ymd_opt(year, 11, 1).unwrap();
        while nov_dst_end.weekday() != Weekday::Sun {
            nov_dst_end = nov_dst_end.succ_opt().unwrap();
        }

        // Check if UTC is within daylight-savings boundaries
        let is_dst = utc_now.date_naive() >= march_dst_start && utc_now.date_naive() < nov_dst_end;
        let offset_hours = if is_dst { 7 } else { 8 };

        let la_time = utc_now - chrono::Duration::hours(offset_hours);
        la_time.date_naive()
    }
}
```

---

### Suggested Testing Additions

To ensure the new models match their limits, we can add a targeted unit test suite inside `src/services/rate_limiter.rs` and verify model retrieval:

```rust
// Added tests inside src/state.rs or tests/model_checks.rs
#[cfg(test)]
mod model_checks {
    use super::*;

    #[test]
    fn test_unique_model_names() {
        let models = get_default_models();
        let mut names = std::collections::HashSet::new();
        for m in &models {
            assert!(names.insert(m.name.clone()), "Duplicate model configuration name registered: {}", m.name);
        }
    }

    #[test]
    fn test_model_pricing_is_valid() {
        let models = get_default_models();
        for m in models {
            assert!(m.input_price_per_mtoken >= 0.0);
            assert!(m.output_price_per_mtoken >= 0.0);
        }
    }

    #[test]
    fn test_updated_model_limits() {
        let models = get_default_models();
        
        // Verify Gemini 3.5 Flash limits
        let gemini_35 = models.iter().find(|m| m.name == "gemini-3.5-flash").unwrap();
        assert_eq!(gemini_35.rpm_limit, 5);
        assert_eq!(gemini_35.rpd_limit, 20);

        // Verify Gemma 4 26B MoE limits
        let gemma_4 = models.iter().find(|m| m.name == "gemma-4-26b-a4b-it").unwrap();
        assert_eq!(gemma_4.rpm_limit, 15);
        assert_eq!(gemma_4.rpd_limit, 1500);
    }
}
```
