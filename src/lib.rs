pub mod cache;
pub mod db;
pub mod errors;
pub mod models;
pub mod routes;
pub mod services;
pub mod state;
pub mod tasks;
pub mod templates;
pub mod utils;

use axum::{routing::{get, post}, Router};
use tower_http::services::ServeDir;

use crate::state::AppState;

/// Build the application router. Used by both main.rs and integration tests.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(routes::index))
        .route("/process_transcript", post(routes::process_transcript))
        .route("/generations/{identifier}", post(routes::get_generation))
        .route("/browse", get(routes::browse_summaries))
        .route("/search", post(routes::search_similar))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state)
}
