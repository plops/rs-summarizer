use crate::errors::NnMapperError;
use fast_umap::prelude::*;
use cubecl::wgpu::WgpuRuntime;
use serde::{Deserialize, Serialize};
use std::path::Path;
use burn_autodiff::Autodiff;
use burn_cubecl::CubeBackend;

type MyBackend = CubeBackend<WgpuRuntime, f32, i32, u32>;
type MyAutodiffBackend = Autodiff<MyBackend>;

/// Sidecar configuration for NN Mapper model persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NnMapperConfig {
    pub umap_config: UmapConfig,
    pub embedding_dim: usize,
}

pub struct NnMapper {
    fitted: FittedUmap<MyAutodiffBackend>,
    embedding_dim: usize,
}

impl NnMapper {
    /// Lädt Modell + Sidecar-Config aus dem Dateisystem.
    /// Benötigt die Modelldatei (.bin) und die Sidecar-Config-Datei (_nn_mapper_config.json).
    pub fn load(model_path: &Path) -> Result<Self, NnMapperError> {
        // Sidecar-Config-Datei laden
        let config_path = model_path.with_extension("_nn_mapper_config.json");
        let config_json = std::fs::read_to_string(&config_path)
            .map_err(|e| NnMapperError::ConfigLoadError(format!("Konnte Config-Datei nicht lesen: {}", e)))?;
        
        let config: NnMapperConfig = serde_json::from_str(&config_json)
            .map_err(|e| NnMapperError::ConfigLoadError(format!("Konnte Config nicht parsen: {}", e)))?;

        // Use default device for the backend
        let device = Default::default();

        // Modell laden
        let fitted = FittedUmap::<MyAutodiffBackend>::load(
            model_path,
            config.umap_config,
            config.embedding_dim,
            device,
        ).map_err(|e| NnMapperError::ModelLoadError(format!("Konnte Modell nicht laden: {}", e)))?;

        Ok(Self {
            fitted,
            embedding_dim: config.embedding_dim,
        })
    }

    /// Projiziert ein einzelnes Embedding auf 2D.
    /// Gibt NnMapperError::DimensionMismatch zurück wenn embedding.len() != embedding_dim.
    pub fn project(&self, embedding: &[f32]) -> Result<(f32, f32), NnMapperError> {
        if embedding.len() != self.embedding_dim {
            return Err(NnMapperError::DimensionMismatch {
                expected: self.embedding_dim,
                actual: embedding.len(),
            });
        }

        // Convert single embedding to Vec<Vec<f64>> for transform
        let embedding_f64: Vec<Vec<f64>> = vec![
            embedding.iter().map(|&x| x as f64).collect()
        ];

        // Transform using the fitted model
        let result = self.fitted.transform(embedding_f64);

        // Extract the first (and only) 2D coordinate
        if result.is_empty() || result[0].len() != 2 {
            return Err(NnMapperError::ProjectionError("Unerwartetes Ergebnis von UMAP Transform".to_string()));
        }

        let x = result[0][0] as f32;
        let y = result[0][1] as f32;
        
        Ok((x, y))
    }

    pub fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }
}
