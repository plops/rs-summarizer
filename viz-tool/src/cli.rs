use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "viz-tool")]
#[command(about = "Embedding Visualization Tool")]
pub struct Cli {
    /// Database file path
    pub database: PathBuf,

    /// Embedding dimension
    #[arg(short, long, default_value = "768")]
    pub embedding_dim: usize,

    /// Output format
    #[arg(short, long, default_value = "text", value_parser = ["text", "json", "csv"])]
    pub output_format: String,

    /// Output file path (optional, defaults to stdout)
    #[arg(short, long)]
    pub output_file: Option<PathBuf>,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Load and validate database
    Load,
    /// Run UMAP dimensionality reduction
    Umap(UmapArgs),
    /// Run DBSCAN clustering
    Cluster(ClusterArgs),
    /// Run complete pipeline
    Pipeline(PipelineArgs),
}

#[derive(Parser, Clone)]
pub struct UmapArgs {
    /// Target dimensions (2 or 4)
    #[arg(short, long, default_value = "2", value_parser = clap::value_parser!(u8).range(2..=4))]
    pub components: u8,

    /// Number of neighbors
    #[arg(short, long, default_value = "15")]
    pub neighbors: usize,

    /// Minimum distance
    #[arg(short, long, default_value = "0.1")]
    pub min_dist: f32,

    /// Training epochs
    #[arg(short, long, default_value = "200")]
    pub epochs: usize,
}

#[derive(Parser, Clone)]
pub struct ClusterArgs {
    /// Epsilon parameter
    #[arg(short, long, default_value = "0.3")]
    pub eps: f64,

    /// Minimum samples
    #[arg(short, long, default_value = "5")]
    pub min_samples: usize,
}

#[derive(Parser, Clone)]
pub struct PipelineArgs {
    /// UMAP target dimensions (2 or 4)
    #[arg(long, default_value = "2", value_parser = clap::value_parser!(u8).range(2..=4))]
    pub umap_components: u8,

    /// UMAP number of neighbors
    #[arg(long, default_value = "15")]
    pub umap_neighbors: usize,

    /// UMAP minimum distance
    #[arg(long, default_value = "0.1")]
    pub umap_min_dist: f32,

    /// UMAP training epochs
    #[arg(long, default_value = "200")]
    pub umap_epochs: usize,

    /// DBSCAN epsilon parameter
    #[arg(long, default_value = "0.3")]
    pub dbscan_eps: f64,

    /// DBSCAN minimum samples
    #[arg(long, default_value = "5")]
    pub dbscan_min_samples: usize,

    /// Skip cluster title generation
    #[arg(long)]
    pub skip_titles: bool,

    /// Skip NN mapper training
    #[arg(long)]
    pub skip_nn_mapper: bool,
}
