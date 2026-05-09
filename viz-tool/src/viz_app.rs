use eframe::{egui, App as EframeApp};
use egui_plot::{Plot, Points, PlotPoints, Text, PlotPoint};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::collections::HashMap;
use std::thread;
use crate::data_loader::{EmbeddingPoint, load_compact_db};
use crate::umap_engine::{UmapParams, compute_umap};
use crate::dbscan_engine::{DbscanParams, compute_dbscan};
use crate::cluster_titler::{generate_titles, save_titles, load_titles};
use crate::nn_mapper::{NnMapper};
use crate::errors::VizError;

/// Application status states
#[derive(Debug, Clone, PartialEq)]
pub enum AppStatus {
    Idle,
    Loading,
    ComputingUmap,
    ComputingDbscan,
    GeneratingTitles,
    TrainingNnMapper,
}

/// Results from background computations
#[derive(Debug, Clone)]
pub enum ComputeResult {
    LoadDone { points: Vec<EmbeddingPoint>, skipped: usize },
    UmapDone { embeddings_4d: Vec<[f32; 4]>, embeddings_2d: Vec<[f32; 2]> },
    DbscanDone { labels: Vec<i32>, n_clusters: usize },
    TitlesDone { titles: HashMap<i32, String> },
    NnMapperDone,
    Error(String),
}

/// Main application state
pub struct VizApp {
    // Data fields
    db_path: Option<PathBuf>,
    points: Vec<EmbeddingPoint>,
    embedding_dim: usize,

    // UMAP parameters
    umap4d_neighbors: usize,
    umap4d_min_dist: f32,
    umap2d_neighbors: usize,
    umap2d_min_dist: f32,
    
    // Computed results
    embeddings_4d: Option<Vec<[f32; 4]>>,
    embeddings_2d: Option<Vec<[f32; 2]>>,
    cluster_labels: Option<Vec<i32>>,
    
    // DBSCAN parameters
    dbscan_eps: f32,
    dbscan_min_samples: usize,
    
    // Cluster titles and NN mapper
    cluster_titles: HashMap<i32, String>,
    nn_mapper: Option<NnMapper>,
    
    // UI state
    status: AppStatus,
    error_message: Option<String>,
    skipped_blobs: usize,
    compute_tx: Sender<ComputeResult>,
    compute_rx: Receiver<ComputeResult>,
}

impl Default for VizApp {
    fn default() -> Self {
        let (compute_tx, compute_rx) = mpsc::channel();
        
        Self {
            db_path: None,
            points: Vec::new(),
            embedding_dim: 768, // Default embedding dimension
            
            umap4d_neighbors: 5,
            umap4d_min_dist: 0.1,
            umap2d_neighbors: 12,
            umap2d_min_dist: 0.13,
            
            embeddings_4d: None,
            embeddings_2d: None,
            cluster_labels: None,
            
            dbscan_eps: 0.3,
            dbscan_min_samples: 5,
            
            cluster_titles: HashMap::new(),
            nn_mapper: None,
            
            status: AppStatus::Idle,
            error_message: None,
            skipped_blobs: 0,
            compute_tx,
            compute_rx,
        }
    }
}

