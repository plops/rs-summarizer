use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::Row;

use rs_summarizer::commands::export_db::{ExportDbArgs, run_export};
use rs_summarizer::state::{AppState, ModelOption};
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
        nn_mapper,
        viz_data,
    };

    // Build router
    let app = build_router(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 5001));
    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;

    Ok(())
}

async fn load_visualization_components() -> (
    Option<std::sync::Arc<std::sync::Mutex<rs_summarizer::services::nn_mapper::NnMapper>>>,
    Option<std::sync::Arc<rs_summarizer::models::VizData>>,
) {
    let compact_db_path = match std::env::var("COMPACT_DB_PATH") {
        Ok(path) => path,
        Err(_) => {
            tracing::info!("COMPACT_DB_PATH nicht gesetzt, Visualisierungskomponenten werden nicht geladen");
            return (None, None);
        }
    };

    tracing::info!("Lade Visualisierungskomponenten von: {}", compact_db_path);
    
    let db_path = std::path::Path::new(&compact_db_path);
    let stem = db_path.file_stem().and_then(|s| s.to_str()).unwrap_or("compact");
    let parent_dir = db_path.parent().unwrap_or(std::path::Path::new("."));

    // NN Mapper laden
    let nn_mapper = load_nn_mapper(parent_dir, stem).await;
    
    // VizData laden
    let viz_data = load_viz_data(&compact_db_path, parent_dir, stem).await;

    (nn_mapper, viz_data)
}

async fn load_nn_mapper(
    parent_dir: &std::path::Path,
    stem: &str,
) -> Option<std::sync::Arc<std::sync::Mutex<rs_summarizer::services::nn_mapper::NnMapper>>> {
    let model_path = parent_dir.join(format!("{}_nn_mapper.bin", stem));
    
    if !model_path.exists() {
        tracing::info!("NN-Mapper Datei nicht gefunden: {:?}", model_path);
        return None;
    }

    match rs_summarizer::services::nn_mapper::NnMapper::load(&model_path) {
        Ok(mapper) => {
            tracing::info!("NN-Mapper erfolgreich geladen von: {:?}", model_path);
            Some(std::sync::Arc::new(std::sync::Mutex::new(mapper)))
        }
        Err(e) => {
            tracing::error!("Fehler beim Laden des NN-Mappers: {:?}", e);
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

#[derive(sqlx::FromRow)]
struct ClusterLabel {
    identifier: i64,
    dbscan_label: Option<i32>,
}

async fn load_viz_data(
    compact_db_path: &str,
    parent_dir: &std::path::Path,
    stem: &str,
) -> Option<std::sync::Arc<rs_summarizer::models::VizData>> {
    // Zuerst die Compact DB laden
    let db = match sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(compact_db_path)
            .create_if_missing(false)
            .read_only(true)
    ).await {
        Ok(db) => db,
        Err(e) => {
            tracing::error!("Konnte Compact DB nicht öffnen: {:?}", e);
            return None;
        }
    };

    // 2D-Punkte aus der Datenbank laden
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
            tracing::error!("Fehler beim Laden der 2D-Punkte: {:?}", e);
            return None;
        }
    };

    if points_2d.is_empty() {
        tracing::info!("Keine 2D-Punkte in der Datenbank gefunden");
        return None;
    }

    // Cluster-Labels laden
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
            tracing::error!("Fehler beim Laden der Cluster-Labels: {:?}", e);
            return None;
        }
    };

    // Cluster-Titel aus JSON-Datei laden
    let titles_path = parent_dir.join(format!("{}_cluster_titles.json", stem));
    let cluster_titles: std::collections::HashMap<i32, String> = if titles_path.exists() {
        match std::fs::read_to_string(&titles_path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(titles) => {
                    tracing::info!("Cluster-Titel geladen von: {:?}", titles_path);
                    titles
                }
                Err(e) => {
                    tracing::error!("Fehler beim Parsen der Cluster-Titel: {:?}", e);
                    std::collections::HashMap::new()
                }
            },
            Err(e) => {
                tracing::error!("Fehler beim Lesen der Cluster-Titel: {:?}", e);
                std::collections::HashMap::new()
            }
        }
    } else {
        tracing::info!("Keine Cluster-Titel Datei gefunden: {:?}", titles_path);
        std::collections::HashMap::new()
    };

    // Cluster-Zentroide berechnen
    let mut cluster_centroids: std::collections::HashMap<i32, (f32, f32)> = std::collections::HashMap::new();
    let mut cluster_points: std::collections::HashMap<i32, Vec<(f32, f32)>> = std::collections::HashMap::new();

    // Punkte nach Cluster gruppieren
    for point in &points_2d {
        if let Some(&label) = cluster_labels.get(&point.identifier) {
            cluster_points.entry(label).or_default().push((point.umap_2d_x, point.umap_2d_y));
        }
    }

    // Zentroide berechnen
    for (label, points) in cluster_points {
        if !points.is_empty() {
            let sum_x: f32 = points.iter().map(|(x, _)| x).sum();
            let sum_y: f32 = points.iter().map(|(_, y)| y).sum();
            let count = points.len() as f32;
            cluster_centroids.insert(label, (sum_x / count, sum_y / count));
        }
    }

    // Konvertiere Point2D structs zu (i64, f32, f32) tuples
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
        "VizData geladen: {} Punkte, {} Cluster, {} Titel",
        viz_data.points_2d.len(),
        viz_data.cluster_labels.len(),
        viz_data.cluster_titles.len()
    );

    Some(std::sync::Arc::new(viz_data))
}

async fn handle_export_command(args: &[String]) -> anyhow::Result<()> {
    let mut source = None;
    let mut output = None;
    
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
            _ => {
                eprintln!("Error: Unknown argument '{}'", args[i]);
                eprintln!("Usage: {} export-db --source <path> --output <path>", args[0]);
                std::process::exit(1);
            }
        }
    }
    
    let source = source.ok_or_else(|| {
        eprintln!("Error: --source argument is required");
        std::process::exit(1);
        anyhow::anyhow!("Missing --source argument")
    })?;
    
    let output = output.ok_or_else(|| {
        eprintln!("Error: --output argument is required");
        std::process::exit(1);
        anyhow::anyhow!("Missing --output argument")
    })?;
    
    let export_args = ExportDbArgs { source, output };
    run_export(export_args).await?;
    
    Ok(())
}
