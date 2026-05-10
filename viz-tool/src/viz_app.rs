use eframe::{egui, App};
use egui_plot::{Plot, PlotPoints, Points};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::data_loader::{load_compact_db, load_compact_db_subset, EmbeddingPoint};
use crate::umap_engine::{compute_umap, UmapParams};

#[derive(Debug, Clone, PartialEq)]
enum AppStatus {
    Idle,
    Loading,
    ComputingUmap,
}

#[derive(Debug, Clone)]
enum ComputeResult {
    LoadDone {
        points: Vec<EmbeddingPoint>,
        skipped: usize,
    },
    Umap2dDone {
        embeddings_2d: Vec<[f32; 2]>,
        params: UmapParams,
    },
    Error(String),
}

pub struct VizApp {
    // Data
    db_path: Option<PathBuf>,
    points: Vec<EmbeddingPoint>,
    embedding_dim: usize,

    // Results
    embeddings_2d: Option<Vec<[f32; 2]>>,
    // Plotting
    point_radius: f32,
    // Last used UMAP params (for display)
    last_umap_params: Option<UmapParams>,

    // UI state
    status: AppStatus,
    error_message: Option<String>,
    skipped_blobs: usize,
    max_points: usize,

    // UMAP params
    umap_neighbors: usize,
    umap_min_dist: f32,
    umap_epochs: usize,

    // Background worker channel
    compute_tx: Sender<ComputeResult>,
    compute_rx: Receiver<ComputeResult>,
}

impl Default for VizApp {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            db_path: None,
            points: Vec::new(),
            embedding_dim: 768,
            embeddings_2d: None,
            point_radius: 2.0,
            last_umap_params: None,
            status: AppStatus::Idle,
            error_message: None,
            skipped_blobs: 0,
            max_points: 0,
            umap_neighbors: 12,
            umap_min_dist: 0.13,
            umap_epochs: 200,
            compute_tx: tx,
            compute_rx: rx,
        }
    }
}

impl App for VizApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process background results
        self.process_compute_results();

        // Top bar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.heading("Embedding Visualization Tool (minimal)");
            match self.status {
                AppStatus::Idle => {
                    if !self.points.is_empty() {
                        ui.label(format!(
                            "Status: {} points loaded, {} skipped",
                            self.points.len(),
                            self.skipped_blobs
                        ));
                    } else {
                        ui.label("Status: Ready");
                    }
                }
                AppStatus::Loading => {
                    ui.label("Status: Loading...");
                }
                AppStatus::ComputingUmap => {
                    ui.label("Status: Computing UMAP...");
                }
            }

            if let Some(ref err) = self.error_message {
                ui.colored_label(egui::Color32::RED, err.clone());
            }
        });

        // Left panel: controls
        egui::SidePanel::left("control_panel").show(ctx, |ui| {
            ui.heading("Controls");

            ui.horizontal(|ui| {
                ui.label("Max points");
                ui.add(egui::Slider::new(&mut self.max_points, 0..=50000).text("max_points"));

                if ui.button("Load DB").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("SQLite DB", &["db"])
                        .pick_file()
                    {
                        self.db_path = Some(path.clone());
                        self.start_loading(path);
                    }
                }
            });

            ui.separator();

            ui.heading("UMAP 2D");
            ui.add(egui::Slider::new(&mut self.umap_neighbors, 2..=200).text("n_neighbors"));
            ui.add(egui::Slider::new(&mut self.umap_min_dist, 0.0..=1.0).text("min_dist"));
            ui.horizontal(|ui| {
                ui.label("epochs");
                ui.add(egui::DragValue::new(&mut self.umap_epochs));
            });

            ui.add(egui::Slider::new(&mut self.point_radius, 0.5..=8.0).text("point radius"));

            let umap_enabled = !self.points.is_empty() && self.status == AppStatus::Idle;
            if ui
                .add_enabled(umap_enabled, egui::Button::new("Compute 2D UMAP"))
                .clicked()
            {
                self.start_umap_2d_computation();
            }

            if let Some(ref p) = self.last_umap_params {
                ui.label(format!(
                    "Last UMAP: neighbors={}, min_dist={}, epochs={}",
                    p.n_neighbors, p.min_dist, p.n_epochs
                ));
                if let Some(ref emb) = self.embeddings_2d {
                    ui.label(format!("Points: {}", emb.len()));
                }
            }
        });

        // Central panel: plot
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Scatter Plot (2D UMAP)");

            if let Some(ref embeddings) = self.embeddings_2d {
                // Prepare points as f64 for egui_plot
                let points: Vec<[f64; 2]> = embeddings
                    .iter()
                    .map(|p| [p[0] as f64, p[1] as f64])
                    .collect();

                Plot::new("umap_scatter").show(ui, |plot_ui| {
                    let plot_points: PlotPoints = points.into_iter().collect();
                    let points_item = Points::new("UMAP 2D", plot_points)
                        .color(egui::Color32::LIGHT_BLUE)
                        .radius(self.point_radius);
                    plot_ui.points(points_item);
                });
            } else {
                ui.label("No UMAP results yet. Load a DB and click 'Compute 2D UMAP'.");
            }
        });

        // Request repaint while working
        if self.status != AppStatus::Idle {
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Backward-compat helper: eframe expects `ui` method on some versions
    }
}

