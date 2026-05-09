use eframe::{egui, NativeOptions};
use std::path::PathBuf;

use viz_tool::viz_app::VizApp;

fn main() -> Result<(), eframe::Error> {
    // Parse CLI arguments
    let args: Vec<String> = std::env::args().collect();
    let mut db_path: Option<PathBuf> = None;
    let mut embedding_dim: usize = 768; // Default
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--embedding-dim" => {
                if i + 1 < args.len() {
                    if let Ok(dim) = args[i + 1].parse() {
                        embedding_dim = dim;
                    }
                    i += 1;
                }
            }
            arg => {
                // Treat as database path if it doesn't start with --
                if !arg.starts_with("--") && db_path.is_none() {
                    db_path = Some(PathBuf::from(arg));
                }
            }
        }
        i += 1;
    }
    
    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Embedding Visualization Tool",
        native_options,
        Box::new(|cc| {
            let app = VizApp::new(db_path, embedding_dim);
            Ok(Box::new(app))
        }),
    )
}
