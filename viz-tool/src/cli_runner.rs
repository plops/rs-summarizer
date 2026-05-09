use std::path::Path;
use std::fs::File;
use std::io::{self, Write};
use tokio::runtime::Runtime;
use crate::cli::{Cli, Commands};
use crate::data_loader::load_compact_db;
use crate::umap_engine::{UmapParams, compute_umap};
use crate::dbscan_engine::{DbscanParams, compute_dbscan};
use crate::cluster_titler::generate_titles;
use crate::nn_mapper::NnMapper;
use crate::errors::VizError;

pub fn run_cli(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let rt = Runtime::new()?;
    
    match cli.command {
        Some(Commands::Load) => {
            rt.block_on(async {
                run_load_command(&cli).await
            })?
        }
        Some(Commands::Umap(ref umap_args)) => {
            rt.block_on(async {
                run_umap_command(&cli, umap_args.clone()).await
            })?
        }
        Some(Commands::Cluster(ref cluster_args)) => {
            rt.block_on(async {
                run_cluster_command(&cli, cluster_args.clone()).await
            })?
        }
        Some(Commands::Pipeline(ref pipeline_args)) => {
            rt.block_on(async {
                run_pipeline_command(&cli, pipeline_args.clone()).await
            })?
        }
        None => {
            // No subcommand - launch GUI (original behavior)
            return run_gui(cli);
        }
    }
    
    Ok(())
}

fn run_gui(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    use eframe::{egui, NativeOptions};
    use crate::viz_app::VizApp;
    
    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Embedding Visualization Tool",
        native_options,
        Box::new(|_cc| {
            let app = VizApp::new(Some(cli.database), cli.embedding_dim);
            Ok(Box::new(app))
        }),
    )?;
    
    Ok(())
}

async fn run_load_command(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let result = load_compact_db(&cli.database, cli.embedding_dim).await?;
    
    let output = match cli.output_format.as_str() {
        "json" => serde_json::json!({
            "load_result": {
                "points_loaded": result.points.len(),
                "skipped_invalid": result.skipped_invalid_length,
                "skipped_too_short": result.skipped_too_short
            }
        }).to_string(),
        "csv" => {
            let mut wtr = csv::Writer::from_writer(io::stdout());
            for point in &result.points {
                wtr.write_record(&[
                    point.identifier.to_string(),
                    point.original_source_link.clone(),
                    point.summary.clone(),
                    point.model.clone(),
                    point.embedding_model.clone(),
                ])?;
            }
            wtr.flush()?;
            String::new()
        }
        _ => format!(
            "Loaded {} embeddings from database\nSkipped {} invalid blobs\nSkipped {} too short blobs",
            result.points.len(),
            result.skipped_invalid_length,
            result.skipped_too_short
        ),
    };
    
    write_output(cli, &output)?;
    Ok(())
}

async fn run_umap_command(cli: &Cli, umap_args: crate::cli::UmapArgs) -> Result<(), Box<dyn std::error::Error>> {
    let load_result = load_compact_db(&cli.database, cli.embedding_dim).await?;
    let embeddings: Vec<Vec<f32>> = load_result.points.iter()
        .map(|p| p.embedding.clone())
        .collect();
    
    let params = UmapParams {
        n_components: umap_args.components as usize,
        n_neighbors: umap_args.neighbors,
        min_dist: umap_args.min_dist,
        n_epochs: umap_args.epochs,
    };
    
    let reduced_embeddings = match compute_umap(&embeddings, params) {
        Ok(result) => result,
        Err(e) => {
            if cli.verbose {
                eprintln!("GPU UMAP failed, using CPU fallback: {}", e);
            }
            // Simple CPU fallback using random projection for testing
            compute_cpu_umap_fallback(&embeddings, umap_args.components as usize)?
        }
    };
    
    let output = match cli.output_format.as_str() {
        "json" => serde_json::json!({
            "umap_result": {
                "input_dimensions": cli.embedding_dim,
                "output_dimensions": umap_args.components,
                "points_processed": reduced_embeddings.len(),
                "embeddings": reduced_embeddings
            }
        }).to_string(),
        "csv" => {
            let mut wtr = csv::Writer::from_writer(io::stdout());
            wtr.write_record(&["id"])?;
            for i in 1..=umap_args.components {
                wtr.write_field(&format!("dim{}", i))?;
            }
            wtr.write_record(None::<&str>)?;
            
            for (i, embedding) in reduced_embeddings.iter().enumerate() {
                let mut record = vec![i.to_string()];
                record.extend(embedding.iter().map(|v| v.to_string()));
                wtr.write_record(&record)?;
            }
            wtr.flush()?;
            String::new()
        }
        _ => format!(
            "UMAP reduction completed: {} points -> {}D\nFirst few points: {:?}",
            reduced_embeddings.len(),
            umap_args.components,
            reduced_embeddings.get(0..3.min(reduced_embeddings.len()))
        ),
    };
    
    write_output(cli, &output)?;
    Ok(())
}

