use eframe::{egui, App};
use egui_plot::{Plot, PlotPoints, Points};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crossbeam_channel;

use crate::data_loader::{load_compact_db, load_compact_db_subset, EmbeddingPoint};
use crate::umap_engine::{compute_umap, ProgressUpdate, UmapParams};

#[derive(Debug, Copy, Clone, PartialEq)]
enum UmapMode {
    Classic,
    Parametric,
}

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
    last_umap_mode: Option<UmapMode>,

    // UI state
    status: AppStatus,
    error_message: Option<String>,
    skipped_blobs: usize,
    max_points: usize,
    // Auto recompute
    auto_recompute: bool,
    pending_recompute: bool,

    // UMAP params
    umap_mode: UmapMode,
    /// log10(learning_rate) slider value (e.g. -3.0 -> 1e-3)
    umap_lr_log: f32,
    hidden_sizes_str: String,
    umap_neighbors: usize,
    umap_min_dist: f32,
    umap_epochs: usize,

    // Background worker channel
    compute_tx: Sender<ComputeResult>,
    compute_rx: Receiver<ComputeResult>,

    // Progress updates for GUI
    progress: Option<ProgressUpdate>,
    progress_rx: Option<crossbeam_channel::Receiver<ProgressUpdate>>,
    // Cancellation sender to signal compute thread to stop early
    cancel_tx: Option<crossbeam_channel::Sender<()>>,
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
            last_umap_mode: None,
            status: AppStatus::Idle,
            error_message: None,
            skipped_blobs: 0,
            max_points: 0,
            umap_mode: UmapMode::Classic,
            umap_lr_log: -3.0,
            hidden_sizes_str: "100,100,100".to_string(),
            umap_neighbors: 12,
            umap_min_dist: 0.13,
            umap_epochs: 200,
            auto_recompute: false,
            pending_recompute: false,
            compute_tx: tx,
            compute_rx: rx,
            progress: None,
            progress_rx: None,
            cancel_tx: None,
        }
    }
}

