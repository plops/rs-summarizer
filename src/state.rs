use chrono::NaiveDate;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::ThinkingPreference;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ModelArchitecture {
    Gemini,
    Gemma,
    Hetzner,
    Other,
}

impl ModelArchitecture {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelArchitecture::Gemini => "Gemini",
            ModelArchitecture::Gemma => "Gemma",
            ModelArchitecture::Hetzner => "Hetzner",
            ModelArchitecture::Other => "Other",
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelOption {
    pub name: String,
    pub input_price_per_mtoken: f64,
    pub output_price_per_mtoken: f64,
    pub context_window: u64,
    pub rpm_limit: u32,
    pub tpm_limit: u32,
    pub rpd_limit: u32,
    pub architecture: ModelArchitecture,
}

impl ModelOption {
    /// Whether this model belongs to the expensive tier.
    ///
    /// "Expensive" is a pricing class, not a fallback rank: a cheap or free
    /// model must never fall back onto an expensive model (see
    /// `get_fallback_chain` in `tasks.rs`). The threshold sits just above
    /// `gemini-3.5-flash-lite` (0.30/2.50), which is deliberately cheap, and
    /// at or below `gemini-3-flash-preview` (0.50/3.00), which is expensive.
    pub fn is_expensive(&self) -> bool {
        self.input_price_per_mtoken > 0.30 || self.output_price_per_mtoken > 2.50
    }

    /// Comma-separated thinking levels for the model dropdown
    /// (`data-thinking-levels` in `templates/index.html`).
    pub fn thinking_levels_csv(&self) -> String {
        supported_thinking_levels(&self.name)
            .iter()
            .map(|level| level.as_str())
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Highest supported thinking level for the model dropdown
    /// (`data-default-thinking` in `templates/index.html`).
    pub fn default_thinking(&self) -> &'static str {
        default_thinking_level(&self.name).as_str()
    }
}

/// Named thinking levels supported per model (Interactions API capability).
///
/// Replaces string-prefix checks such as `starts_with("gemini-3.")`. Models
/// not listed here (Gemini 2.5, Gemma, Hetzner) reject `thinking_level` in
/// the Interactions API and therefore only support `Auto` (send nothing).
const HYDROGEN_THINKING_LEVELS: &[ThinkingPreference] = &[
    ThinkingPreference::Auto,
    ThinkingPreference::Low,
    ThinkingPreference::Medium,
    ThinkingPreference::High,
    ThinkingPreference::XHigh,
];
const FLASH_THINKING_LEVELS: &[ThinkingPreference] = &[
    ThinkingPreference::Auto,
    ThinkingPreference::Low,
    ThinkingPreference::Medium,
    ThinkingPreference::High,
];
const LITE_THINKING_LEVELS: &[ThinkingPreference] = &[
    ThinkingPreference::Auto,
    ThinkingPreference::Minimal,
    ThinkingPreference::Low,
    ThinkingPreference::Medium,
    ThinkingPreference::High,
];
const AUTO_ONLY_THINKING_LEVELS: &[ThinkingPreference] = &[ThinkingPreference::Auto];

/// Returns the thinking levels the Interactions API accepts for `model_name`.
/// The `"auto"` pseudo-model delegates to [`crate::tasks::AUTO_MODEL`].
pub fn supported_thinking_levels(model_name: &str) -> &'static [ThinkingPreference] {
    if model_name.eq_ignore_ascii_case("auto") {
        return supported_thinking_levels(crate::tasks::AUTO_MODEL);
    }
    match model_name.to_ascii_lowercase().as_str() {
        "gemini-4-hydrogen" => HYDROGEN_THINKING_LEVELS,
        "gemini-3.8-flash" => FLASH_THINKING_LEVELS,
        "gemini-3.6-flash"
        | "gemini-3.5-flash-lite"
        | "gemini-3.1-flash-lite"
        | "gemini-3-flash-preview" => LITE_THINKING_LEVELS,
        _ => AUTO_ONLY_THINKING_LEVELS,
    }
}