async fn run_cluster_command(cli: &Cli, cluster_args: crate::cli::ClusterArgs) -> Result<(), Box<dyn std::error::Error>> {
    let load_result = load_compact_db(&cli.database, cli.embedding_dim).await?;
    let embeddings: Vec<Vec<f32>> = load_result.points.iter()
        .map(|p| p.embedding.clone())
        .collect();
    
    // First run UMAP to get 4D embeddings for clustering
    let umap_params = UmapParams {
        n_components: 4,
        n_neighbors: 15,
        min_dist: 0.1,
        n_epochs: 200,
    };
    
    let embeddings_4d_vec = compute_umap(&embeddings, umap_params)?;
    let embeddings_4d: Vec<[f32; 4]> = embeddings_4d_vec.into_iter()
        .map(|v| {
            let mut arr = [0.0f32; 4];
            for (i, val) in v.into_iter().take(4).enumerate() {
                arr[i] = val;
            }
            arr
        })
        .collect();
    
    let dbscan_params = DbscanParams {
        eps: cluster_args.eps,
        min_samples: cluster_args.min_samples,
    };
    
    let labels = compute_dbscan(&embeddings_4d, dbscan_params)?;
    let n_clusters = labels.iter().filter(|&&l| l >= 0).max().map(|m| m + 1).unwrap_or(0);
    let noise_points = labels.iter().filter(|&&l| l == -1).count();
    
    let output = match cli.output_format.as_str() {
        "json" => serde_json::json!({
            "cluster_result": {
                "n_clusters": n_clusters,
                "noise_points": noise_points,
                "total_points": labels.len(),
                "labels": labels
            }
        }).to_string(),
        "csv" => {
            let mut wtr = csv::Writer::from_writer(io::stdout());
            wtr.write_record(&["id", "cluster_id"])?;
            for (i, label) in labels.iter().enumerate() {
                wtr.write_record(&[i.to_string(), label.to_string()])?;
            }
            wtr.flush()?;
            String::new()
        }
        _ => format!(
            "DBSCAN clustering completed: {} clusters found\n{} noise points out of {} total",
            n_clusters, noise_points, labels.len()
        ),
    };
    
    write_output(cli, &output)?;
    Ok(())
}

