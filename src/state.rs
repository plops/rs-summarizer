use chrono::NaiveDate;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ModelArchitecture {
    Gemini,
    Gemma,
    Other,
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
            input_price_per_mtoken: 0.07,
            output_price_per_mtoken: 0.34,
            context_window: 256_000,
            rpm_limit: 15,
            rpd_limit: 1500,
            architecture: ModelArchitecture::Gemma,
        },
        // 4. Gemini 3.5 Flash (Text-out models)
        ModelOption {
            name: "gemini-3.5-flash".to_string(),
            input_price_per_mtoken: 0.10,
            output_price_per_mtoken: 0.40,
            context_window: 1_000_000,
            rpm_limit: 5,
            rpd_limit: 20,
            architecture: ModelArchitecture::Gemini,
        },
        // 5. Imagen 4 Generate (Multi-modal)
        ModelOption {
            name: "imagen-4.0-generate-001".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 0,
            rpm_limit: 5,
            rpd_limit: 25,
            architecture: ModelArchitecture::Other,
        },
        // 6. Imagen 4 Ultra Generate (Multi-modal)
        ModelOption {
            name: "imagen-4.0-ultra-generate-001".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 0,
            rpm_limit: 5,
            rpd_limit: 25,
            architecture: ModelArchitecture::Other,
        },
        // 7. Imagen 4 Fast Generate (Multi-modal)
        ModelOption {
            name: "imagen-4.0-fast-generate-001".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 0,
            rpm_limit: 5,
            rpd_limit: 25,
            architecture: ModelArchitecture::Other,
        },
        // 8. Gemini 2.5 Flash (Map grounding)
        ModelOption {
            name: "gemini-2.5-flash-map".to_string(),
            input_price_per_mtoken: 0.15,
            output_price_per_mtoken: 0.60,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 500,
            architecture: ModelArchitecture::Other,
        },
        // 9. Gemini 3.1 Flash Lite (Map grounding)
        ModelOption {
            name: "gemini-3.1-flash-lite-map".to_string(),
            input_price_per_mtoken: 0.075,
            output_price_per_mtoken: 0.30,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 500,
            architecture: ModelArchitecture::Other,
        },
        // 10. Gemini 3.1 Flash TTS (Map grounding)
        ModelOption {
            name: "gemini-3.1-flash-tts-map".to_string(),
            input_price_per_mtoken: 0.075,
            output_price_per_mtoken: 0.30,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 500,
            architecture: ModelArchitecture::Other,
        },
        // 11. Gemini Robotics ER 1.6 Preview (Map grounding)
        ModelOption {
            name: "gemini-robotics-er-1.6-preview-map".to_string(),
            input_price_per_mtoken: 0.15,
            output_price_per_mtoken: 0.60,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 500,
            architecture: ModelArchitecture::Other,
        },
        // 12. Computer Use Preview (Map grounding)
        ModelOption {
            name: "computer-use-preview-map".to_string(),
            input_price_per_mtoken: 0.15,
            output_price_per_mtoken: 0.60,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 500,
            architecture: ModelArchitecture::Other,
        },
        // 13. Deep Research Pro Preview (Map grounding)
        ModelOption {
            name: "deep-research-pro-preview-map".to_string(),
            input_price_per_mtoken: 0.15,
            output_price_per_mtoken: 0.60,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 500,
            architecture: ModelArchitecture::Other,
        },
        // 14. Gemini 2.0 (Search grounding)
        ModelOption {
            name: "gemini-2.0".to_string(),
            input_price_per_mtoken: 0.15,
            output_price_per_mtoken: 0.60,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 1500,
            architecture: ModelArchitecture::Gemini,
        },
        // 15. Gemini 2.5 (Search grounding)
        ModelOption {
            name: "gemini-2.5".to_string(),
            input_price_per_mtoken: 0.15,
            output_price_per_mtoken: 0.60,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 1500,
            architecture: ModelArchitecture::Gemini,
        },
        // 16. Default (Search grounding)
        ModelOption {
            name: "default".to_string(),
            input_price_per_mtoken: 0.15,
            output_price_per_mtoken: 0.60,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 1500,
            architecture: ModelArchitecture::Gemini,
        },
    ]
}

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