impl EframeApp for VizApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process background computation results
        self.process_compute_results();
        
        // Request repaint if we're processing something
        if self.status != AppStatus::Idle {
            ctx.request_repaint();
        }
        
        // Main layout: left control panel + right scatter plot
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.heading("Embedding Visualization Tool");
            
            // Status banner
            match self.status {
                AppStatus::Idle => {
                    if !self.points.is_empty() {
                        ui.label(format!("Status: {} Punkte geladen, {} übersprungen", 
                            self.points.len(), self.skipped_blobs));
                    } else {
                        ui.label("Status: Bereit zum Laden");
                    }
                }
                AppStatus::Loading => {
                    ui.label("Status: Lade Daten...");
                }
                AppStatus::ComputingUmap => {
                    ui.label("Status: Berechne UMAP...");
                }
                AppStatus::ComputingDbscan => {
                    ui.label("Status: Berechne DBSCAN...");
                }
                AppStatus::GeneratingTitles => {
                    ui.label("Status: Generiere Cluster-Titel...");
                }
                AppStatus::TrainingNnMapper => {
                    ui.label("Status: Trainiere NN-Mapper...");
                }
            }
            
            // Error message banner
            if let Some(ref error) = self.error_message {
                ui.colored_label(egui::Color32::RED, format!("Fehler: {}", error));
            }
        });
        
        // Left control panel
        egui::SidePanel::left("control_panel").show(ctx, |ui| {
            ui.heading("Steuerung");
            
            // File loading
            if ui.button("Datenbank laden").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SQLite Datenbank", &["db"])
                    .pick_file() 
                {
                    self.db_path = Some(path.clone());
                    self.start_loading(path);
                }
            }
            
            ui.separator();
            
            // UMAP 4D parameters
            ui.heading("UMAP 4D");
            ui.add(egui::Slider::new(&mut self.umap4d_neighbors, 2..=200)
                .text("n_neighbors"));
            ui.add(egui::Slider::new(&mut self.umap4d_min_dist, 0.0..=1.0)
                .text("min_dist"));
            
            // UMAP 2D parameters  
            ui.heading("UMAP 2D");
            ui.add(egui::Slider::new(&mut self.umap2d_neighbors, 2..=200)
                .text("n_neighbors"));
            ui.add(egui::Slider::new(&mut self.umap2d_min_dist, 0.0..=1.0)
                .text("min_dist"));
            
            // UMAP compute button
            let umap_button_enabled = !self.points.is_empty() && self.status == AppStatus::Idle;
            if ui.add_enabled(umap_button_enabled, egui::Button::new("Berechnen")).clicked() {
                self.start_umap_computation();
            }
            
            // Progress indicator
            if self.status == AppStatus::ComputingUmap {
                ui.spinner();
                ui.label("Berechne UMAP...");
            }
            
            ui.separator();
            
            // DBSCAN parameters
            ui.heading("DBSCAN");
            
            // eps input with validation
            let eps_valid = self.dbscan_eps > 0.0;
            if eps_valid {
                ui.add(egui::Slider::new(&mut self.dbscan_eps, 0.01..=2.0)
                    .text("eps"));
            } else {
                ui.colored_label(egui::Color32::RED, "eps muss > 0.0 sein");
                ui.add(egui::Slider::new(&mut self.dbscan_eps, 0.01..=2.0)
                    .text("eps"));
            }
            
            ui.add(egui::Slider::new(&mut self.dbscan_min_samples, 1..=50)
                .text("min_samples"));
            
            // DBSCAN button and cluster count
            let dbscan_enabled = self.embeddings_4d.is_some() && eps_valid && self.status == AppStatus::Idle;
            if ui.add_enabled(dbscan_enabled, egui::Button::new("Clustern")).clicked() {
                self.start_dbscan_computation();
            }
            
            if let Some(ref labels) = self.cluster_labels {
                let n_clusters = labels.iter()
                    .filter(|&&label| label >= 0)
                    .collect::<std::collections::HashSet<_>>()
                    .len();
                ui.label(format!("{} Cluster gefunden", n_clusters));
            }
            
            ui.separator();
            
            // Cluster titles button
            let titles_enabled = self.cluster_labels.is_some() && self.status == AppStatus::Idle;
            if ui.add_enabled(titles_enabled, egui::Button::new("Cluster-Titel generieren")).clicked() {
                self.start_title_generation();
            }
            
            // NN Mapper training button
            let nn_mapper_enabled = self.embeddings_2d.is_some() && self.status == AppStatus::Idle;
            if ui.add_enabled(nn_mapper_enabled, egui::Button::new("NN-Mapper trainieren")).clicked() {
                self.start_nn_mapper_training();
            }
        });
        
        // Right scatter plot area
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Scatter Plot");
            
            if let (Some(ref embeddings_2d), Some(ref labels)) = (&self.embeddings_2d, &self.cluster_labels) {
                self.show_scatter_plot(ui, embeddings_2d, labels);
            } else {
                ui.label("Keine Daten zur Visualisierung verfügbar. Bitte laden Sie eine Datenbank und berechnen Sie UMAP.");
            }
        });
    }
    
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // This method is required by the trait but we use update() instead
    }
}

impl VizApp {
    /// Create a new VizApp with optional initial database path and embedding dimension
    pub fn new(db_path: Option<PathBuf>, embedding_dim: usize) -> Self {
        let mut app = Self::default();
        app.db_path = db_path;
        app.embedding_dim = embedding_dim;
        
        // Auto-load database if path provided
        if let Some(ref path) = app.db_path {
            app.start_loading(path.clone());
        }
        
        app
    }
    
