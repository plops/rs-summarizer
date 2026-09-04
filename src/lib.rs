pub mod cache;
pub mod commands;
pub mod db;
pub mod errors;
pub mod generation;
pub mod models;
pub mod routes;
pub mod services;
pub mod state;
pub mod tasks;
pub mod templates;
pub mod utils;

/// Version of the binary as set by Cargo when it is compiled.
///
/// This deliberately has no runtime override: provenance recorded for a
/// summary must identify the artifact that generated it.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

use axum::{
    Router,
    routing::{get, post},
};
use tower_http::services::ServeDir;

use crate::state::AppState;

/// Build the application router. Used by both main.rs and integration tests.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(routes::index))
        .route("/process_transcript", post(routes::process_transcript))
        .route("/generations/{identifier}", post(routes::get_generation))
        .route(
            "/generations/{identifier}/retry",
            post(routes::retry_generation),
        )
        .route("/browse", get(routes::browse_summaries))
        .route("/summaries/{identifier}/rate", post(routes::submit_rating))
        .route("/search", post(routes::search_similar))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state)
}
