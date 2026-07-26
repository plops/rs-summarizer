use crate::errors::NnMapperError;
use burn_autodiff::Autodiff;
use burn_cubecl::CubeBackend;
use cubecl::wgpu::WgpuRuntime;
use fast_umap::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path;

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

// SAFETY: `NnMapper` holds a trained `FittedUmap` model and configuration metadata.
// `FittedUmap` is constructed once during loading (`NnMapper::load`) and remains immutable
// for the entire lifecycle of `NnMapper`. Transferring `NnMapper` across thread boundaries
// does not trigger any concurrent state mutation or un-synchronized pointer access.
unsafe impl Send for NnMapper {}

// SAFETY: `NnMapper` exposes only immutable read-only operations (`project` and `embedding_dim`).
// During runtime projection (`NnMapper::project`), the underlying `FittedUmap` executes a forward pass
// through the neural network layers (`UMAPModel::forward`). This read-only evaluation does not use
// any unsynchronized interior mutability (such as `Cell`, `RefCell`, or raw pointer mutation).
// Therefore, sharing `NnMapper` across Tokio worker threads via `AppState` is safe and data-race free.
unsafe impl Sync for NnMapper {}

impl NnMapper {
    /// Lädt Modell + Sidecar-Config aus dem Dateisystem.
    /// Benötigt die Modelldatei (.bin) und die Sidecar-Config-Datei (_nn_mapper_config.json).
    pub fn load(model_path: &Path) -> Result<Self, NnMapperError> {
        // Sidecar-Config-Datei laden
        let config_path = model_path.with_extension("_nn_mapper_config.json");
        let config_json = std::fs::read_to_string(&config_path).map_err(|e| {
            NnMapperError::ConfigLoadError(format!("Konnte Config-Datei nicht lesen: {}", e))
        })?;

        let config: NnMapperConfig = serde_json::from_str(&config_json).map_err(|e| {
            NnMapperError::ConfigLoadError(format!("Konnte Config nicht parsen: {}", e))
        })?;

        // Use default device for the backend
        let device = Default::default();

        // Modell laden
        let fitted = FittedUmap::<MyAutodiffBackend>::load(
            model_path,
            config.umap_config,
            config.embedding_dim,
            device,
        )
        .map_err(|e| NnMapperError::ModelLoadError(format!("Konnte Modell nicht laden: {}", e)))?;

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
        let embedding_f64: Vec<Vec<f64>> = vec![embedding.iter().map(|&x| x as f64).collect()];

        // Transform using the fitted model
        let result = self.fitted.transform(embedding_f64);

        // Extract the first (and only) 2D coordinate
        if result.is_empty() || result[0].len() != 2 {
            return Err(NnMapperError::ProjectionError(
                "Unerwartetes Ergebnis von UMAP Transform".to_string(),
            ));
        }

        let x = result[0][0] as f32;
        let y = result[0][1] as f32;

        Ok((x, y))
    }

    pub fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn test_nn_mapper_send_sync() {
        assert_send_sync::<NnMapper>();
    }
}