    /// Auto-load saved artifacts for a database
    fn auto_load_artifacts(&mut self, db_path: &PathBuf) {
        // Get the stem (filename without extension) for artifact files
        let stem = db_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("database");
        
        let titles_path = db_path.parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join(format!("{}_cluster_titles.json", stem));
        
        let nn_mapper_path = db_path.parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join(format!("{}_nn_mapper.bin", stem));
        
        // Load cluster titles if they exist
        if titles_path.exists() {
            match load_titles(&titles_path) {
                Ok(titles) => {
                    self.cluster_titles = titles;
                    println!("Loaded cluster titles from {}", titles_path.display());
                }
                Err(e) => {
                    eprintln!("Failed to load cluster titles: {}", e);
                }
            }
        }
        
        // Load NN mapper if it exists
        if nn_mapper_path.exists() {
            match NnMapper::load(&nn_mapper_path, self.embedding_dim) {
                Ok(nn_mapper) => {
                    self.nn_mapper = Some(nn_mapper);
                    println!("Loaded NN mapper from {}", nn_mapper_path.display());
                }
                Err(e) => {
                    eprintln!("Failed to load NN mapper: {}", e);
                }
            }
        }
    }
    
    /// Save cluster titles to file
    fn save_cluster_titles(&self, db_path: &PathBuf) -> Result<(), VizError> {
        if self.cluster_titles.is_empty() {
            return Ok(());
        }
        
        let stem = db_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("database");
        
        let titles_path = db_path.parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join(format!("{}_cluster_titles.json", stem));
        
        save_titles(&self.cluster_titles, &titles_path)
    }
    
    /// Save NN mapper to file
    fn save_nn_mapper(&self, db_path: &PathBuf) -> Result<(), VizError> {
        if let Some(ref nn_mapper) = self.nn_mapper {
            let stem = db_path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("database");
            
            let nn_mapper_path = db_path.parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join(format!("{}_nn_mapper.bin", stem));
            
            nn_mapper.save(&nn_mapper_path)?;
        }
        
        Ok(())
    }
    
