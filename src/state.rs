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

impl ModelArchitecture {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelArchitecture::Gemini => "Gemini",
            ModelArchitecture::Gemma => "Gemma",
            ModelArchitecture::Other => "Other",
        }
    }
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
    pub model_locks: Arc<RwLock<HashMap<String, Arc<tokio::sync::Mutex<Option<std::time::Instant>>>>>>,
}

impl AppState {
    pub async fn get_model_lock(&self, model_name: &str) -> Arc<tokio::sync::Mutex<Option<std::time::Instant>>> {
        {
            let locks = self.model_locks.read().await;
            if let Some(lock) = locks.get(model_name) {
                return lock.clone();
            }
        }
        let mut locks = self.model_locks.write().await;
        locks.entry(model_name.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(None)))
            .clone()
    }
}


/// Retrieve default baseline configurations reflecting the model list
pub fn get_default_models() -> Vec<ModelOption> {
    vec![
        // Auto Model Selection (Heuristic)
        ModelOption {
            name: "auto".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 1_048_576,
            rpm_limit: 15,
            rpd_limit: 1000,
            architecture: ModelArchitecture::Gemini,
        },
        // 1. Gemini 3.6 Flash (Text-out models)
        ModelOption {
            name: "gemini-3.6-flash".to_string(),
            input_price_per_mtoken: 0.10,
            output_price_per_mtoken: 0.40,
            context_window: 1_000_000,
            rpm_limit: 5,
            rpd_limit: 20,
            architecture: ModelArchitecture::Gemini,
        },
        // 2. Gemini 3.5 Flash Lite (Text-out models)
        ModelOption {
            name: "gemini-3.5-flash-lite".to_string(),
            input_price_per_mtoken: 0.075,
            output_price_per_mtoken: 0.30,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 500,
            architecture: ModelArchitecture::Gemini,
        },
        // 3. Gemini 3.5 Flash (Text-out models)
        ModelOption {
            name: "gemini-3.5-flash".to_string(),
            input_price_per_mtoken: 0.10,
            output_price_per_mtoken: 0.40,
            context_window: 1_000_000,
            rpm_limit: 5,
            rpd_limit: 20,
            architecture: ModelArchitecture::Gemini,
        },
        // 4. Gemma 4 31B (Mixture-of-Experts)
        ModelOption {
            name: "gemma-4-31b-it".to_string(),
            input_price_per_mtoken: 0.07,
            output_price_per_mtoken: 0.34,
            context_window: 262_144,
            rpm_limit: 30,
            rpd_limit: 14400,
            architecture: ModelArchitecture::Gemma,
        },
        // 5. Gemma 4 26B (Mixture-of-Experts)
        ModelOption {
            name: "gemma-4-26b-a4b-it".to_string(),
            input_price_per_mtoken: 0.07,
            output_price_per_mtoken: 0.34,
            context_window: 256_000,
            rpm_limit: 30,
            rpd_limit: 14400,
            architecture: ModelArchitecture::Gemma,
        },
        // 6. Gemini 3.1 Flash Lite (Text-out models)
        ModelOption {
            name: "gemini-3.1-flash-lite".to_string(),
            input_price_per_mtoken: 0.075,
            output_price_per_mtoken: 0.30,
            context_window: 1_048_576,
            rpm_limit: 15,
            rpd_limit: 500,
            architecture: ModelArchitecture::Gemini,
        },
        // 7. Gemini 2.5 Flash (Text-out models)
        ModelOption {
            name: "gemini-2.5-flash".to_string(),
            input_price_per_mtoken: 0.10,
            output_price_per_mtoken: 0.40,
            context_window: 1_048_576,
            rpm_limit: 5,
            rpd_limit: 20,
            architecture: ModelArchitecture::Gemini,
        },
        // 8. Gemini 2.5 Flash Lite (Text-out models)
        ModelOption {
            name: "gemini-2.5-flash-lite".to_string(),
            input_price_per_mtoken: 0.075,
            output_price_per_mtoken: 0.30,
            context_window: 1_048_576,
            rpm_limit: 10,
            rpd_limit: 20,
            architecture: ModelArchitecture::Gemini,
        },
        // 9. Gemini 3 Flash Preview (Text-out models)
        ModelOption {
            name: "gemini-3-flash-preview".to_string(),
            input_price_per_mtoken: 0.10,
            output_price_per_mtoken: 0.40,
            context_window: 1_048_576,
            rpm_limit: 5,
            rpd_limit: 20,
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
        
        // Verify Gemini 3.6 Flash limits
        let gemini_36 = models.iter().find(|m| m.name == "gemini-3.6-flash").unwrap();
        assert_eq!(gemini_36.rpm_limit, 5);
        assert_eq!(gemini_36.rpd_limit, 20);

        // Verify Gemini 3.5 Flash Lite limits
        let gemini_35_lite = models.iter().find(|m| m.name == "gemini-3.5-flash-lite").unwrap();
        assert_eq!(gemini_35_lite.rpm_limit, 15);
        assert_eq!(gemini_35_lite.rpd_limit, 500);

        // Verify Gemini 3.5 Flash limits
        let gemini_35 = models.iter().find(|m| m.name == "gemini-3.5-flash").unwrap();
        assert_eq!(gemini_35.rpm_limit, 5);
        assert_eq!(gemini_35.rpd_limit, 20);

        // Verify Gemma 4 26B MoE limits
        let gemma_4 = models.iter().find(|m| m.name == "gemma-4-26b-a4b-it").unwrap();
        assert_eq!(gemma_4.rpm_limit, 30);
        assert_eq!(gemma_4.rpd_limit, 14400);
    }
}
