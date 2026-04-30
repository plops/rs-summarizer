use gemini_rust::{Gemini, Model, TaskType};
use sqlx::SqlitePool;

use crate::db;
use crate::errors::EmbeddingError;

/// Service for generating text embeddings and performing similarity search.
pub struct EmbeddingService {
    api_key: String,
    model: String,
    dimensions: usize,
}

impl EmbeddingService {
    /// Create a new EmbeddingService with the given API key, model name, and output dimensions.
    pub fn new(api_key: String, model: &str, dimensions: usize) -> Self {
        Self {
            api_key,
            model: model.to_string(),
            dimensions,
        }
    }

    /// Generate an embedding vector for the given text using the Gemini embedding model.
    pub async fn embed_text(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if text.is_empty() {
            return Err(EmbeddingError::EmptyText);
        }

        let client = Gemini::with_model(&self.api_key, Model::TextEmbedding004)
            .map_err(|e| EmbeddingError::ApiError(e.to_string()))?;

        let response = client
            .embed_content()
            .with_text(text)
            .with_task_type(TaskType::RetrievalDocument)
            .with_output_dimensionality(self.dimensions as i32)
            .execute()
            .await
            .map_err(|e| EmbeddingError::ApiError(e.to_string()))?;

        Ok(response.embedding.values)
    }

    /// Compute cosine similarity between two embedding vectors.
    /// Supports Matryoshka truncation: if vectors differ in length, truncates to the shorter.
    /// Returns 0.0 if either vector has zero magnitude.
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        assert!(!a.is_empty() && !b.is_empty());

        // Handle Matryoshka dimension mismatch: truncate to shorter vector
        let len = a.len().min(b.len());
        let a = &a[..len];
        let b = &b[..len];

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    /// Find the top-k most similar embeddings in the database to the given query embedding.
    /// Returns a vector of (identifier, similarity_score) pairs sorted by descending similarity.
    pub async fn find_similar(
        &self,
        db_pool: &SqlitePool,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<(i64, f32)>, EmbeddingError> {
        let rows = db::fetch_all_embeddings(db_pool).await?;

        let mut similarities: Vec<(i64, f32)> = rows
            .iter()
            .filter_map(|(identifier, blob)| {
                let embedding = bytes_to_embedding(blob);
                if embedding.is_empty() {
                    return None;
                }
                let similarity = Self::cosine_similarity(query_embedding, &embedding);
                Some((*identifier, similarity))
            })
            .collect();

        // Sort by similarity descending
        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Return top-k results
        similarities.truncate(top_k);

        Ok(similarities)
    }
}

/// Convert a raw byte blob (little-endian f32s) to a Vec<f32>.
fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Convert a Vec<f32> embedding to raw bytes (little-endian).
#[allow(dead_code)]
pub fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = EmbeddingService::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = EmbeddingService::cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        let sim = EmbeddingService::cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_magnitude() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = EmbeddingService::cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_matryoshka_truncation() {
        // Longer vector gets truncated to match shorter
        let a = vec![1.0, 0.0, 0.0, 99.0, 99.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = EmbeddingService::cosine_similarity(&a, &b);
        // After truncation, a becomes [1.0, 0.0, 0.0], identical to b
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_bytes_to_embedding_roundtrip() {
        let original = vec![1.0f32, -2.5, 3.14, 0.0];
        let bytes = embedding_to_bytes(&original);
        let recovered = bytes_to_embedding(&bytes);
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_bytes_to_embedding_empty() {
        let bytes: Vec<u8> = vec![];
        let result = bytes_to_embedding(&bytes);
        assert!(result.is_empty());
    }

    #[test]
    #[should_panic]
    fn test_cosine_similarity_empty_vector_panics() {
        let a: Vec<f32> = vec![];
        let b = vec![1.0, 2.0];
        EmbeddingService::cosine_similarity(&a, &b);
    }
}
