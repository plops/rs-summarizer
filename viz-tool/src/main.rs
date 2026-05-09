mod errors;
mod embedding;
mod viz_app;

use eframe::{egui, NativeOptions};

use crate::viz_app::VizApp;

fn main() -> Result<(), eframe::Error> {
    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Embedding Visualization Tool",
        native_options,
        Box::new(|cc| Box::pin(VizApp::default())),
    )
}
