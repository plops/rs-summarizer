use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
use std::path::Path;

use crate::embedding::bytes_to_embedding_truncated;
use crate::errors::VizError;

/// Represents a single embedding point loaded from the compact database
#[derive(Debug, Clone)]
pub struct EmbeddingPoint {
    pub identifier: i64,
    pub original_source_link: String,
    pub summary: String,
    pub model: String,
    pub embedding_model: String,
    pub timestamped_summary: String,
    pub embedding: Vec<f32>,
}

/// Result of loading a compact database
#[derive(Debug, Clone)]
pub struct LoadResult {
    pub points: Vec<EmbeddingPoint>,
    pub skipped_invalid_length: usize,
    pub skipped_too_short: usize,
}

/// Loads embeddings from a compact SQLite database
///
/// This function:
/// - Opens database in read-only mode
/// - Queries all rows with non-null embeddings
/// - Deserializes BLOBs using bytes_to_embedding_truncated
/// - Skips invalid BLOBs with warnings to stderr
/// - Returns statistics about skipped blobs
pub async fn load_compact_db(path: &Path, embedding_dim: usize) -> Result<LoadResult, VizError> {
    load_compact_db_subset(path, embedding_dim, 0).await
}

/// Loads a subset of embeddings from a compact SQLite database
///
/// This function:
/// - Opens database in read-only mode
/// - Queries first N rows with non-null embeddings (if limit > 0)
/// - Deserializes BLOBs using bytes_to_embedding_truncated
/// - Skips invalid BLOBs with warnings to stderr
/// - Returns statistics about skipped blobs
pub async fn load_compact_db_subset(
    path: &Path,
    embedding_dim: usize,
    limit: usize,
) -> Result<LoadResult, VizError> {
    // Configure SQLite connection for read-only access
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .create_if_missing(false);

    // Create connection pool
    let pool = SqlitePool::connect_with(options).await?;

    // Query rows with non-null embeddings, optionally limited
    let query = if limit > 0 {
        "SELECT
            identifier,
            original_source_link,
            summary,
            model,
            embedding_model,
            timestamped_summary_in_youtube_format,
            embedding
         FROM summaries
         WHERE embedding IS NOT NULL
         LIMIT ?"
    } else {
        "SELECT
            identifier,
            original_source_link,
            summary,
            model,
            embedding_model,
            timestamped_summary_in_youtube_format,
            embedding
         FROM summaries
         WHERE embedding IS NOT NULL"
    };

    let rows = if limit > 0 {
        sqlx::query(query)
            .bind(limit as i64)
            .fetch_all(&pool)
            .await?
    } else {
        sqlx::query(query).fetch_all(&pool).await?
    };

    let mut points = Vec::new();
    let mut skipped_invalid_length = 0;
    let mut skipped_too_short = 0;

    for row in rows {
        let identifier: i64 = row.get("identifier");
        let original_source_link: String = row.get("original_source_link");
        let summary: String = row.get("summary");
        let model: String = row.get("model");
        let embedding_model: String = row.get("embedding_model");
        let timestamped_summary: String = row.get("timestamped_summary_in_youtube_format");
        let embedding_bytes: Vec<u8> = row.get("embedding");

        // Try to deserialize and truncate the embedding
        match bytes_to_embedding_truncated(&embedding_bytes, embedding_dim) {
            Ok(embedding) => {
                points.push(EmbeddingPoint {
                    identifier,
                    original_source_link,
                    summary,
                    model,
                    embedding_model,
                    timestamped_summary,
                    embedding,
                });
            }
            Err(VizError::InvalidBlobLength { length: _ }) => {
                eprintln!(
                    "Warning: Skipping identifier {} - invalid BLOB length (not multiple of 4)",
                    identifier
                );
                skipped_invalid_length += 1;
            }
            Err(VizError::BlobTooShort { actual, required }) => {
                eprintln!(
                    "Warning: Skipping identifier {} - BLOB too short ({} bytes, required {})",
                    identifier, actual, required
                );
                skipped_too_short += 1;
            }
            Err(e) => {
                // Unexpected error - propagate it
                return Err(e);
            }
        }
    }

    Ok(LoadResult {
        points,
        skipped_invalid_length,
        skipped_too_short,
    })
}
