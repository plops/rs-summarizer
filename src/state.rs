use chrono::NaiveDate;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct ModelOption {
    pub name: String,
    pub input_price_per_mtoken: f64,
    pub output_price_per_mtoken: f64,
    pub context_window: u64,
    pub rpm_limit: u32,
    pub rpd_limit: u32,
}

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub model_options: Arc<Vec<ModelOption>>,
    pub model_counts: Arc<RwLock<HashMap<String, u32>>>,
    pub last_reset_day: Arc<RwLock<Option<NaiveDate>>>,
    pub gemini_api_key: String,
}
