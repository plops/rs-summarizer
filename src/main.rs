mod cache;
mod db;
mod errors;
mod models;
mod routes;
mod services;
mod state;
mod tasks;
mod templates;
mod utils;

use axum::{routing::{get, post}, Router};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::services::ServeDir;

use crate::state::{AppState, ModelOption};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("rs-summarizer starting up");

    // Load Gemini API key from environment
    let gemini_api_key = std::env::var("GEMINI_API_KEY")
        .unwrap_or_else(|_| {
            tracing::warn!("GEMINI_API_KEY not set, API calls will fail");
            String::new()
        });

    // Initialize database
    let db = db::init_db("sqlite:data/summaries.db").await?;

    // Configure model options
    let model_options = vec![
        ModelOption {
            name: "gemini-3-flash-preview".to_string(),
            input_price_per_mtoken: 0.10,
            output_price_per_mtoken: 0.40,
            context_window: 1_000_000,
            rpm_limit: 5,
            rpd_limit: 20,
        },
        ModelOption {
            name: "gemini-3.1-flash-lite-preview".to_string(),
            input_price_per_mtoken: 0.075,
            output_price_per_mtoken: 0.30,
            context_window: 1_000_000,
            rpm_limit: 15,
            rpd_limit: 500,
        },
        ModelOption {
            name: "gemini-2.5-flash".to_string(),
            input_price_per_mtoken: 0.15,
            output_price_per_mtoken: 0.60,
            context_window: 1_000_000,
            rpm_limit: 5,
            rpd_limit: 20,
        },
        ModelOption {
            name: "gemini-2.5-flash-lite".to_string(),
            input_price_per_mtoken: 0.075,
            output_price_per_mtoken: 0.30,
            context_window: 1_000_000,
            rpm_limit: 10,
            rpd_limit: 20,
        },
        ModelOption {
            name: "gemma-4-31b-it".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 128_000,
            rpm_limit: 15,
            rpd_limit: 1500,
        },
        ModelOption {
            name: "gemma-4-26b-a4b-it".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 128_000,
            rpm_limit: 15,
            rpd_limit: 1500,
        },
        ModelOption {
            name: "gemma-3-27b-it".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 128_000,
            rpm_limit: 30,
            rpd_limit: 14400,
        },
        ModelOption {
            name: "gemma-3-12b-it".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 128_000,
            rpm_limit: 30,
            rpd_limit: 14400,
        },
        ModelOption {
            name: "gemma-3-4b-it".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 128_000,
            rpm_limit: 30,
            rpd_limit: 14400,
        },
        ModelOption {
            name: "gemma-3-1b-it".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 128_000,
            rpm_limit: 30,
            rpd_limit: 14400,
        },
    ];

    // Build application state
    let state = AppState {
        db,
        model_options: Arc::new(model_options),
        model_counts: Arc::new(RwLock::new(HashMap::new())),
        last_reset_day: Arc::new(RwLock::new(None)),
        gemini_api_key,
    };

    // Build router
    let app = Router::new()
        .route("/", get(routes::index))
        .route("/process_transcript", post(routes::process_transcript))
        .route("/generations/{identifier}", post(routes::get_generation))
        .route("/browse", get(routes::browse_summaries))
        .route("/search", post(routes::search_similar))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 5001));
    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;

    Ok(())
}