impl App for VizApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process background results
        self.process_compute_results();

        // Poll progress channel (if any) and request repaint when new progress arrives
        if let Some(rx) = &self.progress_rx {
            let mut got = false;
            while let Ok(p) = rx.try_recv() {
                self.progress = Some(p);
                got = true;
            }
            if got {
                ctx.request_repaint();
            }
        }

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
                    if let Some(p) = &self.progress {
                        let frac = p.epoch as f32 / p.total_epochs as f32;
                        ui.add(
                            egui::ProgressBar::new(frac)
                                .text(format!(
                                    "Epoch {}/{} — loss={:.4}",
                                    p.epoch, p.total_epochs, p.loss
                                ))
                                .animate(true),
                        );
                    }
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
                let resp_max = ui.add(egui::Slider::new(&mut self.max_points, 0..=50000).text("max_points"));
                resp_max.clone().on_hover_text("Limit to first N points (0 = all). Changing this will recompute when Auto is enabled.");
                if resp_max.changed() && self.auto_recompute {
                    if self.status == AppStatus::Idle && !self.points.is_empty() {
                        self.start_umap_2d_computation();
                    } else {
                        self.pending_recompute = true;
                    }
                }

                if ui.button("Load DB").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.db_path = Some(path.clone());
                        self.start_loading(path);
                    }
                }
            });

            ui.separator();

            ui.heading("UMAP 2D");
            // UMAP mode selector
            ui.horizontal(|ui| {
                ui.label("UMAP mode:");
                let parametric_supported = cfg!(feature = "gpu");
                let resp_classic = ui.radio_value(&mut self.umap_mode, UmapMode::Classic, "Classic");
                resp_classic.clone().on_hover_text("Classical non-parametric UMAP (no neural network). Works with the CPU backend.");
                if resp_classic.changed() && self.auto_recompute {
                    if self.status == AppStatus::Idle && !self.points.is_empty() {
                        self.start_umap_2d_computation();
                    } else {
                        self.pending_recompute = true;
                    }
                }

                let resp = ui.add_enabled(parametric_supported, egui::RadioButton::new(self.umap_mode == UmapMode::Parametric, "Parametric"));
                let hover_text = if parametric_supported { "Parametric UMAP trains a neural network (hidden sizes) and supports out-of-sample transform." } else { "Parametric UMAP requires building with the 'gpu' feature (not enabled in this build)" };
                resp.clone().on_hover_text(hover_text.to_string());
                if resp.changed() {
                    if parametric_supported {
                        self.umap_mode = UmapMode::Parametric;
                    }
                    if self.auto_recompute {
                        if self.status == AppStatus::Idle && !self.points.is_empty() {
                            self.start_umap_2d_computation();
                        } else {
                            self.pending_recompute = true;
                        }
                    }
                }
            });

            // Core UMAP params
            let resp_neighbors = ui.add(egui::Slider::new(&mut self.umap_neighbors, 2..=500).text("n_neighbors"));
            resp_neighbors.clone().on_hover_text("Number of neighbors for the k-NN graph. Smaller values capture local structure; larger values capture global structure.");
            if resp_neighbors.changed() && self.auto_recompute {
                if self.status == AppStatus::Idle && !self.points.is_empty() {
                    self.start_umap_2d_computation();
                } else {
                    self.pending_recompute = true;
                }
            }

            let resp_min = ui.add(egui::Slider::new(&mut self.umap_min_dist, 0.0..=1.0).text("min_dist"));
            resp_min.clone().on_hover_text("Minimum distance between points in the low-dimensional embedding. Smaller => tighter clusters.");
            if resp_min.changed() && self.auto_recompute {
                if self.status == AppStatus::Idle && !self.points.is_empty() {
                    self.start_umap_2d_computation();
                } else {
                    self.pending_recompute = true;
                }
            }

            ui.horizontal(|ui| {
                ui.label("epochs");
                let resp_epochs = ui.add(egui::DragValue::new(&mut self.umap_epochs));
                resp_epochs.clone().on_hover_text("Number of training epochs. Higher values may improve convergence but take longer.");
                if resp_epochs.changed() && self.auto_recompute {
                    if self.status == AppStatus::Idle && !self.points.is_empty() {
                        self.start_umap_2d_computation();
                    } else {
                        self.pending_recompute = true;
                    }
                }
            });

            // Log slider for learning rate: log10(lr) in [-5, -1]
            let lr_inner = ui.horizontal(|ui| {
                ui.label("log10(lr)");
                let slider_resp = ui.add(egui::Slider::new(&mut self.umap_lr_log, -5.0..=-1.0).text("log10(lr)"));
                ui.label(format!("lr={:.5}", 10f32.powf(self.umap_lr_log)));
                slider_resp
            });
            lr_inner.response.clone().on_hover_text("Log slider for learning rate: move to change lr = 10^{x}. Typical default lr=1e-3 (log10=-3)");
            if lr_inner.response.changed() && self.auto_recompute {
                if self.status == AppStatus::Idle && !self.points.is_empty() {
                    self.start_umap_2d_computation();
                } else {
                    self.pending_recompute = true;
                }
            }

            // Parametric-specific: hidden sizes
            if self.umap_mode == UmapMode::Parametric {
                let disabled = !cfg!(feature = "gpu");
                let resp = ui.add_enabled(!disabled, egui::TextEdit::singleline(&mut self.hidden_sizes_str));
                resp.clone().on_hover_text("Hidden layer sizes for the parametric UMAP neural network, comma-separated, e.g. 100,100,100");
                if disabled {
                    resp.clone().on_disabled_hover_text("Parametric UMAP not available in this build");
                }
                if resp.changed() && self.auto_recompute {
                    if self.status == AppStatus::Idle && !self.points.is_empty() {
                        self.start_umap_2d_computation();
                    } else {
                        self.pending_recompute = true;
                    }
                }
            }

            ui.add(egui::Slider::new(&mut self.point_radius, 0.01..=2.0).text("point radius")).on_hover_text(
                "Radius of plotted points in pixels. Reduce to see dense structure.",
            );

            let umap_enabled = !self.points.is_empty() && self.status == AppStatus::Idle && (self.umap_mode == UmapMode::Classic || cfg!(feature = "gpu"));
            let auto_resp = ui.checkbox(&mut self.auto_recompute, "Auto recompute");
            auto_resp.clone().on_hover_text("Automatically recompute UMAP whenever a parameter changes (will wait for current run to finish)");
            if auto_resp.changed() && self.auto_recompute {
                // If turned on and idle, kick off an immediate compute
                if self.status == AppStatus::Idle && !self.points.is_empty() {
                    self.start_umap_2d_computation();
                }
            }

            let compute_btn = ui.add_enabled(umap_enabled, egui::Button::new("Compute 2D UMAP"));
            if !umap_enabled {
                compute_btn.clone().on_disabled_hover_text("Cannot compute: no data loaded or parametric mode requires GPU build");
            } else {
                compute_btn.clone().on_hover_text("Start UMAP with the selected parameters");
            }

            if compute_btn.clicked() {
                self.start_umap_2d_computation();
            }

            // Show cancel button when computing
            if self.status == AppStatus::ComputingUmap {
                if let Some(tx) = &self.cancel_tx {
                    if ui.button("Cancel").clicked() {
                        let _ = tx.send(());
                        // Clear progress UI immediately
                        self.progress = None;
                        self.progress_rx = None;
                        self.cancel_tx = None;
                        self.status = AppStatus::Idle;
                    }
                }
            }

            if let Some(ref p) = self.last_umap_params {
                ui.label(format!(
                    "Last UMAP: neighbors={}, min_dist={}, epochs={}, lr={}",
                    p.n_neighbors, p.min_dist, p.n_epochs, p.learning_rate
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

        // Build UMAP params with hidden sizes and learning rate
        // Parse hidden sizes (comma separated)
        let hidden_sizes = if self.umap_mode == UmapMode::Parametric {
            let parsed: Result<Vec<usize>, String> = self
                .hidden_sizes_str
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| {
                    s.parse::<usize>()
                        .map_err(|e| format!("Invalid hidden size '{}': {}", s, e))
                })
                .collect();
            match parsed {
                Ok(v) => v,
                Err(err_msg) => {
                    self.error_message = Some(err_msg);
                    return;
                }
            }
        } else {
            Vec::new()
        };

        let params = UmapParams {
            n_components: 2,
            n_neighbors: self.umap_neighbors,
            min_dist: self.umap_min_dist,
            n_epochs: self.umap_epochs,
            learning_rate: 10f64.powf(self.umap_lr_log as f64),
            hidden_sizes: hidden_sizes.clone(),
        };
        let tx = self.compute_tx.clone();

        // Clone params for compute and for reporting
        let params_for_compute = params.clone();
        let params_for_result = params.clone();

        // Remember the selected mode for reporting
        self.last_umap_mode = Some(self.umap_mode);

        // Create progress & cancel channels so GUI can receive epoch updates and cancel
        let (progress_tx, progress_rx) = crossbeam_channel::unbounded::<ProgressUpdate>();
        let (cancel_tx, cancel_rx) = crossbeam_channel::unbounded::<()>();
        self.progress_rx = Some(progress_rx);
        self.cancel_tx = Some(cancel_tx.clone());
        self.progress = None;

        thread::spawn(move || {
            match compute_umap(
                &embeddings,
                params_for_compute,
                Some(progress_tx),
                Some(cancel_rx),
            ) {
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
                    // Clear progress channels
                    self.progress = None;
                    self.progress_rx = None;
                    self.cancel_tx = None;
                    // If a parameter changed while running, trigger another run
                    if self.pending_recompute {
                        self.pending_recompute = false;
                        if !self.points.is_empty() {
                            self.start_umap_2d_computation();
                        }
                    }
                }
                ComputeResult::Error(msg) => {
                    self.error_message = Some(msg);
                    self.status = AppStatus::Idle;
                    // Clear progress channels
                    self.progress = None;
                    self.progress_rx = None;
                    self.cancel_tx = None;
                    if self.pending_recompute {
                        self.pending_recompute = false;
                        if !self.points.is_empty() {
                            self.start_umap_2d_computation();
                        }
                    }
                }
            }
        }
    }
}
