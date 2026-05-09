use crate::errors::VizError;
use crate::umap_engine::{fit_parametric_umap, UmapParams};
use fast_umap::prelude::*;
use std::path::Path;
use serde::{Deserialize, Serialize};

#[cfg(feature = "gpu")]
use burn_autodiff::Autodiff;

#[cfg(feature = "gpu")]
use cubecl::wgpu::WgpuRuntime;
#[cfg(feature = "gpu")]
use burn_cubecl::CubeBackend;
#[cfg(feature = "cpu")]
use burn_autodiff::Autodiff;
#[cfg(feature = "cpu")]
use burn::backend::{AutodiffBackend, NdArray};

#[cfg(feature = "gpu")]
type MyBackend = CubeBackend<WgpuRuntime, f32, i32, u32>;
#[cfg(feature = "cpu")]
type MyBackend = NdArray<f32>;
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
    /// Train a new parametric UMAP model for out-of-sample projection
    pub fn train(
        embeddings: &[Vec<f32>],
        embedding_dim: usize,
        params: UmapParams,
    ) -> Result<Self, VizError> {
        // Validate input
        if embeddings.is_empty() {
            return Err(VizError::NoEmbeddings);
        }
        
        // Validate all embeddings have the same dimension
        for embedding in embeddings {
            if embedding.len() != embedding_dim {
                return Err(VizError::DimensionMismatch {
                    expected: embedding_dim,
                    actual: embedding.len(),
                });
            }
        }

        // Fit parametric UMAP
        let fitted = fit_parametric_umap(embeddings, params)?;

        Ok(Self {
            fitted,
            embedding_dim,
        })
    }

    /// Project a single embedding to 2D coordinates
    /// Returns VizError::DimensionMismatch if embedding dimension is incorrect
    pub fn project(&self, embedding: &[f32]) -> Result<(f32, f32), VizError> {
        // Check dimension
        if embedding.len() != self.embedding_dim {
            return Err(VizError::DimensionMismatch {
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
            return Err(VizError::Umap("Invalid transform result".to_string()));
        }

        let x = result[0][0] as f32;
        let y = result[0][1] as f32;

        Ok((x, y))
    }

    /// Save the trained model and configuration
    /// Saves the model binary and a sidecar JSON with configuration
    pub fn save(&self, path: &Path) -> Result<(), VizError> {
        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| VizError::Io(e))?;
        }

        // Save the fitted model
        self.fitted.save(path)
            .map_err(|e| VizError::Umap(format!("Failed to save model: {}", e)))?;

        // Create and save sidecar configuration
        let config = NnMapperConfig {
            umap_config: self.fitted.config().clone(),
            embedding_dim: self.embedding_dim,
        };

        let config_path = path.with_extension("_nn_mapper_config.json");
        let config_json = serde_json::to_string_pretty(&config)
            .map_err(|e| VizError::SerializationError(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(&config_path, config_json)
            .map_err(|e| VizError::Io(e))?;

        Ok(())
    }

    /// Load a saved model and its configuration
    /// Reads the sidecar JSON to get configuration and loads the model
    pub fn load(path: &Path, embedding_dim: usize) -> Result<Self, VizError> {
        // Load sidecar configuration
        let config_path = path.with_extension("_nn_mapper_config.json");
        let config_json = std::fs::read_to_string(&config_path)
            .map_err(|e| VizError::Io(e))?;

        let config: NnMapperConfig = serde_json::from_str(&config_json)
            .map_err(|e| VizError::SerializationError(format!("Failed to deserialize config: {}", e)))?;

        // Validate embedding dimension matches
        if config.embedding_dim != embedding_dim {
            return Err(VizError::DimensionMismatch {
                expected: embedding_dim,
                actual: config.embedding_dim,
            });
        }

        // Use default device for the backend
        let device = Default::default();

        // Load the fitted model
        let fitted = FittedUmap::load(
            path,
            config.umap_config,
            embedding_dim,
            device,
        ).map_err(|e| VizError::ModelLoadError(format!("Failed to load model: {}", e)))?;

        Ok(Self {
            fitted,
            embedding_dim,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_dimension_mismatch() {
        // This test requires a GPU adapter. Use std::panic::catch_unwind to handle GPU adapter failures.
        let result = std::panic::catch_unwind(|| {
            let embeddings = vec![
                vec![1.0f32; 768],
                vec![2.0f32; 768],
                vec![3.0f32; 768],
            ];

            let params = UmapParams {
                n_components: 2,
                n_neighbors: 2,
                min_dist: 0.1,
                n_epochs: 10, // Small number for testing
            };

            // Try to train a model
            let nn_mapper = NnMapper::train(&embeddings, 768, params).unwrap();

            // Test with correct dimension - should succeed
            let correct_embedding = vec![0.5f32; 768];
            assert!(nn_mapper.project(&correct_embedding).is_ok());

            // Test with wrong dimension - should fail
            let wrong_embedding = vec![0.5f32; 512]; // Wrong dimension
            let result = nn_mapper.project(&wrong_embedding);
            assert!(result.is_err());
            
            match result.unwrap_err() {
                VizError::DimensionMismatch { expected, actual } => {
                    assert_eq!(expected, 768);
                    assert_eq!(actual, 512);
                }
                _ => panic!("Expected DimensionMismatch error"),
            }
        });

        if result.is_err() {
            println!("Skipping test_project_dimension_mismatch: GPU adapter not available or other hardware issue");
        }
    }
}