impl VizApp {
    pub fn new(db_path: Option<PathBuf>, embedding_dim: usize) -> Self {
        let mut app = Self::default();
        app.db_path = db_path;
        app.embedding_dim = embedding_dim;
        if let Some(ref path) = app.db_path {
            app.start_loading(path.clone());
        }
        app
    }

    fn start_loading(&mut self, path: PathBuf) {
        self.status = AppStatus::Loading;
        self.error_message = None;

        let embedding_dim = self.embedding_dim;
        let max_points = self.max_points;
        let tx = self.compute_tx.clone();

        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let res = if max_points > 0 {
                    load_compact_db_subset(&path, embedding_dim, max_points).await
                } else {
                    load_compact_db(&path, embedding_dim).await
                };

                match res {
                    Ok(result) => {
                        let _ = tx.send(ComputeResult::LoadDone {
                            points: result.points,
                            skipped: result.skipped_invalid_length + result.skipped_too_short,
                        });
                    }
                    Err(e) => {
                        let _ = tx.send(ComputeResult::Error(format!("Failed to load DB: {}", e)));
                    }
                }
            });
        });
    }

    fn start_umap_2d_computation(&mut self) {
        if self.points.is_empty() {
            return;
        }

        self.status = AppStatus::ComputingUmap;
        self.error_message = None;

        // Prepare embeddings; honor max_points as a subset if requested
        let embeddings: Vec<Vec<f32>> =
            if self.max_points > 0 && self.max_points < self.points.len() {
                self.points
                    .iter()
                    .take(self.max_points)
                    .map(|p| p.embedding.clone())
                    .collect()
            } else {
                self.points.iter().map(|p| p.embedding.clone()).collect()
            };

        let params = UmapParams {
            n_components: 2,
            n_neighbors: self.umap_neighbors,
            min_dist: self.umap_min_dist,
            n_epochs: self.umap_epochs,
        };
        let tx = self.compute_tx.clone();

        // Clone params for compute and for reporting
        let params_for_compute = params.clone();
        let params_for_result = params.clone();

        thread::spawn(move || {
            match compute_umap(&embeddings, params_for_compute) {
                Ok(emb) => {
                    // Convert to fixed size arrays
                    let emb2d: Vec<[f32; 2]> = emb
                        .into_iter()
                        .map(|v| {
                            let mut arr = [0.0f32; 2];
                            for (i, val) in v.into_iter().take(2).enumerate() {
                                arr[i] = val;
                            }
                            arr
                        })
                        .collect();
                    let _ = tx.send(ComputeResult::Umap2dDone {
                        embeddings_2d: emb2d,
                        params: params_for_result,
                    });
                }
                Err(e) => {
                    let _ = tx.send(ComputeResult::Error(format!("UMAP failed: {}", e)));
                }
            }
        });
    }

    fn process_compute_results(&mut self) {
        while let Ok(result) = self.compute_rx.try_recv() {
            match result {
                ComputeResult::LoadDone { points, skipped } => {
                    self.points = points;
                    self.skipped_blobs = skipped;
                    self.status = AppStatus::Idle;
                    self.embeddings_2d = None;
                    self.error_message = None;
                }
                ComputeResult::Umap2dDone {
                    embeddings_2d,
                    params,
                } => {
                    self.embeddings_2d = Some(embeddings_2d);
                    self.last_umap_params = Some(params);
                    self.status = AppStatus::Idle;
                }
                ComputeResult::Error(msg) => {
                    self.error_message = Some(msg);
                    self.status = AppStatus::Idle;
                }
            }
        }
    }
}