/// Returns the highest supported thinking level for `model_name`
/// (`XHigh` for `gemini-4-hydrogen`, `High` for Gemini 3.x / `auto`,
/// `Auto` for everything else).
pub fn default_thinking_level(model_name: &str) -> ThinkingPreference {
    if model_name.eq_ignore_ascii_case("auto") {
        return default_thinking_level(crate::tasks::AUTO_MODEL);
    }
    match model_name.to_ascii_lowercase().as_str() {
        "gemini-4-hydrogen" => ThinkingPreference::XHigh,
        "gemini-3.8-flash"
        | "gemini-3.6-flash"
        | "gemini-3.5-flash-lite"
        | "gemini-3.1-flash-lite"
        | "gemini-3-flash-preview" => ThinkingPreference::High,
        _ => ThinkingPreference::Auto,
    }
}

pub type ModelLockMap =
    Arc<RwLock<HashMap<String, Arc<tokio::sync::Mutex<Option<std::time::Instant>>>>>>;

#[derive(Clone)]
pub struct AppState {
    /// Version of the running rs-summarizer binary.
    pub app_version: &'static str,
    pub db: SqlitePool,
    pub model_options: Arc<Vec<ModelOption>>,
    pub model_counts: Arc<RwLock<HashMap<String, u32>>>,
    pub last_reset_day: Arc<RwLock<Option<NaiveDate>>>,
    pub tpm_tracker: Arc<tokio::sync::Mutex<crate::services::rate_limiter::TpmTracker>>,
    pub gemini_api_key: String,
    #[cfg(feature = "nn-mapper")]
    pub nn_mapper: Option<std::sync::Arc<std::sync::Mutex<crate::services::nn_mapper::NnMapper>>>,
    pub viz_data: Option<std::sync::Arc<crate::models::VizData>>,
    pub model_locks: ModelLockMap,
    pub dedup_service: crate::services::deduplication::DeduplicationService,
    pub download_limiter: Arc<crate::services::download_limiter::DownloadLimiter>,
}

impl AppState {
    pub async fn get_model_lock(
        &self,
        model_name: &str,
    ) -> Arc<tokio::sync::Mutex<Option<std::time::Instant>>> {
        {
            let locks = self.model_locks.read().await;
            if let Some(lock) = locks.get(model_name) {
                return lock.clone();
            }
        }
        let mut locks = self.model_locks.write().await;
        locks
            .entry(model_name.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(None)))
            .clone()
    }
}

