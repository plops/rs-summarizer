use crate::errors::VizError;

/// UMAP computation parameters
#[derive(Debug, Clone)]
pub struct UmapParams {
    pub n_components: usize,
    pub n_neighbors: usize,
    pub min_dist: f32,
    pub n_epochs: usize,
    /// Learning rate for optimization (used by parametric and CPU training)
    pub learning_rate: f64,
    /// Hidden sizes for parametric UMAP neural network (parametric only)
    pub hidden_sizes: Vec<usize>,
}

impl Default for UmapParams {
    fn default() -> Self {
        Self {
            n_components: 2,
            n_neighbors: 15,
            min_dist: 0.1,
            n_epochs: 200,
            learning_rate: 1e-3,
            hidden_sizes: vec![100, 100, 100],
        }
    }
}

/// Computes UMAP embeddings. Prefer GPU parametric UMAP when the `gpu` feature
/// is enabled; otherwise use the fast-umap CPU backend when the `cpu` feature
/// is enabled. If neither feature is enabled, return an error.
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

    // GPU parametric UMAP (preferred if available)
    #[cfg(feature = "gpu")]
    {
        use burn_autodiff::Autodiff;
        use burn_cubecl::CubeBackend;
        use cubecl::wgpu::WgpuRuntime;
        use fast_umap::prelude::*;
        use fast_umap::{GraphParams, ManifoldParams, Metric, OptimizationParams, UmapConfig};

        type MyBackend = CubeBackend<WgpuRuntime, f32, i32, u32>;
        type MyAutodiffBackend = Autodiff<MyBackend>;

        let data: Vec<Vec<f64>> = embeddings
            .iter()
            .map(|emb| emb.iter().map(|&x| x as f64).collect())
            .collect();

        let config = UmapConfig {
            n_components: params.n_components,
            hidden_sizes: params.hidden_sizes.clone(),
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
                learning_rate: params.learning_rate,
                ..Default::default()
            },
            ..Default::default()
        };

        use crossbeam_channel::unbounded;
        use indicatif::{ProgressBar, ProgressStyle};

        let umap = fast_umap::Umap::<MyAutodiffBackend>::new(config);

        // Create a progress bar for epoch-level reporting
        let pb = ProgressBar::new(params.n_epochs as u64);
        pb.set_style(
            ProgressStyle::with_template(
                "{msg} {bar:40.cyan/blue} {pos}/{len} epochs [{elapsed_precise}] {eta}",
            )
            .unwrap()
            .progress_chars("=>-"),
        );
        pb.set_message("UMAP training");

        // Channel for graceful cancellation (currently unused, but required by API)
        let (_exit_tx, exit_rx) = unbounded::<()>();

        let pb_clone = pb.clone();
        let on_progress = Box::new(move |p: fast_umap::EpochProgress| {
            // epoch is 1-based in progress reports
            let pos = p.epoch as u64;
            pb_clone.set_position(pos);
            pb_clone.set_message(format!("loss={:.6}", p.loss));
            if pos >= p.total_epochs as u64 {
                pb_clone.finish_with_message("UMAP training complete");
            }
        });

        let fitted =
            std::panic::catch_unwind(|| umap.fit_with_progress(data, None, exit_rx, on_progress));

        match fitted {
            Ok(result) => {
                // Ensure progress bar finished
                pb.finish_and_clear();

                let embedding = result.embedding();
                let result_vec: Vec<Vec<f32>> = embedding
                    .iter()
                    .map(|emb: &Vec<f64>| emb.iter().map(|&x| x as f32).collect())
                    .collect();
                return Ok(result_vec);
            }
            Err(_) => {
                pb.finish_and_clear();
                return Err(VizError::Umap("GPU UMAP fitting panicked".to_string()));
            }
        }
    }

    // CPU backend for fast-umap (classical UMAP)
    #[cfg(all(not(feature = "gpu"), feature = "cpu"))]
    {
        use fast_umap::cpu_backend::api as cpu_api;
        use fast_umap::{GraphParams, ManifoldParams, Metric, OptimizationParams, UmapConfig};

        let data: Vec<Vec<f64>> = embeddings
            .iter()
            .map(|emb| emb.iter().map(|&x| x as f64).collect())
            .collect();

        let config = UmapConfig {
            n_components: params.n_components,
            hidden_sizes: params.hidden_sizes.clone(),
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
                learning_rate: params.learning_rate,
                ..Default::default()
            },
            ..Default::default()
        };

        // Fit using CPU UMAP
        let fitted = cpu_api::fit_cpu(config, data, None);
        let embedding = fitted.embedding();
        let result_vec: Vec<Vec<f32>> = embedding
            .iter()
            .map(|emb: &Vec<f64>| emb.iter().map(|&x| x as f32).collect())
            .collect();
        return Ok(result_vec);
    }

    // If neither CPU nor GPU fast-umap is enabled, return an error
    Err(VizError::Umap(
        "fast-umap is not enabled in this build (enable the 'cpu' or 'gpu' feature)".to_string(),
    ))
}

// Parametric UMAP fit (GPU only)
#[cfg(feature = "gpu")]
pub fn fit_parametric_umap(
    embeddings: &[Vec<f32>],
    params: UmapParams,
) -> Result<
    fast_umap::FittedUmap<
        burn_autodiff::Autodiff<burn_cubecl::CubeBackend<cubecl::wgpu::WgpuRuntime, f32, i32, u32>>,
    >,
    VizError,
> {
    use burn_autodiff::Autodiff;
    use burn_cubecl::CubeBackend;
    use cubecl::wgpu::WgpuRuntime;
    use fast_umap::prelude::*;
    use fast_umap::{
        FittedUmap, GraphParams, ManifoldParams, Metric, OptimizationParams, UmapConfig,
    };

    if embeddings.is_empty() {
        return Err(VizError::NoEmbeddings);
    }

    if params.n_neighbors >= embeddings.len() {
        return Err(VizError::InsufficientPoints {
            actual: embeddings.len(),
            required: params.n_neighbors + 1,
        });
    }

    let data: Vec<Vec<f64>> = embeddings
        .iter()
        .map(|emb| emb.iter().map(|&x| x as f64).collect())
        .collect();

    let config = UmapConfig {
        n_components: params.n_components,
        hidden_sizes: params.hidden_sizes.clone(),
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
            learning_rate: params.learning_rate,
            ..Default::default()
        },
        ..Default::default()
    };

    let umap =
        fast_umap::Umap::<burn_autodiff::Autodiff<CubeBackend<WgpuRuntime, f32, i32, u32>>>::new(
            config,
        );
    let fitted = umap.fit(data, None);
    Ok(fitted)
}

// Non-gpu builds don't support parametric UMAP
#[cfg(not(feature = "gpu"))]
#[allow(dead_code)]
pub fn fit_parametric_umap(_embeddings: &[Vec<f32>], _params: UmapParams) -> Result<(), VizError> {
    Err(VizError::ComputationError(
        "Parametric UMAP requires GPU feature".to_string(),
    ))
}