    /// Start loading a compact database in background thread
    fn start_loading(&mut self, path: PathBuf) {
        self.status = AppStatus::Loading;
        self.error_message = None;
        
        let embedding_dim = self.embedding_dim;
        let tx = self.compute_tx.clone();
        
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                match load_compact_db(&path, embedding_dim).await {
                    Ok(result) => {
                        let _ = tx.send(ComputeResult::LoadDone {
                            points: result.points,
                            skipped: result.skipped_invalid_length + result.skipped_too_short,
                        });
                    }
                    Err(e) => {
                        let _ = tx.send(ComputeResult::Error(format!("Failed to load database: {}", e)));
                    }
                }
            });
        });
    }
    
    /// Start UMAP computation in background thread
    fn start_umap_computation(&mut self) {
        if self.points.is_empty() {
            return;
        }
        
        self.status = AppStatus::ComputingUmap;
        self.error_message = None;
        
        let embeddings: Vec<Vec<f32>> = self.points.iter()
            .map(|p| p.embedding.clone())
            .collect();
        
        let umap4d_params = UmapParams {
            n_components: 4,
            n_neighbors: self.umap4d_neighbors,
            min_dist: self.umap4d_min_dist,
            n_epochs: 200,
        };
        
        let umap2d_params = UmapParams {
            n_components: 2,
            n_neighbors: self.umap2d_neighbors,
            min_dist: self.umap2d_min_dist,
            n_epochs: 200,
        };
        
        let tx = self.compute_tx.clone();
        
        thread::spawn(move || {
            // Compute 4D UMAP
            let embeddings_4d_result = compute_umap(&embeddings, umap4d_params);
            let embeddings_2d_result = compute_umap(&embeddings, umap2d_params);
            
            match (embeddings_4d_result, embeddings_2d_result) {
                (Ok(embeddings_4d), Ok(embeddings_2d)) => {
                    // Convert to fixed-size arrays
                    let embeddings_4d_fixed: Vec<[f32; 4]> = embeddings_4d.into_iter()
                        .map(|v: Vec<f32>| v.try_into().unwrap_or([0.0; 4]))
                        .collect();
                    let embeddings_2d_fixed: Vec<[f32; 2]> = embeddings_2d.into_iter()
                        .map(|v: Vec<f32>| v.try_into().unwrap_or([0.0; 2]))
                        .collect();
                    
                    let _ = tx.send(ComputeResult::UmapDone {
                        embeddings_4d: embeddings_4d_fixed,
                        embeddings_2d: embeddings_2d_fixed,
                    });
                }
                (Err(e1), Err(e2)) => {
                    let _ = tx.send(ComputeResult::Error(
                        format!("UMAP computation failed - 4D: {}, 2D: {}", e1, e2)
                    ));
                }
                (Err(e), _) | (_, Err(e)) => {
                    let _ = tx.send(ComputeResult::Error(
                        format!("UMAP computation failed: {}", e)
                    ));
                }
            }
        });
    }
    
    /// Start DBSCAN computation in background thread
    fn start_dbscan_computation(&mut self) {
        if let Some(ref embeddings_4d) = self.embeddings_4d {
            self.status = AppStatus::ComputingDbscan;
            self.error_message = None;
            
            let embeddings = embeddings_4d.clone();
            let dbscan_params = DbscanParams {
                eps: self.dbscan_eps as f64,
                min_samples: self.dbscan_min_samples,
            };
            
            let tx = self.compute_tx.clone();
            
            thread::spawn(move || {
                match compute_dbscan(&embeddings, dbscan_params) {
                    Ok(labels) => {
                        let n_clusters: usize = labels.iter()
                            .filter(|&&label| label >= 0)
                            .collect::<std::collections::HashSet<_>>()
                            .len();
                        
                        let _ = tx.send(ComputeResult::DbscanDone { labels, n_clusters });
                    }
                    Err(e) => {
                        let _ = tx.send(ComputeResult::Error(format!("DBSCAN computation failed: {}", e)));
                    }
                }
            });
        }
    }
    
    /// Start cluster title generation in background thread
    fn start_title_generation(&mut self) {
        if let Some(ref labels) = self.cluster_labels {
            self.status = AppStatus::GeneratingTitles;
            self.error_message = None;
            
            let points = self.points.clone();
            let labels = labels.clone();
            let tx = self.compute_tx.clone();
            
            thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    // This would need API key and model name - for now use placeholder
                    match generate_titles(&points, &labels, "dummy_api_key", "gemini-pro").await {
                        Ok(titles) => {
                            let _ = tx.send(ComputeResult::TitlesDone { titles });
                        }
                        Err(e) => {
                            let _ = tx.send(ComputeResult::Error(format!("Title generation failed: {}", e)));
                        }
                    }
                });
            });
        }
    }
    
    /// Start NN mapper training in background thread
    fn start_nn_mapper_training(&mut self) {
        if let Some(ref embeddings_2d) = self.embeddings_2d {
            self.status = AppStatus::TrainingNnMapper;
            self.error_message = None;
            
            let embeddings: Vec<Vec<f32>> = self.points.iter()
                .map(|p| p.embedding.clone())
                .collect();
            
            let umap_params = UmapParams {
                n_components: 2,
                n_neighbors: self.umap2d_neighbors,
                min_dist: self.umap2d_min_dist,
                n_epochs: 200,
            };
            
            let embedding_dim = self.embedding_dim;
            let tx = self.compute_tx.clone();
            
            thread::spawn(move || {
                match NnMapper::train(&embeddings, embedding_dim, umap_params) {
                    Ok(nn_mapper) => {
                        // In a real implementation, we would save the mapper here
                        let _ = tx.send(ComputeResult::NnMapperDone);
                    }
                    Err(e) => {
                        let _ = tx.send(ComputeResult::Error(format!("NN mapper training failed: {}", e)));
                    }
                }
            });
        }
    }
    
    /// Show scatter plot with cluster visualization
    fn show_scatter_plot(&self, ui: &mut egui::Ui, embeddings_2d: &[[f32; 2]], labels: &[i32]) {
        let plot = Plot::new("umap_scatter")
            .label_formatter(|name, value| {
                // Find the point index from the name (cluster_0, cluster_1, etc.)
                if let Some(cluster_str) = name.strip_prefix("cluster_") {
                    if let Ok(cluster_id) = cluster_str.parse::<i32>() {
                        // Find a point from this cluster to show its info
                        for (i, &label) in labels.iter().enumerate() {
                            if label == cluster_id && i < self.points.len() {
                                let point = &self.points[i];
                                return format!(
                                    "{}\n{}\nx: {:.3}, y: {:.3}",
                                    name,
                                    point.original_source_link.chars().take(50).collect::<String>(),
                                    value.x,
                                    value.y
                                );
                            }
                        }
                    }
                }
                format!("{}\nx: {:.3}, y: {:.3}", name, value.x, value.y)
            })
            .show(ui, |plot_ui| {
                // Color palette for clusters
                let colors = vec![
                    egui::Color32::BLUE,
                    egui::Color32::RED,
                    egui::Color32::GREEN,
                    egui::Color32::YELLOW,
                    egui::Color32::from_rgb(255, 128, 0), // Orange
                    egui::Color32::from_rgb(128, 0, 255), // Purple
                    egui::Color32::from_rgb(0, 255, 255), // Cyan
                    egui::Color32::from_rgb(255, 0, 255), // Magenta
                    egui::Color32::from_rgb(128, 128, 128), // Gray (for noise)
                ];
                
                // Group points by cluster
                let mut cluster_points: std::collections::HashMap<i32, Vec<[f64; 2]>> = std::collections::HashMap::new();
                
                for (i, &embedding) in embeddings_2d.iter().enumerate() {
                    let label = if i < labels.len() { labels[i] } else { -1 };
                    let point = [embedding[0] as f64, embedding[1] as f64];
                    cluster_points.entry(label).or_insert_with(Vec::new).push(point);
                }
                
                // Plot each cluster
                for (cluster_id, points) in cluster_points {
                    let color = if cluster_id == -1 {
                        colors[8] // Gray for noise
                    } else {
                        colors[(cluster_id as usize) % colors.len()]
                    };
                    
                    let cluster_name = if cluster_id == -1 {
                        "Rauschen".to_string()
                    } else {
                        // Get cluster title if available
                        self.cluster_titles.get(&cluster_id)
                            .cloned()
                            .unwrap_or_else(|| format!("Cluster {}", cluster_id))
                    };
                    
                    let plot_points: PlotPoints = points.clone().into_iter().collect();
                    
                    plot_ui.points(
                        Points::new(cluster_name.clone(), plot_points)
                            .color(color)
                            .radius(3.0)
                    );
                    
                    // Add centroid label for non-noise clusters
                    if cluster_id != -1 && !points.is_empty() {
                        let centroid = points.iter().fold(
                            [0.0_f64, 0.0_f64],
                            |acc, point| [acc[0] + point[0], acc[1] + point[1]]
                        );
                        let centroid = [centroid[0] / points.len() as f64, centroid[1] / points.len() as f64];
                        
                        plot_ui.text(
                            Text::new(
                                cluster_name.clone(),
                                PlotPoint::new(centroid[0], centroid[1]),
                                cluster_name
                            )
                            .color(color)
                            .anchor(egui::Align2::CENTER_BOTTOM)
                        );
                    }
                }
            });
    }
    
    /// Process results from background computations
    fn process_compute_results(&mut self) {
        while let Ok(result) = self.compute_rx.try_recv() {
            match result {
                ComputeResult::LoadDone { points, skipped } => {
                    self.points = points;
                    self.skipped_blobs = skipped;
                    self.status = AppStatus::Idle;
                    
                    // Clear previous results
                    self.embeddings_4d = None;
                    self.embeddings_2d = None;
                    self.cluster_labels = None;
                    self.cluster_titles.clear();
                    self.nn_mapper = None;
                    
                    // Auto-load saved artifacts
                    if let Some(db_path) = self.db_path.clone() {
                        self.auto_load_artifacts(&db_path);
                    }
                }
                ComputeResult::UmapDone { embeddings_4d, embeddings_2d } => {
                    self.embeddings_4d = Some(embeddings_4d);
                    self.embeddings_2d = Some(embeddings_2d);
                    self.status = AppStatus::Idle;
                }
                ComputeResult::DbscanDone { labels, n_clusters: _ } => {
                    self.cluster_labels = Some(labels);
                    self.status = AppStatus::Idle;
                }
                ComputeResult::TitlesDone { titles } => {
                    self.cluster_titles = titles;
                    self.status = AppStatus::Idle;
                    
                    // Save titles to file
                    if let Some(ref db_path) = self.db_path {
                        if let Err(e) = self.save_cluster_titles(db_path) {
                            eprintln!("Failed to save cluster titles: {}", e);
                        }
                    }
                }
                ComputeResult::NnMapperDone => {
                    self.status = AppStatus::Idle;
                    // In a real implementation, we would store the trained mapper
                    // Save NN mapper to file
                    if let Some(ref db_path) = self.db_path {
                        if let Err(e) = self.save_nn_mapper(db_path) {
                            eprintln!("Failed to save NN mapper: {}", e);
                        }
                    }
                }
                ComputeResult::Error(msg) => {
                    self.error_message = Some(msg);
                    self.status = AppStatus::Idle;
                }
            }
        }
    }
}
