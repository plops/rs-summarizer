mod errors;
mod models;
mod state;

use tracing_subscriber;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("rs-summarizer starting up");
}