async fn run_pipeline_command(cli: &Cli, pipeline_args: crate::cli::PipelineArgs) -> Result<(), Box<dyn std::error::Error>> {
    let load_result = load_compact_db(&cli.database, cli.embedding_dim).await?;
    let embeddings: Vec<Vec<f32>> = load_result.points.iter()
        .map(|p| p.embedding.clone())
        .collect();
    
    // UMAP reduction
    let umap_params = UmapParams {
        n_components: pipeline_args.umap_components as usize,
        n_neighbors: pipeline_args.umap_neighbors,
        min_dist: pipeline_args.umap_min_dist,
        n_epochs: pipeline_args.umap_epochs,
    };
    
    let reduced_embeddings = compute_umap(&embeddings, umap_params.clone())?;
    
    // For clustering, always use 4D
    let umap_4d_params = UmapParams {
        n_components: 4,
        n_neighbors: pipeline_args.umap_neighbors,
        min_dist: pipeline_args.umap_min_dist,
        n_epochs: pipeline_args.umap_epochs,
    };
    
    let embeddings_4d_vec = compute_umap(&embeddings, umap_4d_params)?;
    let embeddings_4d: Vec<[f32; 4]> = embeddings_4d_vec.into_iter()
        .map(|v| {
            let mut arr = [0.0f32; 4];
            for (i, val) in v.into_iter().take(4).enumerate() {
                arr[i] = val;
            }
            arr
        })
        .collect();
    
    // DBSCAN clustering
    let dbscan_params = DbscanParams {
        eps: pipeline_args.dbscan_eps,
        min_samples: pipeline_args.dbscan_min_samples,
    };
    
    let labels = compute_dbscan(&embeddings_4d, dbscan_params)?;
    let n_clusters = labels.iter().filter(|&&l| l >= 0).max().map(|m| m + 1).unwrap_or(0);
    let noise_points = labels.iter().filter(|&&l| l == -1).count();
    
    let output = match cli.output_format.as_str() {
        "json" => serde_json::json!({
            "pipeline_result": {
                "load_result": {
                    "points_loaded": load_result.points.len(),
                    "skipped_invalid": load_result.skipped_invalid_length,
                    "skipped_too_short": load_result.skipped_too_short
                },
                "umap_result": {
                    "input_dimensions": cli.embedding_dim,
                    "output_dimensions": pipeline_args.umap_components,
                    "points_processed": reduced_embeddings.len()
                },
                "cluster_result": {
                    "n_clusters": n_clusters,
                    "noise_points": noise_points,
                    "total_points": labels.len()
                }
            }
        }).to_string(),
        _ => format!(
            "Pipeline completed successfully:\n\
             - Loaded {} embeddings\n\
             - UMAP reduction: {}D -> {}D\n\
             - DBSCAN clustering: {} clusters found\n\
             - {} noise points out of {} total",
            load_result.points.len(),
            cli.embedding_dim,
            pipeline_args.umap_components,
            n_clusters,
            noise_points,
            labels.len()
        ),
    };
    
    write_output(cli, &output)?;
    Ok(())
}

/// Simple CPU fallback for UMAP using random projection
/// This is a basic implementation for testing when GPU fails
fn compute_cpu_umap_fallback(embeddings: &[Vec<f32>], n_components: usize) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
    if embeddings.is_empty() {
        return Ok(Vec::new());
    }
    
    let n_points = embeddings.len();
    let input_dim = embeddings[0].len();
    
    // Simple random projection matrix
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut projection: Vec<Vec<f32>> = Vec::with_capacity(n_components);
    
    for _ in 0..n_components {
        let mut component = Vec::with_capacity(input_dim);
        for _ in 0..input_dim {
            component.push(rng.gen_range(-1.0..1.0));
        }
        projection.push(component);
    }
    
    // Apply projection
    let mut result: Vec<Vec<f32>> = Vec::with_capacity(n_points);
    for embedding in embeddings {
        let mut reduced = Vec::with_capacity(n_components);
        for component in &projection {
            let mut sum = 0.0;
            for (i, &val) in embedding.iter().enumerate() {
                if i < component.len() {
                    sum += val * component[i];
                }
            }
            reduced.push(sum);
        }
        result.push(reduced);
    }
    
    Ok(result)
}

fn write_output(cli: &Cli, output: &str) -> Result<(), Box<dyn std::error::Error>> {
    match &cli.output_file {
        Some(path) => {
            let mut file = File::create(path)?;
            file.write_all(output.as_bytes())?;
            if cli.verbose {
                eprintln!("Output written to {}", path.display());
            }
        }
        None => {
            print!("{}", output);
        }
    }
    Ok(())
}
