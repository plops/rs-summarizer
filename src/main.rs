use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::Row;

use rs_summarizer::commands::export_db::{ExportDbArgs, run_export};
use rs_summarizer::state::AppState;
use rs_summarizer::{build_router, db};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("rs-summarizer starting up");

    // Check for export-db CLI command
    let args: Vec<String> = env::args().collect();
    if args.len() >= 2 && args[1] == "export-db" {
        return handle_export_command(&args).await;
    }

    // Load Gemini API key from environment
    let gemini_api_key = std::env::var("GEMINI_API_KEY")
        .unwrap_or_else(|_| {
            tracing::warn!("GEMINI_API_KEY not set, API calls will fail");
            String::new()
        });

    // Initialize database
    let db = db::init_db("sqlite:data/summaries.db").await?;

    // Load visualization data and NN mapper if COMPACT_DB_PATH is set
    let (nn_mapper, viz_data) = load_visualization_components().await;

    // Configure model options
    let model_options = rs_summarizer::state::get_default_models();

    // Build application state
    let state = AppState {
        db: db.clone(),
        model_options: Arc::new(model_options),
        model_counts: Arc::new(RwLock::new(HashMap::new())),
        last_reset_day: Arc::new(RwLock::new(None)),
        gemini_api_key,
        nn_mapper,
        viz_data,
        model_locks: Arc::new(RwLock::new(HashMap::new())),
        dedup_service: rs_summarizer::services::deduplication::DeduplicationService::new(
            std::time::Duration::from_secs(300),
        ),
    };

    // Build router
    let app = build_router(state);

    // Start server
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "5001".to_string());
    let addr_str = format!("{}:{}", host, port);
    let addr: SocketAddr = addr_str
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid socket address '{addr_str}': {e}"))?;

    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Perform database cleanup on graceful shutdown
    tracing::info!("Cleaning up database connections...");
    let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE);")
        .execute(&db)
        .await;
    db.close().await;
    tracing::info!("Shutdown complete");

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn load_visualization_components() -> (
    Option<std::sync::Arc<std::sync::Mutex<rs_summarizer::services::nn_mapper::NnMapper>>>,
    Option<std::sync::Arc<rs_summarizer::models::VizData>>,
) {
    let compact_db_path = match std::env::var("COMPACT_DB_PATH") {
        Ok(path) => path,
        Err(_) => {
            tracing::info!("COMPACT_DB_PATH not set, visualization components will not be loaded");
            return (None, None);
        }
    };

    tracing::info!("Loading visualization components from: {}", compact_db_path);
    
    let db_path = std::path::Path::new(&compact_db_path);
    let stem = db_path.file_stem().and_then(|s| s.to_str()).unwrap_or("compact");
    let parent_dir = db_path.parent().unwrap_or(std::path::Path::new("."));

    // Load NN Mapper
    let nn_mapper = load_nn_mapper(parent_dir, stem).await;
    
    // Load VizData
    let viz_data = load_viz_data(&compact_db_path, parent_dir, stem).await;

    (nn_mapper, viz_data)
}

async fn load_nn_mapper(
    parent_dir: &std::path::Path,
    stem: &str,
) -> Option<std::sync::Arc<std::sync::Mutex<rs_summarizer::services::nn_mapper::NnMapper>>> {
    let model_path = parent_dir.join(format!("{}_nn_mapper.bin", stem));
    
    if !model_path.exists() {
        tracing::info!("NN mapper file not found: {:?}", model_path);
        return None;
    }

    match rs_summarizer::services::nn_mapper::NnMapper::load(&model_path) {
        Ok(mapper) => {
            tracing::info!("NN mapper loaded successfully from: {:?}", model_path);
            Some(std::sync::Arc::new(std::sync::Mutex::new(mapper)))
        }
        Err(e) => {
            tracing::error!("Error loading NN mapper: {:?}", e);
            None
        }
    }
}

// Temporary structs for SQL queries
#[derive(sqlx::FromRow)]
struct Point2D {
    identifier: i64,
    umap_2d_x: f32,
    umap_2d_y: f32,
}