/// Retrieve default baseline configurations reflecting the model list.
///
/// Order and values must stay in sync with `config/models.json` (covered by
/// `test_load_models_config`). `tpm_limit: 0` means "no per-minute limit".
pub fn get_default_models() -> Vec<ModelOption> {
    vec![
        // Auto pseudo-model (resolves to AUTO_MODEL in tasks.rs)
        ModelOption {
            name: "auto".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 1_048_576,
            rpm_limit: 15,
            tpm_limit: 250_000,
            rpd_limit: 500,
            architecture: ModelArchitecture::Gemini,
        },
        // Gemini 3.5 Flash Lite (newest 500-RPD model, AUTO_MODEL target)
        ModelOption {
            name: "gemini-3.5-flash-lite".to_string(),
            input_price_per_mtoken: 0.30,
            output_price_per_mtoken: 2.50,
            context_window: 1_048_576,
            rpm_limit: 15,
            tpm_limit: 250_000,
            rpd_limit: 500,
            architecture: ModelArchitecture::Gemini,
        },
        // Gemini 3.1 Flash Lite
        ModelOption {
            name: "gemini-3.1-flash-lite".to_string(),
            input_price_per_mtoken: 0.25,
            output_price_per_mtoken: 1.50,
            context_window: 1_048_576,
            rpm_limit: 15,
            tpm_limit: 250_000,
            rpd_limit: 500,
            architecture: ModelArchitecture::Gemini,
        },
        // Gemini 4 Hydrogen (xhigh-capable flagship)
        ModelOption {
            name: "gemini-4-hydrogen".to_string(),
            input_price_per_mtoken: 1.50,
            output_price_per_mtoken: 7.50,
            context_window: 1_048_576,
            rpm_limit: 5,
            tpm_limit: 250_000,
            rpd_limit: 20,
            architecture: ModelArchitecture::Gemini,
        },
        // Gemini 3.8 Flash
        ModelOption {
            name: "gemini-3.8-flash".to_string(),
            input_price_per_mtoken: 0.75,
            output_price_per_mtoken: 3.75,
            context_window: 1_048_576,
            rpm_limit: 5,
            tpm_limit: 250_000,
            rpd_limit: 20,
            architecture: ModelArchitecture::Gemini,
        },
        // Gemini 3.6 Flash
        ModelOption {
            name: "gemini-3.6-flash".to_string(),
            input_price_per_mtoken: 0.75,
            output_price_per_mtoken: 3.75,
            context_window: 1_048_576,
            rpm_limit: 5,
            tpm_limit: 250_000,
            rpd_limit: 20,
            architecture: ModelArchitecture::Gemini,
        },
        // Gemini 3 Flash Preview
        ModelOption {
            name: "gemini-3-flash-preview".to_string(),
            input_price_per_mtoken: 0.50,
            output_price_per_mtoken: 3.00,
            context_window: 1_048_576,
            rpm_limit: 5,
            tpm_limit: 250_000,
            rpd_limit: 20,
            architecture: ModelArchitecture::Gemini,
        },
        // Gemini 2.5 Flash Lite (Interactions API rejects thinking_level)
        ModelOption {
            name: "gemini-2.5-flash-lite".to_string(),
            input_price_per_mtoken: 0.10,
            output_price_per_mtoken: 0.40,
            context_window: 1_048_576,
            rpm_limit: 10,
            tpm_limit: 250_000,
            rpd_limit: 20,
            architecture: ModelArchitecture::Gemini,
        },
        // Gemma 4 31B (Mixture-of-Experts; tight 16K TPM, no auto fallback)
        ModelOption {
            name: "gemma-4-31b-it".to_string(),
            input_price_per_mtoken: 0.07,
            output_price_per_mtoken: 0.34,
            context_window: 262_144,
            rpm_limit: 30,
            tpm_limit: 16_000,
            rpd_limit: 14400,
            architecture: ModelArchitecture::Gemma,
        },
        // Gemma 4 26B (Mixture-of-Experts; tight 16K TPM, no auto fallback)
        ModelOption {
            name: "gemma-4-26b-a4b-it".to_string(),
            input_price_per_mtoken: 0.07,
            output_price_per_mtoken: 0.34,
            context_window: 262_144,
            rpm_limit: 30,
            tpm_limit: 16_000,
            rpd_limit: 14400,
            architecture: ModelArchitecture::Gemma,
        },
        // Hetzner Qwen 3.8 27B (OpenAI-compatible experimental inference API)
        ModelOption {
            name: "hetzner-qwen-3.8-27b".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 262_144,
            rpm_limit: 60,
            tpm_limit: 0,
            rpd_limit: 14400,
            architecture: ModelArchitecture::Hetzner,
        },
        // Hetzner Qwen 3.6 35B (OpenAI-compatible experimental inference API)
        ModelOption {
            name: "hetzner-qwen-3.6-35b".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 262_144,
            rpm_limit: 60,
            tpm_limit: 0,
            rpd_limit: 14400,
            architecture: ModelArchitecture::Hetzner,
        },
    ]
}

/// Load model configurations from external JSON file with fallback logic.
pub fn load_models_config(config_path: Option<&std::path::Path>) -> Vec<ModelOption> {
    let path_buf;
    let path: &std::path::Path = match config_path {
        Some(p) => p,
        None => {
            if let Ok(env_path) = std::env::var("MODELS_CONFIG_PATH") {
                path_buf = std::path::PathBuf::from(env_path);
                &path_buf
            } else {
                std::path::Path::new("config/models.json")
            }
        }
    };

    match std::fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<Vec<ModelOption>>(&content) {
            Ok(models) => {
                tracing::info!("Loaded model configurations from {:?}", path);
                models
            }
            Err(err) => {
                tracing::warn!(
                    "Failed to parse model config at {:?}: {}. Falling back to default models.",
                    path,
                    err
                );
                get_default_models()
            }
        },
        Err(err) => {
            tracing::warn!(
                "Failed to read model config at {:?}: {}. Falling back to default models.",
                path,
                err
            );
            get_default_models()
        }
    }
}

