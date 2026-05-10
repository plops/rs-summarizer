use eframe::{egui, App};
use egui_plot::{Plot, PlotPoints, Points};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::SystemTime;

use crossbeam_channel;

use crate::data_loader::{load_compact_db, load_compact_db_subset, EmbeddingPoint};
use crate::dbscan_engine::{compute_dbscan, DbscanParams};
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
    ComputingClusters,
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
    /// 4D UMAP finished (optionally provides a 2D projection of the first two components)
    Umap4dDone {
        embeddings_4d: Vec<[f32; 4]>,
        embeddings_2d_from_4d: Option<Vec<[f32; 2]>>,
        params: UmapParams,
    },
    /// Clustering finished (labels aligned with current embeddings)
    ClusterDone {
        labels: Vec<i32>,
        /// Optional 2D embeddings derived from the 4D run (used when no 2D UMAP was computed)
        embeddings_2d_from_4d: Option<Vec<[f32; 2]>>,
        /// DBSCAN parameters used for the clustering
        params: DbscanParams,
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
    /// Optional cluster labels (aligned with embeddings_2d) - -1 == noise
    cluster_labels: Option<Vec<i32>>,
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

    // UMAP params (2D)
    umap_mode: UmapMode,
    /// log10(learning_rate) slider value (e.g. -3.0 -> 1e-3)
    umap_lr_log: f32,
    hidden_sizes_str: String,
    umap_neighbors: usize,
    umap_min_dist: f32,
    umap_epochs: usize,

    // UMAP 4D params
    umap4_neighbors: usize,
    umap4_min_dist: f32,
    umap4_epochs: usize,
    umap4_hidden_sizes_str: String,
    /// Optional stored 4D embedding (computed via Compute 4D UMAP)
    embeddings_4d: Option<Vec<[f32; 4]>>,
    /// When true, computing 4D UMAP will replace the 2D plot with the first-two components of the 4D embedding
    replace_2d_with_4d: bool,

    // Snapshots & timestamps for presence indicators
    umap2_params_snapshot: Option<UmapParams>,
    umap2_computed_at: Option<SystemTime>,
    umap4_params_snapshot: Option<UmapParams>,
    umap4_computed_at: Option<SystemTime>,
    dbscan_params_snapshot: Option<DbscanParams>,
    dbscan_computed_at: Option<SystemTime>,

    // DBSCAN params
    dbscan_eps: f64,
    dbscan_min_samples: usize,
    auto_dbscan: bool,

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
            cluster_labels: None,
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
            // UMAP 4D defaults
            umap4_neighbors: 5,
            umap4_min_dist: 0.1,
            umap4_epochs: 200,
            umap4_hidden_sizes_str: "100,100,100".to_string(),
            embeddings_4d: None,
            replace_2d_with_4d: false,

            // snapshots & timestamps
            umap2_params_snapshot: None,
            umap2_computed_at: None,
            umap4_params_snapshot: None,
            umap4_computed_at: None,
            dbscan_params_snapshot: None,
            dbscan_computed_at: None,

            // DBSCAN defaults
            dbscan_eps: 0.3,
            dbscan_min_samples: 5,
            auto_dbscan: false,
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
                AppStatus::ComputingClusters => {
                    ui.label("Status: Computing clusters (4D UMAP -> DBSCAN)...");
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
                    if self.status == AppStatus::ComputingUmap || self.status == AppStatus::ComputingClusters {
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

                    // Presence indicators for UMAP 2D, UMAP 4D, and DBSCAN
                    ui.separator();
                    ui.label(egui::RichText::new("Run status").size(12.0));

                    ui.horizontal(|ui| {
                        // 2D UMAP indicator
                        let has_2d = self.embeddings_2d.is_some();
                        let dot = if has_2d { "●" } else { "○" };
                        let color = if has_2d { egui::Color32::from_rgb(80, 200, 120) } else { egui::Color32::LIGHT_GRAY };
                        ui.label(egui::RichText::new(dot).color(color).size(12.0));
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("UMAP 2D").size(11.0));
                            if let Some(params) = &self.umap2_params_snapshot {
                                let txt = format!("n={} md={} ep={}", params.n_neighbors, params.min_dist, params.n_epochs);
                                let tooltip = if let Some(t) = self.umap2_computed_at {
                                    let epoch = match t.duration_since(SystemTime::UNIX_EPOCH) { Ok(d) => d.as_secs(), Err(_) => 0 };
                                    let ago = match SystemTime::now().duration_since(t) {
                                        Ok(d) if d.as_secs() < 60 => format!("{}s", d.as_secs()),
                                        Ok(d) if d.as_secs() < 3600 => format!("{}m", d.as_secs()/60),
                                        Ok(d) => format!("{}h", d.as_secs()/3600),
                                        Err(_) => "n/a".to_string(),
                                    };
                                    format!("Computed at: {} ({} ago)", epoch, ago)
                                } else {
                                    "Not computed".to_string()
                                };
                                ui.label(egui::RichText::new(txt).size(9.0)).on_hover_text(tooltip);
                            } else {
                                ui.label(egui::RichText::new("not computed").size(9.0));
                            }
                        });

                        ui.add_space(10.0);

                        // 4D UMAP indicator
                        let has_4d = self.embeddings_4d.is_some();
                        let dot4 = if has_4d { "●" } else { "○" };
                        let color4 = if has_4d { egui::Color32::from_rgb(100, 160, 255) } else { egui::Color32::LIGHT_GRAY };
                        ui.label(egui::RichText::new(dot4).color(color4).size(12.0));
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("UMAP 4D").size(11.0));
                            if let Some(params) = &self.umap4_params_snapshot {
                                let txt = format!("n={} md={} ep={}", params.n_neighbors, params.min_dist, params.n_epochs);
                                let tooltip = if let Some(t) = self.umap4_computed_at {
                                    let epoch = match t.duration_since(SystemTime::UNIX_EPOCH) { Ok(d) => d.as_secs(), Err(_) => 0 };
                                    let ago = match SystemTime::now().duration_since(t) {
                                        Ok(d) if d.as_secs() < 60 => format!("{}s", d.as_secs()),
                                        Ok(d) if d.as_secs() < 3600 => format!("{}m", d.as_secs()/60),
                                        Ok(d) => format!("{}h", d.as_secs()/3600),
                                        Err(_) => "n/a".to_string(),
                                    };
                                    format!("Computed at: {} ({} ago)", epoch, ago)
                                } else {
                                    "Not computed".to_string()
                                };
                                ui.label(egui::RichText::new(txt).size(9.0)).on_hover_text(tooltip);
                            } else {
                                ui.label(egui::RichText::new("not computed").size(9.0));
                            }
                        });

                        ui.add_space(10.0);

                        // DBSCAN indicator
                        let has_db = self.cluster_labels.is_some();
                        let dotc = if has_db { "●" } else { "○" };
                        let colorc = if has_db { egui::Color32::from_rgb(255, 180, 80) } else { egui::Color32::LIGHT_GRAY };
                        ui.label(egui::RichText::new(dotc).color(colorc).size(12.0));
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("DBSCAN").size(11.0));
                            if let Some(params) = &self.dbscan_params_snapshot {
                                let txt = format!("eps={} min_samples={}", params.eps, params.min_samples);
                                let tooltip = if let Some(t) = self.dbscan_computed_at {
                                    let epoch = match t.duration_since(SystemTime::UNIX_EPOCH) { Ok(d) => d.as_secs(), Err(_) => 0 };
                                    let ago = match SystemTime::now().duration_since(t) {
                                        Ok(d) if d.as_secs() < 60 => format!("{}s", d.as_secs()),
                                        Ok(d) if d.as_secs() < 3600 => format!("{}m", d.as_secs()/60),
                                        Ok(d) => format!("{}h", d.as_secs()/3600),
                                        Err(_) => "n/a".to_string(),
                                    };
                                    format!("Computed at: {} ({} ago)", epoch, ago)
                                } else {
                                    "Not computed".to_string()
                                };
                                ui.label(egui::RichText::new(txt).size(9.0)).on_hover_text(tooltip);
                            } else {
                                ui.label(egui::RichText::new("not computed").size(9.0));
                            }
                        });
                    });

                    // UMAP 4D controls
                    ui.separator();
                    ui.heading("UMAP 4D");
                    ui.horizontal(|ui| {
                        ui.label("n_neighbors");
                        ui.add(egui::Slider::new(&mut self.umap4_neighbors, 2..=500).text("n_neighbors"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("min_dist");
                        ui.add(egui::Slider::new(&mut self.umap4_min_dist, 0.0..=1.0).text("min_dist"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("epochs");
                        ui.add(egui::DragValue::new(&mut self.umap4_epochs));
                    });

                    if self.umap_mode == UmapMode::Parametric {
                        let disabled = !cfg!(feature = "gpu");
                        let resp = ui.add_enabled(!disabled, egui::TextEdit::singleline(&mut self.umap4_hidden_sizes_str));
                        resp.clone().on_hover_text("Hidden sizes for parametric 4D UMAP, comma-separated (e.g. 100,100)");
                        if disabled {
                            resp.clone().on_disabled_hover_text("Parametric UMAP not available in this build");
                        }
                    }

                    ui.checkbox(&mut self.replace_2d_with_4d, "Replace 2D plot with 4D projection");

                    let umap4_enabled = !self.points.is_empty() && self.status == AppStatus::Idle && (self.umap_mode == UmapMode::Classic || cfg!(feature = "gpu"));
                    let compute_umap4_btn = ui.add_enabled(umap4_enabled, egui::Button::new("Compute 4D UMAP"));
                    if !umap4_enabled {
                        compute_umap4_btn.clone().on_disabled_hover_text("Cannot compute: no data loaded or parametric mode requires GPU build");
                    }
                    if compute_umap4_btn.clicked() {
                        self.start_umap_4d_computation();
                    }

                    // DBSCAN controls
                    ui.separator();
                    ui.heading("DBSCAN Clustering (from 4D UMAP)");
                    ui.horizontal(|ui| {
                                            ui.label("eps");
                                            let resp = ui.add(egui::Slider::new(&mut self.dbscan_eps, 0.001..=5.0).text("eps"));
                                            resp.on_hover_text("DBSCAN epsilon (distance threshold).\nSmaller eps => more noise; larger eps => larger clusters.\nNote: release runs set RAYON_NUM_THREADS to the machine CPU count to accelerate CPU-bound phases (kNN / nn-descent), which can speed DBSCAN preprocessing.");
                                        });
                    ui.horizontal(|ui| {
                        ui.label("min_samples");
                        ui.add(egui::DragValue::new(&mut self.dbscan_min_samples));
                    });

                    let dbscan_enabled = !self.points.is_empty() && self.status == AppStatus::Idle && cfg!(feature = "gpu");
                    if !dbscan_enabled {
                        ui.label("DBSCAN requires data loaded and GPU parametric UMAP enabled");
                    }

                    let compute_clusters_btn = ui.add_enabled(dbscan_enabled, egui::Button::new("Compute Clusters (4D UMAP -> DBSCAN)"));
                    if compute_clusters_btn.clicked() {
                        self.start_dbscan_clustering();
                    }

                    ui.checkbox(&mut self.auto_dbscan, "Auto run DBSCAN after UMAP");

                    if let Some(labels) = &self.cluster_labels {
                        let mut counts = std::collections::HashMap::new();
                        for &l in labels.iter() {
                            *counts.entry(l).or_insert(0usize) += 1;
                        }
                        ui.label(format!("Clusters: {:?}", counts));
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
                    // If cluster labels exist and match point count, draw per-cluster series
                    if let Some(labels) = &self.cluster_labels {
                        if labels.len() == points.len() {
                            let mut groups: std::collections::HashMap<i32, Vec<[f64; 2]>> =
                                std::collections::HashMap::new();
                            for (i, pt) in points.iter().enumerate() {
                                let id = labels[i];
                                groups.entry(id).or_default().push(*pt);
                            }

                            // Sort keys so noise (-1) is drawn last
                            let mut keys: Vec<i32> = groups.keys().cloned().collect();
                            keys.sort_by_key(|&k| if k == -1 { i32::MAX } else { k });

                            for key in keys {
                                if let Some(pts) = groups.remove(&key) {
                                    let plot_points: PlotPoints = pts.into_iter().collect();
                                    let color = self.cluster_color(key);
                                    let label = if key == -1 {
                                        "noise".to_string()
                                    } else {
                                        format!("cluster_{}", key)
                                    };
                                    plot_ui.points(
                                        Points::new(label, plot_points)
                                            .color(color)
                                            .radius(self.point_radius),
                                    );
                                }
                            }

                            return;
                        }
                    }

                    // default single-color plot
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

    fn start_umap_4d_computation(&mut self) {
        if self.points.is_empty() {
            return;
        }

        self.status = AppStatus::ComputingUmap;
        self.error_message = None;
        // Clear previous 4D embedding and clusters
        self.embeddings_4d = None;
        self.cluster_labels = None;

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

        // Parse hidden sizes (comma separated) for parametric runs (4D)
        let hidden_sizes = if self.umap_mode == UmapMode::Parametric {
            let parsed: Result<Vec<usize>, String> = self
                .umap4_hidden_sizes_str
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
            n_components: 4,
            n_neighbors: self.umap4_neighbors,
            min_dist: self.umap4_min_dist,
            n_epochs: self.umap4_epochs,
            learning_rate: 10f64.powf(self.umap_lr_log as f64),
            hidden_sizes: hidden_sizes.clone(),
        };

        let tx = self.compute_tx.clone();

        // Create progress & cancel channels so GUI can receive epoch updates and cancel
        let (progress_tx, progress_rx) = crossbeam_channel::unbounded::<ProgressUpdate>();
        let (cancel_tx, cancel_rx) = crossbeam_channel::unbounded::<()>();
        self.progress_rx = Some(progress_rx);
        self.cancel_tx = Some(cancel_tx.clone());
        self.progress = None;

        let params_for_result = params.clone();

        thread::spawn(move || {
            match compute_umap(&embeddings, params, Some(progress_tx), Some(cancel_rx)) {
                Ok(emb4d) => {
                    // Convert to fixed 4D arrays and also provide a 2D projection (first two dims)
                    let mut arr4d: Vec<[f32; 4]> = Vec::with_capacity(emb4d.len());
                    let mut arr2d: Vec<[f32; 2]> = Vec::with_capacity(emb4d.len());
                    for v in emb4d.iter() {
                        let mut a4 = [0.0f32; 4];
                        if v.len() >= 4 {
                            for i in 0..4 {
                                a4[i] = v[i];
                            }
                        } else {
                            for (i, &val) in v.iter().enumerate().take(4) {
                                a4[i] = val;
                            }
                        }
                        arr4d.push(a4);
                        let a2 = [a4[0], a4[1]];
                        arr2d.push(a2);
                    }

                    let _ = tx.send(ComputeResult::Umap4dDone {
                        embeddings_4d: arr4d,
                        embeddings_2d_from_4d: Some(arr2d),
                        params: params_for_result,
                    });
                }
                Err(e) => {
                    let _ = tx.send(ComputeResult::Error(format!("UMAP (4D) failed: {}", e)));
                }
            }
        });
    }

    /// Start the clustering flow: compute 4D UMAP (parametric when available), then run DBSCAN.
    fn start_dbscan_clustering(&mut self) {
        if self.points.is_empty() {
            return;
        }

        self.status = AppStatus::ComputingClusters;
        self.error_message = None;
        self.cluster_labels = None;

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

        // Parse hidden sizes (comma separated) for parametric runs (4D)
        let hidden_sizes = if self.umap_mode == UmapMode::Parametric {
            let parsed: Result<Vec<usize>, String> = self
                .umap4_hidden_sizes_str
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
            n_components: 4,
            n_neighbors: self.umap4_neighbors,
            min_dist: self.umap4_min_dist,
            n_epochs: self.umap4_epochs,
            learning_rate: 10f64.powf(self.umap_lr_log as f64),
            hidden_sizes: hidden_sizes.clone(),
        };

        // Determine subset size to match the currently displayed 2D embedding when possible
        let subset_count: usize = if let Some(ref emb2d) = self.embeddings_2d {
            emb2d.len()
        } else if self.max_points > 0 && self.max_points < self.points.len() {
            self.max_points
        } else {
            self.points.len()
        };

        // Build embeddings for this subset
        let embeddings: Vec<Vec<f32>> = self
            .points
            .iter()
            .take(subset_count)
            .map(|p| p.embedding.clone())
            .collect();

        let tx = self.compute_tx.clone();
        // only consider have_2d true when existing embeddings_2d length matches subset_count
        let have_2d = self
            .embeddings_2d
            .as_ref()
            .map(|v| v.len() == subset_count)
            .unwrap_or(false);
        let eps = self.dbscan_eps;
        let min_samples = self.dbscan_min_samples;

        // Preserve a cached 4D embedding only if it matches the same subset size
        let cached_4d = self
            .embeddings_4d
            .clone()
            .filter(|c| c.len() == subset_count);

        // Create progress & cancel channels so GUI can receive epoch updates and cancel
        let (progress_tx, progress_rx) = crossbeam_channel::unbounded::<ProgressUpdate>();
        let (cancel_tx, cancel_rx) = crossbeam_channel::unbounded::<()>();
        self.progress_rx = Some(progress_rx);
        self.cancel_tx = Some(cancel_tx.clone());
        self.progress = None;

        thread::spawn(move || {
            // If we have a cached 4D embedding for the same subset, use it directly
            if let Some(cached) = cached_4d {
                // Convert to fixed 4D arrays for DBSCAN
                let arr4d: Vec<[f32; 4]> = cached.into_iter().collect();
                let db_params = DbscanParams { eps, min_samples };
                match compute_dbscan(arr4d.as_slice(), db_params) {
                    Ok(labels) => {
                        // Provide a 2D projection matching the cached 4D (useful if we need to update view)
                        let embeddings_2d_from_4d =
                            Some(arr4d.iter().map(|v| [v[0], v[1]]).collect());
                        let _ = tx.send(ComputeResult::ClusterDone {
                            labels,
                            embeddings_2d_from_4d,
                            params: DbscanParams { eps, min_samples },
                        });
                        return;
                    }
                    Err(e) => {
                        let _ = tx.send(ComputeResult::Error(format!("DBSCAN failed: {}", e)));
                        return;
                    }
                }
            }

            // Otherwise compute 4D UMAP then cluster
            match compute_umap(&embeddings, params, Some(progress_tx), Some(cancel_rx)) {
                Ok(emb4d) => {
                    // Convert to fixed 4D arrays for DBSCAN
                    let mut arr4d: Vec<[f32; 4]> = Vec::with_capacity(emb4d.len());
                    for v in &emb4d {
                        let mut a = [0.0f32; 4];
                        for i in 0..4 {
                            if i < v.len() {
                                a[i] = v[i];
                            }
                        }
                        arr4d.push(a);
                    }

                    let db_params = DbscanParams { eps, min_samples };

                    match compute_dbscan(arr4d.as_slice(), db_params) {
                        Ok(labels) => {
                            let embeddings_2d_from_4d = if !have_2d {
                                Some(
                                    emb4d
                                        .into_iter()
                                        .map(|v| {
                                            let mut a = [0.0f32; 2];
                                            if v.len() >= 2 {
                                                a[0] = v[0];
                                                a[1] = v[1];
                                            }
                                            a
                                        })
                                        .collect(),
                                )
                            } else {
                                None
                            };

                            let _ = tx.send(ComputeResult::ClusterDone {
                                labels,
                                embeddings_2d_from_4d,
                                params: DbscanParams { eps, min_samples },
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(ComputeResult::Error(format!("DBSCAN failed: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(ComputeResult::Error(format!(
                        "UMAP (4D for clustering) failed: {}",
                        e
                    )));
                }
            }
        });
    }

    /// Color palette helper for clusters
    fn cluster_color(&self, id: i32) -> egui::Color32 {
        if id == -1 {
            return egui::Color32::from_gray(150);
        }
        let palette = [
            (166u8, 206u8, 227u8),
            (31u8, 120u8, 180u8),
            (178u8, 223u8, 138u8),
            (51u8, 160u8, 44u8),
            (251u8, 154u8, 153u8),
            (227u8, 26u8, 28u8),
            (253u8, 191u8, 111u8),
            (255u8, 127u8, 0u8),
            (202u8, 178u8, 214u8),
            (106u8, 61u8, 154u8),
            (255u8, 255u8, 153u8),
            (177u8, 89u8, 40u8),
        ];
        let idx = (id as usize) % palette.len();
        let (r, g, b) = palette[idx];
        egui::Color32::from_rgb(r, g, b)
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
                    self.last_umap_params = Some(params.clone());
                    // store a dedicated 2D snapshot and timestamp for the presence indicator
                    self.umap2_params_snapshot = Some(params.clone());
                    self.umap2_computed_at = Some(SystemTime::now());

                    self.status = AppStatus::Idle;
                    // Clear progress channels
                    self.progress = None;
                    self.progress_rx = None;
                    self.cancel_tx = None;
                    // If auto-run DBSCAN is enabled, trigger clustering
                    if self.auto_dbscan {
                        self.start_dbscan_clustering();
                    }
                    // If a parameter changed while running, trigger another run
                    if self.pending_recompute {
                        self.pending_recompute = false;
                        if !self.points.is_empty() {
                            self.start_umap_2d_computation();
                        }
                    }
                }
                ComputeResult::Umap4dDone {
                    embeddings_4d,
                    embeddings_2d_from_4d,
                    params,
                } => {
                    // Store 4D embedding
                    self.embeddings_4d = Some(embeddings_4d);
                    // Optionally replace 2D plot with the first-two components of the 4D result
                    if self.replace_2d_with_4d {
                        if let Some(e2d) = embeddings_2d_from_4d {
                            self.embeddings_2d = Some(e2d);
                        }
                    }
                    self.last_umap_params = Some(params.clone());
                    // store a dedicated 4D snapshot and timestamp for the presence indicator
                    self.umap4_params_snapshot = Some(params.clone());
                    self.umap4_computed_at = Some(SystemTime::now());
                    // Clear existing cluster labels (4D changed)
                    self.cluster_labels = None;
                    self.status = AppStatus::Idle;
                    // Clear progress channels
                    self.progress = None;
                    self.progress_rx = None;
                    self.cancel_tx = None;
                    // Auto-run DBSCAN if requested
                    if self.auto_dbscan {
                        self.start_dbscan_clustering();
                    }
                }
                ComputeResult::ClusterDone {
                    labels,
                    embeddings_2d_from_4d,
                    params,
                } => {
                    // Record DBSCAN params snapshot and completion time
                    self.dbscan_params_snapshot = Some(params);
                    self.dbscan_computed_at = Some(SystemTime::now());

                    // Align 2D projection with labels when necessary
                    let labels_len = labels.len();

                    let need_replace_2d = match &self.embeddings_2d {
                        Some(e2d) => e2d.len() != labels_len,
                        None => true,
                    };

                    if need_replace_2d {
                        // Prefer provided 2D projection from the clustering run
                        if let Some(e2d) = embeddings_2d_from_4d {
                            self.embeddings_2d = Some(e2d);
                        } else if let Some(ref emb4d) = self.embeddings_4d {
                            if emb4d.len() == labels_len {
                                let projected: Vec<[f32; 2]> =
                                    emb4d.iter().map(|v| [v[0], v[1]]).collect();
                                self.embeddings_2d = Some(projected);
                            }
                        }
                    }

                    // Set cluster labels (now that embeddings_2d likely matches)
                    self.cluster_labels = Some(labels);

                    self.status = AppStatus::Idle;
                    // Clear progress channels
                    self.progress = None;
                    self.progress_rx = None;
                    self.cancel_tx = None;
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