async fn load_viz_data(
    compact_db_path: &str,
    parent_dir: &std::path::Path,
    stem: &str,
) -> Option<std::sync::Arc<rs_summarizer::models::VizData>> {
    // Connect to Compact DB
    let db = match sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(compact_db_path)
            .create_if_missing(false)
            .read_only(true)
    ).await {
        Ok(db) => db,
        Err(e) => {
            tracing::error!("Could not open Compact DB: {:?}", e);
            return None;
        }
    };

    // Load 2D points from database
    let points_2d: Vec<Point2D> = match sqlx::query("SELECT identifier, umap_2d_x, umap_2d_y FROM summaries WHERE umap_2d_x IS NOT NULL AND umap_2d_y IS NOT NULL")
        .fetch_all(&db)
        .await {
        Ok(rows) => {
            let mut points = Vec::new();
            for row in rows {
                let point = Point2D {
                    identifier: row.get("identifier"),
                    umap_2d_x: row.get("umap_2d_x"),
                    umap_2d_y: row.get("umap_2d_y"),
                };
                points.push(point);
            }
            points
        }
        Err(e) => {
            tracing::error!("Error loading 2D points: {:?}", e);
            return None;
        }
    };

    if points_2d.is_empty() {
        tracing::info!("No 2D points found in database");
        return None;
    }

    // Load cluster labels
    let cluster_labels: std::collections::HashMap<i64, i32> = match sqlx::query("SELECT identifier, dbscan_label FROM summaries WHERE dbscan_label IS NOT NULL")
        .fetch_all(&db)
        .await {
        Ok(rows) => {
            let mut labels = std::collections::HashMap::new();
            for row in rows {
                let identifier: i64 = row.get("identifier");
                let dbscan_label: Option<i32> = row.get("dbscan_label");
                if let Some(label) = dbscan_label {
                    labels.insert(identifier, label);
                }
            }
            labels
        }
        Err(e) => {
            tracing::error!("Error loading cluster labels: {:?}", e);
            return None;
        }
    };

    // Load cluster titles from JSON file
    let titles_path = parent_dir.join(format!("{}_cluster_titles.json", stem));
    let cluster_titles: std::collections::HashMap<i32, String> = if titles_path.exists() {
        match tokio::fs::read_to_string(&titles_path).await {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(titles) => {
                    tracing::info!("Cluster titles loaded from: {:?}", titles_path);
                    titles
                }
                Err(e) => {
                    tracing::error!("Error parsing cluster titles: {:?}", e);
                    std::collections::HashMap::new()
                }
            },
            Err(e) => {
                tracing::error!("Error reading cluster titles: {:?}", e);
                std::collections::HashMap::new()
            }
        }
    } else {
        tracing::info!("No cluster titles file found: {:?}", titles_path);
        std::collections::HashMap::new()
    };

    // Calculate cluster centroids
    let mut cluster_centroids: std::collections::HashMap<i32, (f32, f32)> = std::collections::HashMap::new();
    let mut cluster_points: std::collections::HashMap<i32, Vec<(f32, f32)>> = std::collections::HashMap::new();

    // Group points by cluster
    for point in &points_2d {
        if let Some(&label) = cluster_labels.get(&point.identifier) {
            cluster_points.entry(label).or_default().push((point.umap_2d_x, point.umap_2d_y));
        }
    }

    // Calculate centroids
    for (label, points) in cluster_points {
        if !points.is_empty() {
            let sum_x: f32 = points.iter().map(|(x, _)| x).sum();
            let sum_y: f32 = points.iter().map(|(_, y)| y).sum();
            let count = points.len() as f32;
            cluster_centroids.insert(label, (sum_x / count, sum_y / count));
        }
    }

    // Convert Point2D structs to (i64, f32, f32) tuples
    let points_2d_tuples: Vec<(i64, f32, f32)> = points_2d.into_iter()
        .map(|point| (point.identifier, point.umap_2d_x, point.umap_2d_y))
        .collect();

    let viz_data = rs_summarizer::models::VizData {
        points_2d: points_2d_tuples,
        cluster_labels,
        cluster_titles,
        cluster_centroids,
    };

    tracing::info!(
        "VizData loaded: {} points, {} clusters, {} titles",
        viz_data.points_2d.len(),
        viz_data.cluster_labels.len(),
        viz_data.cluster_titles.len()
    );

    Some(std::sync::Arc::new(viz_data))
}

async fn handle_export_command(args: &[String]) -> anyhow::Result<()> {
    let mut source = None;
    let mut output = None;
    let mut include_embeddings = false;
    let mut compress = false;
    
    let mut i = 2; // Skip "export-db"
    while i < args.len() {
        match args[i].as_str() {
            "--source" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --source requires a path argument");
                    std::process::exit(1);
                }
                source = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--output" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --output requires a path argument");
                    std::process::exit(1);
                }
                output = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--include-embeddings" => {
                include_embeddings = true;
                i += 1;
            }
            "--compress" => {
                compress = true;
                i += 1;
            }
            _ => {
                eprintln!("Error: Unknown argument '{}'", args[i]);
                eprintln!("Usage: {} export-db --source <path> --output <path> [--include-embeddings] [--compress]", args[0]);
                std::process::exit(1);
            }
        }
    }
    
    let source = source.ok_or_else(|| {
        anyhow::anyhow!("--source argument is required")
    })?;
    
    let output = output.ok_or_else(|| {
        anyhow::anyhow!("--output argument is required")
    })?;
    
    let export_args = ExportDbArgs {
        source,
        output,
        include_embeddings,
        compress,
    };
    run_export(export_args).await?;
    
    Ok(())
}
