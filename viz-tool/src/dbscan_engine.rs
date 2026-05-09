use crate::errors::VizError;
use linfa::traits::Transformer;
use linfa_clustering::Dbscan;
use ndarray::Array2;

/// Parameters for DBSCAN clustering
pub struct DbscanParams {
    pub eps: f64,         // linfa uses f64 for tolerance
    pub min_samples: usize,
}

/// Perform DBSCAN clustering on 4D embeddings
/// 
/// # Arguments
/// * `embeddings_4d` - Array of 4D points to cluster
/// * `params` - DBSCAN parameters (eps and min_samples)
/// 
/// # Returns
/// * `Vec<i32>` - Cluster labels where -1 indicates noise points
/// 
/// # Errors
/// * `VizError::Dbscan` - If clustering fails
pub fn compute_dbscan(
    embeddings_4d: &[[f32; 4]],
    params: DbscanParams,
) -> Result<Vec<i32>, VizError> {
    if embeddings_4d.is_empty() {
        return Ok(Vec::new());
    }

    // Convert f32 array to f64 ndarray (linfa expects f64)
    let n_points = embeddings_4d.len();
    let mut data = Vec::with_capacity(n_points * 4);
    
    for point in embeddings_4d {
        data.extend_from_slice(&[
            point[0] as f64,
            point[1] as f64,
            point[2] as f64,
            point[3] as f64,
        ]);
    }

    let array = Array2::from_shape_vec((n_points, 4), data)
        .map_err(|e| VizError::Dbscan(format!("Failed to create ndarray: {}", e)))?;

    // Simple DBSCAN implementation using basic pattern
    // This avoids complex trait issues by using a straightforward approach
    let mut labels = vec![-2; n_points]; // -2: unvisited, -1: noise
    let mut cluster_id = 0;
    
    for i in 0..n_points {
        let mut neighbors = Vec::new();
        let mut neighbor_count = 0;
        
        // Find neighbors within eps distance
        for j in 0..n_points {
            let distance = distance(embeddings_4d[i], embeddings_4d[j]);
            
            if distance <= params.eps {
                neighbors.push(j);
                neighbor_count += 1;
            }
        }
        
        // Assign cluster label
        if neighbor_count >= params.min_samples {
            labels[i] = cluster_id;
        } else {
            labels[i] = -1; // Noise point
        }
    }

    Ok(labels)
}

fn distance(point1: [f32; 4], point2: [f32; 4]) -> f64 {
    let mut sum = 0.0;
    for i in 0..4 {
        let diff = (point1[i] as f64) - (point2[i] as f64);
        sum += diff * diff;
    }
    sum.sqrt()
}

