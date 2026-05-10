use crate::cli::{Cli, Commands};
use crate::data_loader::{load_compact_db, load_compact_db_subset};
use crate::dbscan_engine::{compute_dbscan, DbscanParams};
use crate::umap_engine::{compute_umap, UmapParams};
use std::fs::File;
use std::io::{self, Write};
use tokio::runtime::Runtime;

pub fn run_cli(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let rt = Runtime::new()?;

    match &cli.command {
        Some(cmd) => match cmd {
            Commands::Load => rt.block_on(async { run_load_command(&cli).await })?,
            Commands::Umap2D(umap_args) => {
                let args = umap_args.clone();
                rt.block_on(async { run_umap_2d_command(&cli, args).await })?
            }
            Commands::Umap4D(umap_args) => {
                let args = umap_args.clone();
                rt.block_on(async { run_umap_4d_command(&cli, args).await })?
            }
            Commands::Umap(umap_args) => {
                let args = umap_args.clone();
                // Allow both 2D and 4D via the generic Umap command
                rt.block_on(async { run_umap_command(&cli, args).await })?
            }
            Commands::Cluster(cluster_args) => {
                let args = cluster_args.clone();
                rt.block_on(async { run_cluster_command(&cli, args).await })?
            }
            Commands::Pipeline(pipeline_args) => {
                let args = pipeline_args.clone();
                rt.block_on(async { run_pipeline_command(&cli, args).await })?
            }
            _ => {
                return Err("This command is not supported in the minimal build".into());
            }
        },
        None => {
            // No subcommand. Launch GUI only when compiled with the "gui" feature.
            #[cfg(feature = "gui")]
            {
                return run_gui(cli);
            }

            #[cfg(not(feature = "gui"))]
            {
                eprintln!(
                    "No subcommand provided and GUI not enabled. Use --help to see available commands."
                );
                return Err("No subcommand provided".into());
            }
        }
    }

    Ok(())
}

// GUI launcher is only compiled when the "gui" feature is enabled
#[cfg(feature = "gui")]
fn run_gui(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    use crate::viz_app::VizApp;
    use eframe::{egui, NativeOptions};

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
        })
        .to_string(),
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

