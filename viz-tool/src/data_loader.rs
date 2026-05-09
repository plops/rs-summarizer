use std::path::Path;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool, Row};

use crate::errors::VizError;
use crate::embedding::bytes_to_embedding_truncated;

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
/// - Opens the database in read-only mode
/// - Queries all rows with non-null embeddings
/// - Deserializes BLOBs using bytes_to_embedding_truncated
/// - Skips invalid BLOBs with warnings to stderr
/// - Returns statistics about skipped blobs
pub async fn load_compact_db(
    path: &Path,
    embedding_dim: usize,
) -> Result<LoadResult, VizError> {
    // Configure SQLite connection for read-only access
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .create_if_missing(false);

    // Create connection pool
    let pool = SqlitePool::connect_with(options).await?;

    // Query all rows with non-null embeddings
    let rows = sqlx::query(
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
    )
    .fetch_all(&pool)
    .await?;

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
            Err(VizError::InvalidBlobLength(_)) => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use sqlx::SqlitePool;
    use proptest::prelude::*;

    async fn create_test_db() -> Result<(NamedTempFile, Vec<i64>), VizError> {
        let temp_file = NamedTempFile::new().map_err(VizError::Io)?;
        let pool = SqlitePool::connect(&format!("sqlite:{}", temp_file.path().display()))
            .await
            .map_err(VizError::Database)?;

        // Create the summaries table
        sqlx::query(
            "CREATE TABLE summaries (
                identifier INTEGER PRIMARY KEY,
                original_source_link TEXT NOT NULL DEFAULT '',
                model TEXT NOT NULL DEFAULT '',
                embedding BLOB,
                embedding_model TEXT NOT NULL DEFAULT '',
                summary TEXT NOT NULL DEFAULT '',
                summary_timestamp_start TEXT NOT NULL DEFAULT '',
                summary_timestamp_end TEXT NOT NULL DEFAULT '',
                cost REAL NOT NULL DEFAULT 0.0,
                timestamped_summary_in_youtube_format TEXT NOT NULL DEFAULT ''
            )"
        )
        .execute(&pool)
        .await
        .map_err(VizError::Database)?;

        // Insert test data
        let mut ids = Vec::new();
        
        // Valid embedding (4 f32 values = 16 bytes)
        let valid_embedding = vec![1.0f32, 2.0, 3.0, 4.0];
        let valid_bytes: Vec<u8> = valid_embedding
            .iter()
            .flat_map(|&x| x.to_le_bytes().to_vec())
            .collect();

        let result = sqlx::query(
            "INSERT INTO summaries (identifier, original_source_link, summary, embedding, embedding_model, timestamped_summary_in_youtube_format)
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(1)
        .bind("https://example.com/1")
        .bind("Test summary 1")
        .bind(&valid_bytes)
        .bind("text-embedding-ada-002")
        .bind("Timestamped summary 1")
        .execute(&pool)
        .await
        .map_err(VizError::Database)?;
        
        ids.push(result.last_insert_rowid());

        // Invalid embedding (not multiple of 4 bytes)
        let invalid_bytes = vec![1, 2, 3]; // 3 bytes
        let result = sqlx::query(
            "INSERT INTO summaries (identifier, original_source_link, summary, embedding, embedding_model, timestamped_summary_in_youtube_format)
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(2)
        .bind("https://example.com/2")
        .bind("Test summary 2")
        .bind(&invalid_bytes)
        .bind("text-embedding-ada-002")
        .bind("Timestamped summary 2")
        .execute(&pool)
        .await
        .map_err(VizError::Database)?;
        
        ids.push(result.last_insert_rowid());

        // Too short embedding (only 4 bytes, need 8 for dim=2)
        let short_bytes = vec![1, 2, 3, 4]; // 4 bytes = 1 f32, need 2 f32 for dim=2
        let result = sqlx::query(
            "INSERT INTO summaries (identifier, original_source_link, summary, embedding, embedding_model, timestamped_summary_in_youtube_format)
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(3)
        .bind("https://example.com/3")
        .bind("Test summary 3")
        .bind(&short_bytes)
        .bind("text-embedding-ada-002")
        .bind("Timestamped summary 3")
        .execute(&pool)
        .await
        .map_err(VizError::Database)?;
        
        ids.push(result.last_insert_rowid());

        // NULL embedding (should be filtered out)
        let result = sqlx::query(
            "INSERT INTO summaries (identifier, original_source_link, summary, embedding_model, timestamped_summary_in_youtube_format)
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind(4)
        .bind("https://example.com/4")
        .bind("Test summary 4")
        .bind("text-embedding-ada-002")
        .bind("Timestamped summary 4")
        .execute(&pool)
        .await
        .map_err(VizError::Database)?;
        
        ids.push(result.last_insert_rowid());

        pool.close().await;
        Ok((temp_file, ids))
    }

    #[tokio::test]
    async fn test_load_compact_db_valid_and_invalid_blobs() {
        let (temp_file, _ids) = create_test_db().await.unwrap();
        
        let result = load_compact_db(temp_file.path(), 2).await.unwrap();
        
        // Should have 1 valid point (the one with 4 f32 values, truncated to 2)
        assert_eq!(result.points.len(), 1);
        assert_eq!(result.points[0].identifier, 1);
        assert_eq!(result.points[0].embedding.len(), 2);
        assert_eq!(result.points[0].embedding, vec![1.0, 2.0]);
        
        // Should have skipped 1 invalid length and 1 too short
        assert_eq!(result.skipped_invalid_length, 1);
        assert_eq!(result.skipped_too_short, 1);
    }

    #[tokio::test]
    async fn test_load_compact_db_no_embeddings() {
        let temp_file = NamedTempFile::new().unwrap();
        let pool = SqlitePool::connect(&format!("sqlite:{}", temp_file.path().display()))
            .await
            .unwrap();

        // Create empty table
        sqlx::query(
            "CREATE TABLE summaries (
                identifier INTEGER PRIMARY KEY,
                original_source_link TEXT NOT NULL DEFAULT '',
                model TEXT NOT NULL DEFAULT '',
                embedding BLOB,
                embedding_model TEXT NOT NULL DEFAULT '',
                summary TEXT NOT NULL DEFAULT '',
                summary_timestamp_start TEXT NOT NULL DEFAULT '',
                summary_timestamp_end TEXT NOT NULL DEFAULT '',
                cost REAL NOT NULL DEFAULT 0.0,
                timestamped_summary_in_youtube_format TEXT NOT NULL DEFAULT ''
            )"
        )
        .execute(&pool)
        .await
        .unwrap();

        pool.close().await;

        let result = load_compact_db(temp_file.path(), 2).await.unwrap();
        assert_eq!(result.points.len(), 0);
        assert_eq!(result.skipped_invalid_length, 0);
        assert_eq!(result.skipped_too_short, 0);
    }

    // Feature: embedding-visualization, Property 8: Valid BLOB Count
    proptest! {
        #[test]
        fn prop_valid_blob_count(
            // Generate a mix of valid and invalid BLOBs
            blob_data in prop::collection::vec(
                prop::option::of(prop::collection::vec(prop::num::u8::ANY, 1..=100)),
                1..=20
            ),
            embedding_dim in 1usize..=10
        ) {
            // Create a temporary database
            let rt = tokio::runtime::Runtime::new().unwrap();
            let (temp_file, _) = rt.block_on(async {
                create_test_db().await.unwrap()
            });

            // Count valid BLOBs in generated data
            let expected_valid_count = blob_data.iter().filter(|opt_blob| {
                if let Some(blob) = opt_blob {
                    blob.len() % 4 == 0 && blob.len() >= embedding_dim * 4
                } else {
                    false
                }
            }).count();

            // Insert test data into database
            let pool = SqlitePool::connect(&format!("sqlite:{}", temp_file.path().display()))
                .await
                .unwrap();

            for (i, opt_blob) in blob_data.iter().enumerate() {
                if let Some(blob) = opt_blob {
                    let result = sqlx::query(
                        "INSERT INTO summaries (identifier, original_source_link, summary, embedding, embedding_model, timestamped_summary_in_youtube_format)
                         VALUES (?, ?, ?, ?, ?, ?)"
                    )
                    .bind(i as i64 + 100) // Use different IDs to avoid conflicts
                    .bind(format!("https://example.com/{}", i))
                    .bind(format!("Test summary {}", i))
                    .bind(blob)
                    .bind("text-embedding-ada-002")
                    .bind(format!("Timestamped summary {}", i))
                    .execute(&pool)
                    .await
                    .unwrap();
                }
            }

            pool.close().await;

            // Load data and verify count
            let result = rt.block_on(async {
                load_compact_db(temp_file.path(), embedding_dim).await.unwrap()
            });

            // The number of loaded points should equal number of valid BLOBs
            prop_assert_eq!(result.points.len(), expected_valid_count);
        }
    }
}