#[cfg(test)]
mod model_checks {
    use super::*;

    #[test]
    fn test_unique_model_names() {
        let models = get_default_models();
        let mut names = std::collections::HashSet::new();
        for m in &models {
            assert!(
                names.insert(m.name.clone()),
                "Duplicate model configuration name registered: {}",
                m.name
            );
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

        // Verify Gemini 4 Hydrogen limits (xhigh-capable flagship)
        let hydrogen = models
            .iter()
            .find(|m| m.name == "gemini-4-hydrogen")
            .unwrap();
        assert_eq!(hydrogen.rpm_limit, 5);
        assert_eq!(hydrogen.rpd_limit, 20);
        assert_eq!(hydrogen.tpm_limit, 250_000);
        assert_eq!(hydrogen.context_window, 1_048_576);
        assert_eq!(hydrogen.input_price_per_mtoken, 1.50);
        assert_eq!(hydrogen.output_price_per_mtoken, 7.50);
        assert_eq!(hydrogen.architecture, ModelArchitecture::Gemini);

        // Verify Gemini 3.6 Flash limits
        let gemini_36 = models
            .iter()
            .find(|m| m.name == "gemini-3.6-flash")
            .unwrap();
        assert_eq!(gemini_36.rpm_limit, 5);
        assert_eq!(gemini_36.rpd_limit, 20);
        assert_eq!(gemini_36.tpm_limit, 250_000);

        // Verify Gemini 3.5 Flash Lite limits (AUTO_MODEL target)
        let gemini_35_lite = models
            .iter()
            .find(|m| m.name == "gemini-3.5-flash-lite")
            .unwrap();
        assert_eq!(gemini_35_lite.rpm_limit, 15);
        assert_eq!(gemini_35_lite.rpd_limit, 500);
        assert_eq!(gemini_35_lite.tpm_limit, 250_000);
        assert!(!gemini_35_lite.is_expensive());

        // Verify Gemini 3 Flash Preview pricing and TPM
        let preview = models
            .iter()
            .find(|m| m.name == "gemini-3-flash-preview")
            .unwrap();
        assert_eq!(preview.input_price_per_mtoken, 0.50);
        assert_eq!(preview.output_price_per_mtoken, 3.00);
        assert_eq!(preview.tpm_limit, 250_000);
        assert!(preview.is_expensive());

        // Verify Gemma 4 26B MoE limits including the tight TPM cap
        let gemma_4 = models
            .iter()
            .find(|m| m.name == "gemma-4-26b-a4b-it")
            .unwrap();
        assert_eq!(gemma_4.rpm_limit, 30);
        assert_eq!(gemma_4.rpd_limit, 14400);
        assert_eq!(gemma_4.tpm_limit, 16_000);
        assert!(!gemma_4.is_expensive());

        // Verify Hetzner Qwen 3.6 35B limits & architecture (no TPM cap)
        let hetzner = models
            .iter()
            .find(|m| m.name == "hetzner-qwen-3.6-35b")
            .unwrap();
        assert_eq!(hetzner.rpm_limit, 60);
        assert_eq!(hetzner.rpd_limit, 14400);
        assert_eq!(hetzner.tpm_limit, 0);
        assert_eq!(hetzner.architecture, ModelArchitecture::Hetzner);
    }

    #[test]
    fn test_deprecated_models_are_gone() {
        let models = get_default_models();
        for removed in ["gemini-3.7-flash", "gemini-3.5-flash", "gemini-2.5-flash"] {
            assert!(
                models.iter().all(|m| m.name != removed),
                "deprecated model '{}' must not be registered",
                removed
            );
        }
        // The catalog holds auto + 11 concrete models.
        assert_eq!(models.len(), 12);
    }

    #[test]
    fn test_thinking_capability_matrix() {
        use crate::models::ThinkingPreference;
        use ThinkingPreference::*;

        assert_eq!(
            supported_thinking_levels("gemini-4-hydrogen"),
            &[Auto, Low, Medium, High, XHigh]
        );
        assert_eq!(
            supported_thinking_levels("gemini-3.8-flash"),
            &[Auto, Low, Medium, High]
        );
        for lite in [
            "gemini-3.6-flash",
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite",
            "gemini-3-flash-preview",
        ] {
            assert_eq!(
                supported_thinking_levels(lite),
                &[Auto, Minimal, Low, Medium, High],
                "model {lite}"
            );
        }
        // Only auto is supported where the Interactions API rejects the level.
        for auto_only in [
            "gemini-2.5-flash-lite",
            "gemma-4-31b-it",
            "gemma-4-26b-a4b-it",
            "hetzner-qwen-3.8-27b",
            "hetzner-qwen-3.6-35b",
            "some-future-model",
        ] {
            assert_eq!(
                supported_thinking_levels(auto_only),
                &[Auto],
                "model {auto_only}"
            );
        }
        // "auto" delegates to AUTO_MODEL (gemini-3.5-flash-lite).
        assert_eq!(supported_thinking_levels("auto"), LITE_THINKING_LEVELS);
        assert_eq!(
            supported_thinking_levels("auto"),
            supported_thinking_levels(crate::tasks::AUTO_MODEL)
        );
    }

    #[test]
    fn test_default_thinking_level_is_highest_supported() {
        use crate::models::ThinkingPreference;
        assert_eq!(
            default_thinking_level("gemini-4-hydrogen"),
            ThinkingPreference::XHigh
        );
        for model in [
            "auto",
            "gemini-3.8-flash",
            "gemini-3.6-flash",
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite",
            "gemini-3-flash-preview",
        ] {
            assert_eq!(
                default_thinking_level(model),
                ThinkingPreference::High,
                "model {model}"
            );
        }
        for model in [
            "gemini-2.5-flash-lite",
            "gemma-4-31b-it",
            "hetzner-qwen-3.8-27b",
        ] {
            assert_eq!(
                default_thinking_level(model),
                ThinkingPreference::Auto,
                "model {model}"
            );
        }
    }

    #[test]
    fn test_all_hetzner_models_registered() {
        let models = get_default_models();
        let hetzner_names: Vec<&str> = vec!["hetzner-qwen-3.6-35b", "hetzner-qwen-3.8-27b"];
        for name in &hetzner_names {
            let model = models.iter().find(|m| m.name == *name);
            assert!(
                model.is_some(),
                "Hetzner model '{}' not found in defaults",
                name
            );
            let m = model.unwrap();
            assert_eq!(m.architecture, ModelArchitecture::Hetzner);
            assert_eq!(m.input_price_per_mtoken, 0.0);
            assert_eq!(m.output_price_per_mtoken, 0.0);
        }
    }

    #[test]
    fn test_hetzner_context_windows() {
        let models = get_default_models();
        let qwen36 = models
            .iter()
            .find(|m| m.name == "hetzner-qwen-3.6-35b")
            .unwrap();
        assert_eq!(qwen36.context_window, 262_144);
        let qwen38 = models
            .iter()
            .find(|m| m.name == "hetzner-qwen-3.8-27b")
            .unwrap();
        assert_eq!(qwen38.context_window, 262_144);
    }

    #[test]
    fn test_load_models_config() {
        let models = load_models_config(Some(std::path::Path::new("config/models.json")));
        let default_models = get_default_models();
        assert_eq!(models, default_models);
    }

    #[test]
    fn test_load_models_config_fallback() {
        // Test fallback on missing file
        let fallback_missing =
            load_models_config(Some(std::path::Path::new("non_existent_models_path.json")));
        let default_models = get_default_models();
        assert_eq!(fallback_missing, default_models);

        // Test fallback on invalid JSON content
        use std::io::Write;
        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        write!(temp_file, "invalid json content").unwrap();
        let fallback_invalid = load_models_config(Some(temp_file.path()));
        assert_eq!(fallback_invalid, default_models);
    }
}
