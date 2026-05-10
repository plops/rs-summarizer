pub mod data_loader;
pub mod embedding;
pub mod errors;
pub mod umap_engine;

// Optional/advanced modules enabled when "gui" feature is active
#[cfg(feature = "gui")]
pub mod viz_app;

// Other advanced modules may be added behind features as needed
//#[cfg(feature = "dbscan")]
//pub mod dbscan_engine;
//#[cfg(feature = "titles")]
//pub mod cluster_titler;
//#[cfg(feature = "nn-mapper")]
//pub mod nn_mapper;

pub mod cli;
pub mod cli_runner;