async fn run_umap_command(
    cli: &Cli,
    umap_args: crate::cli::UmapArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let load_result = load_compact_db(&cli.database, cli.embedding_dim).await?;
    let embeddings: Vec<Vec<f32>> = load_result
        .points
        .iter()
        .map(|p| p.embedding.clone())
        .collect();

    let params = UmapParams {
        n_components: umap_args.components as usize,
        n_neighbors: umap_args.neighbors,
        min_dist: umap_args.min_dist,
        n_epochs: umap_args.epochs,
        learning_rate: 1e-3,
        hidden_sizes: vec![100, 100, 100],
    };

    let reduced_embeddings = match compute_umap(&embeddings, params, None, None) {
        Ok(result) => result,
        Err(e) => {
            if cli.verbose {
                eprintln!("UMAP failed: {}", e);
            }
            return Err(Box::new(e));
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
        })
        .to_string(),
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

async fn run_umap_2d_command(
    cli: &Cli,
    umap_args: crate::cli::Umap2DArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let load_result = if umap_args.subset > 0 {
        load_compact_db_subset(&cli.database, cli.embedding_dim, umap_args.subset).await?
    } else {
        load_compact_db(&cli.database, cli.embedding_dim).await?
    };

    let embeddings: Vec<Vec<f32>> = load_result
        .points
        .iter()
        .map(|p| p.embedding.clone())
        .collect();

    let params = UmapParams {
        n_components: 2,
        n_neighbors: umap_args.neighbors,
        min_dist: umap_args.min_dist,
        n_epochs: umap_args.epochs,
        learning_rate: 1e-3,
        hidden_sizes: vec![100, 100, 100],
    };

    let reduced_embeddings = match compute_umap(&embeddings, params, None, None) {
        Ok(result) => result,
        Err(e) => {
            if cli.verbose {
                eprintln!("UMAP failed: {}", e);
            }
            return Err(Box::new(e));
        }
    };

    let output = match cli.output_format.as_str() {
        "json" => serde_json::json!({
            "umap_2d_result": {
                "input_dimensions": cli.embedding_dim,
                "output_dimensions": 2,
                "points_processed": reduced_embeddings.len(),
                "subset_used": umap_args.subset,
                "embeddings": reduced_embeddings
            }
        })
        .to_string(),
        _ => {
            format!(
                "UMAP 2D reduction completed: {} points -> 2D\nSubset: {} points\nFirst few points: {:?}",
                reduced_embeddings.len(),
                if umap_args.subset > 0 { umap_args.subset } else { load_result.points.len() },
                &reduced_embeddings[..reduced_embeddings.len().min(3)]
            )
        }
    };

    write_output(cli, &output)?;
    Ok(())
}

async fn run_umap_4d_command(
    cli: &Cli,
    umap_args: crate::cli::Umap4DArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let load_result = if umap_args.subset > 0 {
        load_compact_db_subset(&cli.database, cli.embedding_dim, umap_args.subset).await?
    } else {
        load_compact_db(&cli.database, cli.embedding_dim).await?
    };

    let embeddings: Vec<Vec<f32>> = load_result
        .points
        .iter()
        .map(|p| p.embedding.clone())
        .collect();

    let params = UmapParams {
        n_components: 4,
        n_neighbors: umap_args.neighbors,
        min_dist: umap_args.min_dist,
        n_epochs: umap_args.epochs,
        learning_rate: 1e-3,
        hidden_sizes: vec![100, 100, 100],
    };

    let reduced_embeddings = match compute_umap(&embeddings, params, None, None) {
        Ok(result) => result,
        Err(e) => {
            if cli.verbose {
                eprintln!("UMAP failed: {}", e);
            }
            return Err(Box::new(e));
        }
    };

    let output = match cli.output_format.as_str() {
        "json" => serde_json::json!({
            "umap_4d_result": {
                "input_dimensions": cli.embedding_dim,
                "output_dimensions": 4,
                "points_processed": reduced_embeddings.len(),
                "subset_used": umap_args.subset,
                "embeddings": reduced_embeddings
            }
        })
        .to_string(),
        "csv" => {
            let mut wtr = csv::Writer::from_writer(io::stdout());
            wtr.write_record(&["id", "dim1", "dim2", "dim3", "dim4"])?;
            for (i, embedding) in reduced_embeddings.iter().enumerate() {
                if embedding.len() != 4 {
                    return Err("UMAP did not produce 4D embeddings".into());
                }
                let mut record = vec![i.to_string()];
                record.extend(embedding.iter().map(|v| v.to_string()));
                wtr.write_record(&record)?;
            }
            wtr.flush()?;
            String::new()
        }
        _ => format!(
            "UMAP 4D reduction completed: {} points -> 4D\nSubset: {} points\nFirst few points: {:?}",
            reduced_embeddings.len(),
            if umap_args.subset > 0 { umap_args.subset } else { load_result.points.len() },
            &reduced_embeddings[..reduced_embeddings.len().min(3)]
        ),
    };

    write_output(cli, &output)?;
    Ok(())
}

async fn run_cluster_command(
    cli: &Cli,
    cluster_args: crate::cli::ClusterArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let load_result = if cluster_args.subset > 0 {
        load_compact_db_subset(&cli.database, cli.embedding_dim, cluster_args.subset).await?
    } else {
        load_compact_db(&cli.database, cli.embedding_dim).await?
    };

    let embeddings: Vec<Vec<f32>> = load_result
        .points
        .iter()
        .map(|p| p.embedding.clone())
        .collect();

    // Compute 4D UMAP before clustering
    let params = UmapParams {
        n_components: 4,
        n_neighbors: cluster_args.neighbors,
        min_dist: cluster_args.min_dist,
        n_epochs: cluster_args.epochs,
        learning_rate: 1e-3,
        hidden_sizes: vec![100, 100, 100],
    };

    let reduced_embeddings = match compute_umap(&embeddings, params, None, None) {
        Ok(result) => result,
        Err(e) => {
            if cli.verbose {
                eprintln!("UMAP (for clustering) failed: {}", e);
            }
            return Err(Box::new(e));
        }
    };

    if reduced_embeddings.is_empty() {
        let output = "No embeddings available for clustering.".to_string();
        write_output(cli, &output)?;
        return Ok(());
    }

    // Convert to fixed 4D arrays for DBSCAN
    let mut embeddings_4d = Vec::with_capacity(reduced_embeddings.len());
    for v in &reduced_embeddings {
        if v.len() != 4 {
            return Err("UMAP did not return 4D embeddings required for DBSCAN".into());
        }
        embeddings_4d.push([v[0], v[1], v[2], v[3]]);
    }

    let db_params = DbscanParams {
        eps: cluster_args.eps,
        min_samples: cluster_args.min_samples,
    };

    let labels = match compute_dbscan(&embeddings_4d, db_params) {
        Ok(l) => l,
        Err(e) => {
            if cli.verbose {
                eprintln!("DBSCAN failed: {}", e);
            }
            return Err(Box::new(e));
        }
    };

    let output = match cli.output_format.as_str() {
        "json" => serde_json::json!({
            "cluster_result": {
                "points_processed": labels.len(),
                "labels": labels
            }
        })
        .to_string(),
        "csv" => {
            let mut wtr = csv::Writer::from_writer(io::stdout());
            wtr.write_record(&["id", "dim1", "dim2", "dim3", "dim4", "cluster"])?;
            for (i, emb) in embeddings_4d.iter().enumerate() {
                let mut record = vec![
                    i.to_string(),
                    emb[0].to_string(),
                    emb[1].to_string(),
                    emb[2].to_string(),
                    emb[3].to_string(),
                    labels[i].to_string(),
                ];
                wtr.write_record(&record)?;
            }
            wtr.flush()?;
            String::new()
        }
        _ => {
            let noise_count = labels.iter().filter(|&&l| l == -1).count();
            let mut counts = std::collections::HashMap::new();
            for &l in &labels {
                *counts.entry(l).or_insert(0usize) += 1;
            }
            format!(
                "DBSCAN clustering completed: {} points; clusters (including noise -1): {:?}; noise_count: {}",
                labels.len(),
                counts,
                noise_count
            )
        }
    };

    write_output(cli, &output)?;
    Ok(())
}

async fn run_pipeline_command(
    cli: &Cli,
    args: crate::cli::PipelineArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    // Run UMAP with the requested components
    let load_result = load_compact_db(&cli.database, cli.embedding_dim).await?;
    let embeddings: Vec<Vec<f32>> = load_result
        .points
        .iter()
        .map(|p| p.embedding.clone())
        .collect();

    let params = UmapParams {
        n_components: args.umap_components as usize,
        n_neighbors: args.umap_neighbors,
        min_dist: args.umap_min_dist,
        n_epochs: args.umap_epochs,
        learning_rate: 1e-3,
        hidden_sizes: vec![100, 100, 100],
    };

    let reduced_embeddings = match compute_umap(&embeddings, params, None, None) {
        Ok(r) => r,
        Err(e) => {
            if cli.verbose {
                eprintln!("UMAP (pipeline) failed: {}", e);
            }
            return Err(Box::new(e));
        }
    };

    // If user requested 4D UMAP, run DBSCAN as part of pipeline
    let mut labels_opt: Option<Vec<i32>> = None;
    if args.umap_components == 4 {
        let mut embeddings_4d = Vec::with_capacity(reduced_embeddings.len());
        for v in &reduced_embeddings {
            if v.len() != 4 {
                return Err("UMAP did not return 4D embeddings required for DBSCAN".into());
            }
            embeddings_4d.push([v[0], v[1], v[2], v[3]]);
        }

        let db_params = DbscanParams {
            eps: args.dbscan_eps,
            min_samples: args.dbscan_min_samples,
        };

        let lbls = match compute_dbscan(&embeddings_4d, db_params) {
            Ok(l) => l,
            Err(e) => {
                if cli.verbose {
                    eprintln!("DBSCAN (pipeline) failed: {}", e);
                }
                return Err(Box::new(e));
            }
        };
        labels_opt = Some(lbls);
    }

    let output = match cli.output_format.as_str() {
        "json" => serde_json::json!({
            "pipeline_result": {
                "umap_components": args.umap_components,
                "points_processed": reduced_embeddings.len(),
                "embeddings": reduced_embeddings,
                "clusters": labels_opt
            }
        })
        .to_string(),
        _ => format!(
            "Pipeline completed: UMAP {}D on {} points; DBSCAN run: {}",
            args.umap_components,
            reduced_embeddings.len(),
            if labels_opt.is_some() { "yes" } else { "no" }
        ),
    };

    write_output(cli, &output)?;
    Ok(())
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
