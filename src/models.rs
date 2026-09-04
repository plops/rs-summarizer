use std::{fmt, str::FromStr};

/// A persisted Gemini 3 thinking-effort preference.
///
/// Values intentionally use stable, lowercase database/form spellings rather
/// than provider enum names so a queued summary remains portable across SDK
/// upgrades.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize,
)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ThinkingPreference {
    Auto,
    Minimal,
    Low,
    Medium,
    #[default]
    High,
}

impl ThinkingPreference {
    pub const ALL: [Self; 5] = [
        Self::Auto,
        Self::Minimal,
        Self::Low,
        Self::Medium,
        Self::High,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl fmt::Display for ThinkingPreference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ThinkingPreference {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "minimal" => Ok(Self::Minimal),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => Err("thinking preference must be auto, minimal, low, medium, or high"),
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct Summary {
    pub identifier: i64,
    pub model: String,
    pub transcript: String,
    pub host: String,
    pub original_source_link: String,
    pub include_comments: bool,
    pub include_timestamps: bool,
    pub include_glossary: bool,
    pub output_language: String,
    pub summary: String,
    pub summary_done: bool,
    pub generation_status: String,
    pub generation_attempt: i64,
    pub generation_epoch: i64,
    pub generation_started_at: String,
    pub generation_updated_at: String,
    pub next_retry_at: String,
    pub generation_error_code: String,
    pub generation_error_message: String,
    pub provider_interaction_id: String,
    pub summary_input_tokens: i64,
    pub summary_output_tokens: i64,
    pub summary_timestamp_start: String,
    pub summary_timestamp_end: String,
    pub timestamps: String,
    pub timestamps_done: bool,
    pub timestamps_input_tokens: i64,
    pub timestamps_output_tokens: i64,
    pub timestamps_timestamp_start: String,
    pub timestamps_timestamp_end: String,
    pub timestamped_summary_in_youtube_format: String,
    pub cost: f64,
    pub embedding: Option<Vec<u8>>,
    pub embedding_model: String,
    pub full_embedding: Option<Vec<u8>>,
    pub google_search_grounding: bool,
    pub url_context: bool,
    pub thinking: String,
    pub thinking_tokens: i64,
    pub thinking_level: ThinkingPreference,
}

#[derive(Debug, serde::Deserialize)]
pub struct SubmitForm {
    pub original_source_link: String,
    pub transcript: Option<String>,
    pub model: String,
    #[serde(default)]
    pub google_search_grounding: bool,
    #[serde(default)]
    pub url_context: bool,
    #[serde(default)]
    pub include_glossary: bool,
    #[serde(default = "default_output_language")]
    pub output_language: String,
    #[serde(default)]
    pub thinking_level: ThinkingPreference,
}

fn default_output_language() -> String {
    "en".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinking_preferences_parse_and_serialize_stably() {
        for preference in ThinkingPreference::ALL {
            assert_eq!(preference.as_str().parse(), Ok(preference));
            assert_eq!(
                serde_json::to_string(&preference).unwrap(),
                format!("\"{preference}\"")
            );
        }
        assert!("maximum".parse::<ThinkingPreference>().is_err());
    }

    #[test]
    fn submit_form_omits_to_high() {
        let form: SubmitForm = serde_json::from_str(
            r#"{"original_source_link":"https://example.com","model":"gemini-3.8-flash"}"#,
        )
        .unwrap();
        assert_eq!(form.thinking_level, ThinkingPreference::High);
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct SearchForm {
    pub query: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct BrowseParams {
    pub page: Option<u32>,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct SummaryRating {
    pub id: i64,
    pub summary_id: i64,
    pub client_ip: String,
    pub summary_rating: Option<i32>,
    pub content_rating: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default, PartialEq)]
pub struct RatingStats {
    pub avg_summary_rating: f64,
    pub count_summary_rating: i64,
    pub avg_content_rating: f64,
    pub count_content_rating: i64,
    pub user_summary_rating: Option<i32>,
    pub user_content_rating: Option<i32>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SubmitRatingForm {
    pub summary_rating: Option<i32>,
    pub content_rating: Option<i32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VizData {
    pub points_2d: Vec<(i64, f32, f32)>, // (identifier, x, y)
    pub cluster_labels: std::collections::HashMap<i64, i32>, // identifier -> label
    pub cluster_titles: std::collections::HashMap<i32, String>, // label -> title
    pub cluster_centroids: std::collections::HashMap<i32, (f32, f32)>, // label -> (cx, cy)
}
