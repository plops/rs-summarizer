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
}

#[derive(Debug, serde::Deserialize)]
pub struct SubmitForm {
    pub original_source_link: String,
    pub transcript: Option<String>,
    pub model: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct SearchForm {
    pub query: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct BrowseParams {
    pub page: Option<u32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VizData {
    pub points_2d: Vec<(i64, f32, f32)>,           // (identifier, x, y)
    pub cluster_labels: std::collections::HashMap<i64, i32>,          // identifier -> label
    pub cluster_titles: std::collections::HashMap<i32, String>,       // label -> title
    pub cluster_centroids: std::collections::HashMap<i32, (f32, f32)>, // label -> (cx, cy)
}
