use crate::errors::VizError;
use fast_umap::prelude::*;
use burn::prelude::*;
use cubecl::wgpu::WgpuRuntime;
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NnMapperResult {
    pub model_path: String,
    pub metadata: ModelMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub n_components: usize,
    pub n_neighbors: usize,
    pub min_dist: f64,
    pub n_epochs: usize,
    pub training_points: usize,
}

pub struct NnMapper {
    runtime: WgpuRuntime,
}

impl NnMapper {
    pub fn new() -> Result<Self, VizError> {
        let runtime = WgpuRuntime::init()
            .map_err(|e| VizError::Umap(format!("Failed to initialize WGPU runtime: {}", e)))?;
        
        Ok(Self { runtime })
    }

    pub async fn save_model(&self, model: FittedUmap<Backend<Burn<WgpuRuntime, f32, i32>>, path: &Path) -> Result<NnMapperResult, VizError> {
        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| VizError::Io(format!("Failed to create directory: {}", e)))?;
        }

        // Save the fitted model using burn's recorder
        let recorder = burn::record::CompactRecorder::new();
        
        let model_data = recorder
            .record(Model::new(&ModelRecord::new()), &model)
            .map_err(|e| VizError::Umap(format!("Failed to record model: {}", e)))?;

        // Save to file
        recorder
            .save(model_data, path)
            .map_err(|e| VizError::Io(format!("Failed to save model: {}", e)))?;

        // Extract metadata
        let metadata = ModelMetadata {
            n_components: model.config().n_components,
            n_neighbors: model.config().n_neighbors,
            min_dist: model.config().min_dist,
            n_epochs: model.config().n_epochs,
            training_points: model.embedding().shape().dims[0],
        };

        Ok(NnMapperResult {
            model_path: path.to_string_lossy().to_string(),
            metadata,
        })
    }

    pub async fn load_model(&self, path: &Path) -> Result<FittedUmap<Backend<Burn<WgpuRuntime, f32, i32>>, VizError> {
        // Load the saved model
        let model_data = burn::record::CompactRecorder::new()
            .load(path)
            .map_err(|e| VizError::Io(format!("Failed to load model: {}", e)))?;

        let model = model_data.model.get::<Model<Burn<WgpuRuntime, f32, i32>>()
            .ok_or_else(|| VizError::Umap("Invalid model format".to_string()))?;

        // Create UMAP instance from loaded model
        let umap = Umap::new(model.config());

        Ok(FittedUmap::new(model, umap))
    }

    pub async fn transform_new_points(&self, model: &FittedUmap<Backend<Burn<WgpuRuntime, f32, i32>>, new_embeddings: &[Vec<f32>]) -> Result<Vec<[f32; 2]>, VizError> {
        // Convert new embeddings to burn tensor
        let n_samples = new_embeddings.len();
        let n_features = new_embeddings[0].len();
        
        let mut flat_data = Vec::with_capacity(n_samples * n_features);
        for embedding in new_embeddings {
            flat_data.extend_from_slice(embedding);
        }

        let data = Tensor::<Backend<Burn<WgpuRuntime, f32, i32>, 2>::from_data(
            flat_data,
            Shape::new([n_samples, n_features]),
            &Default::default(),
        );

        // Transform using the loaded model
        let result = model.transform(data)
            .map_err(|e| VizError::Umap(format!("Transform failed: {}", e)))?;

        // Convert result back to Vec<[f32; 2]>
        let result_data = result.into_data();
        let mut embeddings_2d = Vec::with_capacity(n_samples);
        
        for i in 0..n_samples {
            let mut embedding_2d = [0.0f32; 2];
            for j in 0..2 {
                embedding_2d[j] = result_data[i * 2 + j];
            }
            embeddings_2d.push(embedding_2d);
        }

        Ok(embeddings_2d)
    }
}