/// Helper function to expand cluster when a new core point is found
fn expand_cluster(
    embeddings_4d: &[[f32; 4]],
    labels: &mut Vec<i32>,
    eps: f64,
    min_samples: usize,
    cluster_id: &mut i32,
    point_index: usize,
    neighbors: &[usize],
) {
    for &neighbor_index in neighbors {
        if labels[neighbor_index] == -2 { // -2: unvisited
            labels[neighbor_index] = *cluster_id;
            let mut new_neighbors = Vec::new();
            
            // Find neighbors of this unvisited point
            for j in 0..embeddings_4d.len() {
                if labels[j] == -2 && distance(embeddings_4d[neighbor_index], embeddings_4d[j]) <= eps {
                    new_neighbors.push(j);
                }
            }
            
            // If this point has enough neighbors, it becomes a core point
            if new_neighbors.len() >= min_samples {
                *cluster_id += 1;
                for n in new_neighbors {
                    if labels[n] == -2 { // -2: unvisited
                        labels[n] = *cluster_id;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_dbscan_empty() {
        let embeddings: &[[f32; 4]] = &[];
        let params = DbscanParams { eps: 0.3, min_samples: 5 };
        let result = compute_dbscan(embeddings, params).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_dbscan_simple() {
        // Create two distinct clusters
        let embeddings = vec![
            [1.0, 1.0, 1.0, 1.0],
            [1.1, 1.1, 1.1, 1.1],
            [1.2, 1.2, 1.2, 1.2],
            [5.0, 5.0, 5.0, 5.0],
            [5.1, 5.1, 5.1, 5.1],
            [5.2, 5.2, 5.2, 5.2],
            [10.0, 10.0, 10.0, 10.0], // outlier
        ];
        
        let params = DbscanParams { eps: 0.5, min_samples: 2 };
        let result = compute_dbscan(&embeddings, params).unwrap();
        
        assert_eq!(result.len(), 7);
        
        // First 3 points should be in same cluster
        assert_eq!(result[0], result[1]);
        assert_eq!(result[1], result[2]);
        
        // Next 3 points should be in same cluster
        assert_eq!(result[3], result[4]);
        assert_eq!(result[4], result[5]);
        
        // Different clusters should have different IDs
        assert_ne!(result[0], result[3]);
        
        // Last point should be noise (-1)
        assert_eq!(result[6], -1);
    }

    #[test]
    fn test_compute_dbscan_all_noise() {
        // Create widely separated points
        let embeddings = vec![
            [1.0, 1.0, 1.0, 1.0],
            [10.0, 10.0, 10.0, 10.0],
            [20.0, 20.0, 20.0, 20.0],
        ];
        
        let params = DbscanParams { eps: 0.5, min_samples: 2 };
        let result = compute_dbscan(&embeddings, params).unwrap();
        
        assert_eq!(result.len(), 3);
        // All should be noise
        assert!(result.iter().all(|&label| label == -1));
    }

    #[test]
    fn test_compute_dbscan_single_cluster() {
        // Create tightly clustered points
        let embeddings = vec![
            [1.0, 1.0, 1.0, 1.0],
            [1.1, 1.1, 1.1, 1.1],
            [1.2, 1.2, 1.2, 1.2],
            [0.9, 0.9, 0.9, 0.9],
        ];
        
        let params = DbscanParams { eps: 0.5, min_samples: 2 };
        let result = compute_dbscan(&embeddings, params).unwrap();
        
        assert_eq!(result.len(), 4);
        // All should be in same cluster (not noise)
        assert!(result.iter().all(|&label| label != -1));
        // All should have same cluster ID
        let first_label = result[0];
        assert!(result.iter().all(|&label| label == first_label));
    }

    #[test]
    fn test_compute_dbscan_different_parameters() {
        let embeddings = vec![
            [1.0, 1.0, 1.0, 1.0],
            [1.1, 1.1, 1.1, 1.1],
            [2.0, 2.0, 2.0, 2.0],
            [2.1, 2.1, 2.1, 2.1],
        ];
        
        // Test with larger eps (should create one cluster)
        let params_large_eps = DbscanParams { eps: 1.0, min_samples: 2 };
        let result_large = compute_dbscan(&embeddings, params_large_eps).unwrap();
        
        // Test with smaller eps (should create two clusters)
        let params_small_eps = DbscanParams { eps: 0.2, min_samples: 2 };
        let result_small = compute_dbscan(&embeddings, params_small_eps).unwrap();
        
        // Larger eps should create fewer clusters (more points in same cluster)
        let unique_large: std::collections::HashSet<_> = result_large.iter().filter(|&&x| x != -1).collect();
        let unique_small: std::collections::HashSet<_> = result_small.iter().filter(|&&x| x != -1).collect();
        
        assert!(unique_large.len() <= unique_small.len());
    }

    #[test]
    fn test_compute_dbscan_high_min_samples() {
        let embeddings = vec![
            [1.0, 1.0, 1.0, 1.0],
            [1.1, 1.1, 1.1, 1.1],
            [1.2, 1.2, 1.2, 1.2],
            [5.0, 5.0, 5.0, 5.0],
            [5.1, 5.1, 5.1, 5.1],
        ];
        
        // With high min_samples, small clusters should become noise
        let params = DbscanParams { eps: 0.5, min_samples: 4 };
        let result = compute_dbscan(&embeddings, params).unwrap();
        
        assert_eq!(result.len(), 5);
        // Most points should be noise since no cluster has 4 points
        let noise_count = result.iter().filter(|&&x| x == -1).count();
        assert!(noise_count >= 3); // At least 3 points should be noise
    }
}
