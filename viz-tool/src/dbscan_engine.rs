use crate::errors::VizError;
use linfa::prelude::*;
use linfa_clustering::Dbscan;
use ndarray::Array2;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DbscanParams {
    pub eps: f64,
    pub min_samples: usize,
}

impl Default for DbscanParams {
    fn default() -> Self {
        Self {
            eps: 0.3,
            min_samples: 5,
        }
    }
}

pub struct DbscanEngine;

impl DbscanEngine {
    pub fn new() -> Self {
        Self {}
    }

    pub fn compute_dbscan(&self, embeddings_4d: &[[f32; 4]], params: DbscanParams) -> Result<Vec<i32>, VizError> {
        // Convert embeddings to linfa format (expects f64)
        let n_samples = embeddings_4d.len();
        let mut data = Vec::with_capacity(n_samples * 4);
        
        for embedding in embeddings_4d {
            for &value in embedding {
                data.push(value as f64);
            }
        }

        // Create ndarray Array2
        let dataset = Array2::from_shape_vec((n_samples, 4), data)
            .map_err(|e| VizError::Dbscan(format!("Failed to create dataset: {}", e)))?;

        // Create DBSCAN algorithm
        let dbscan = Dbscan::params()
            .min_samples(params.min_samples)
            .tolerance(params.eps);

        // Fit model
        let model = dbscan.fit(&dataset)
            .map_err(|e| VizError::Dbscan(format!("DBSCAN fitting failed: {}", e)))?;

        // Predict clusters
        let predictions = model.predict(&dataset);

        // Convert linfa's Option<usize> to i32 (None -> -1, Some(id) -> id as i32)
        let clusters: Vec<i32> = predictions
            .into_iter()
            .map(|cluster_id| match cluster_id {
                Some(id) => id as i32,
                None => -1, // Noise points in DBSCAN are labeled as -1
            })
            .collect();

        Ok(clusters)
    }
}
