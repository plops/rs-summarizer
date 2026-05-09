use crate::errors::VizError;
use cubecl::wgpu::WgpuRuntime;
use fast_umap::prelude::*;
use burn_autodiff::Autodiff;
use burn_cubecl::CubeBackend;

type MyBackend = CubeBackend<WgpuRuntime, f32, i32, u32>;
type MyAutodiffBackend = Autodiff<MyBackend>;

/// UMAP computation parameters
#[derive(Debug, Clone)]
pub struct UmapParams {
    pub n_components: usize,
    pub n_neighbors: usize,
    pub min_dist: f32,
    pub n_epochs: usize,
}

impl Default for UmapParams {
    fn default() -> Self {
        Self {
            n_components: 2,
            n_neighbors: 15,
            min_dist: 0.1,
            n_epochs: 200,
        }
    }
}

/// Computes UMAP embeddings (non-parametric) using WGPU backend
/// Returns the reduced dimension coordinates
pub fn compute_umap(
    embeddings: &[Vec<f32>],
    params: UmapParams,
) -> Result<Vec<Vec<f32>>, VizError> {
    if embeddings.is_empty() {
        return Err(VizError::NoEmbeddings);
    }

    if params.n_neighbors >= embeddings.len() {
        return Err(VizError::InsufficientPoints {
            actual: embeddings.len(),
            required: params.n_neighbors + 1,
        });
    }

    // Convert f32 embeddings to f64 for fast-umap
    let data: Vec<Vec<f64>> = embeddings
        .iter()
        .map(|emb| emb.iter().map(|&x| x as f64).collect())
        .collect();

    // Create UMAP configuration
    let config = UmapConfig {
        n_components: params.n_components,
        hidden_sizes: vec![100, 100, 100],
        graph: GraphParams {
            n_neighbors: params.n_neighbors,
            metric: Metric::Euclidean,
            ..Default::default()
        },
        manifold: ManifoldParams {
            min_dist: params.min_dist,
            spread: 1.0,
            ..Default::default()
        },
        optimization: OptimizationParams {
            n_epochs: params.n_epochs,
            learning_rate: 1e-3,
            ..Default::default()
        },
        ..Default::default()
    };

    // Create and fit UMAP model
    let umap = fast_umap::Umap::<MyAutodiffBackend>::new(config);
    
    let fitted = umap.fit(data, None);

    // Extract embeddings and convert back to f32
    let embedding = fitted.embedding();
    let result: Vec<Vec<f32>> = embedding
        .iter()
        .map(|emb: &Vec<f64>| emb.iter().map(|&x| x as f32).collect())
        .collect();

    Ok(result)
}

/// Fits parametric UMAP (WGPU backend) - supports transform()
/// Used for NN_Mapper training
pub fn fit_parametric_umap(
    embeddings: &[Vec<f32>],
    params: UmapParams,
) -> Result<FittedUmap<MyAutodiffBackend>, VizError> {
    if embeddings.is_empty() {
        return Err(VizError::NoEmbeddings);
    }

    if params.n_neighbors >= embeddings.len() {
        return Err(VizError::InsufficientPoints {
            actual: embeddings.len(),
            required: params.n_neighbors + 1,
        });
    }

    // Convert f32 embeddings to f64 for fast-umap
    let data: Vec<Vec<f64>> = embeddings
        .iter()
        .map(|emb| emb.iter().map(|&x| x as f64).collect())
        .collect();

    // Create UMAP configuration
    let config = UmapConfig {
        n_components: params.n_components,
        hidden_sizes: vec![100, 100, 100],
        graph: GraphParams {
            n_neighbors: params.n_neighbors,
            metric: Metric::Euclidean,
            ..Default::default()
        },
        manifold: ManifoldParams {
            min_dist: params.min_dist,
            spread: 1.0,
            ..Default::default()
        },
        optimization: OptimizationParams {
            n_epochs: params.n_epochs,
            learning_rate: 1e-3,
            ..Default::default()
        },
        ..Default::default()
    };

    // Create and fit UMAP model
    let umap = fast_umap::Umap::<MyAutodiffBackend>::new(config);
    
    let fitted = umap.fit(data, None);

    Ok(fitted)
}
