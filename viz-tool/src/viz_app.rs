use eframe::{egui, NativeOptions};
use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;

use crate::data_loader::{DataLoader, EmbeddingPoint};
use crate::errors::VizError;

#[derive(Clone)]
pub enum AppState {
    Loading,
    Ready,
    Processing,
    Error(String),
}

#[derive(Clone)]
pub struct AppStateData {
    state: AppState,
    data: Vec<EmbeddingPoint>,
    embeddings: Vec<Vec<f32>>,
    clusters: Vec<i32>,
    cluster_labels: HashMap<i32, String>,
    umap_2d: Option<Vec<[f32; 2]>>,
}

#[derive(Clone)]
pub struct VizApp {
    state: Arc<Mutex<AppStateData>>,
}

impl eframe::App for VizApp {
    fn update(&mut self, ctx: &egui::Context) -> eframe::App::Update {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Embedding Visualization Tool");
            
            if let Ok(state_data) = self.state.lock() {
                match &state_data.state {
                    AppState::Loading => {
                        ui.label("Loading data...");
                    }
                    AppState::Ready => {
                        ui.label(format!("Loaded {} data points", state_data.data.len()));
                    }
                    AppState::Processing => {
                        ui.label("Processing...");
                    }
                    AppState::Error(ref err) => {
                        ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
                    }
                }
            }
        });
        
        eframe::App::Update::None
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Save application state
        if let Ok(state_data) = self.state.lock() {
            storage.set_string("app_state", serde_json::to_string(&*state_data).unwrap());
        }
    }
}

impl Default for VizApp {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(AppStateData {
                state: AppState::Loading,
                data: Vec::new(),
                embeddings: Vec::new(),
                clusters: Vec::new(),
                cluster_labels: HashMap::new(),
                umap_2d: None,
            })),
        }
    }
}
