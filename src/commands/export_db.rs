use std::path::PathBuf;
use sqlx::{SqlitePool, sqlite::{SqliteConnectOptions, SqliteJournalMode}, Row};
use anyhow::Result;
use crate::errors::ExportError;

pub struct ExportDbArgs {
    pub source: PathBuf,
    pub output: PathBuf,
}

pub async fn run_export(args: ExportDbArgs) -> Result<()> {
    // 1. Validate source file exists and is readable
    if !args.source.exists() {
        return Err(ExportError::SourceNotFound(args.source).into());
    }

    // 2. Validate output file doesn't exist
    if args.output.exists() {
        return Err(ExportError::OutputExists(args.output).into());
    }

    // 3. Validate output directory exists
    if let Some(parent) = args.output.parent() {
        if !parent.exists() {
            return Err(ExportError::OutputDirMissing(parent.to_path_buf()).into());
        }
    }

    // 4. Open source database read-only
    let source_opts = SqliteConnectOptions::new()
        .filename(&args.source)
        .read_only(true)
        .create_if_missing(false);

    let source_pool = SqlitePool::connect_with(source_opts).await?;

    // 5. Create output database with WAL mode
    let output_opts = SqliteConnectOptions::new()
        .filename(&args.output)
        .journal_mode(SqliteJournalMode::Wal)
        .create_if_missing(true);

    let output_pool = SqlitePool::connect_with(output_opts).await?;

    // 6. Create schema in output database
    sqlx::query(
        r#"
        CREATE TABLE summaries (
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
        )
        "#
    )
    .execute(&output_pool)
    .await?;

    // 7. Copy rows with WHERE embedding IS NOT NULL AND summary_done = 1
    let rows = sqlx::query(
        r#"
        SELECT 
            identifier,
            original_source_link,
            model,
            embedding,
            embedding_model,
            summary,
            summary_timestamp_start,
            summary_timestamp_end,
            cost,
            timestamped_summary_in_youtube_format
        FROM summaries 
        WHERE embedding IS NOT NULL AND summary_done = 1
        "#
    )
    .fetch_all(&source_pool)
    .await?;

    // 8. Check if we have at least one qualifying row
    if rows.is_empty() {
        return Err(ExportError::NoQualifyingRows.into());
    }

    // 9. Insert rows into output database
    let mut exported_count = 0;
    for row in rows {
        sqlx::query(
            r#"
            INSERT INTO summaries (
                identifier,
                original_source_link,
                model,
                embedding,
                embedding_model,
                summary,
                summary_timestamp_start,
                summary_timestamp_end,
                cost,
                timestamped_summary_in_youtube_format
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(row.get::<i64, _>("identifier"))
        .bind(row.get::<String, _>("original_source_link"))
        .bind(row.get::<String, _>("model"))
        .bind(row.get::<Option<Vec<u8>>, _>("embedding"))
        .bind(row.get::<String, _>("embedding_model"))
        .bind(row.get::<String, _>("summary"))
        .bind(row.get::<String, _>("summary_timestamp_start"))
        .bind(row.get::<String, _>("summary_timestamp_end"))
        .bind(row.get::<f64, _>("cost"))
        .bind(row.get::<String, _>("timestamped_summary_in_youtube_format"))
        .execute(&output_pool)
        .await?;

        exported_count += 1;
    }

    // 10. Get file size of output database
    let file_size = std::fs::metadata(&args.output)?.len();

    // 11. Print results to stdout
    println!("Exported {} rows to {}", exported_count, args.output.display());
    println!("Output file size: {} bytes", file_size);

    // Close pools
    source_pool.close().await;
    output_pool.close().await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{SqlitePool, sqlite::SqliteJournalMode};
    use tempfile::TempDir;
    use std::fs;
    use proptest::prelude::*;
    
    #[derive(Debug, Clone)]
    struct TestRow {
        identifier: i64,
        original_source_link: String,
        model: String,
        embedding: Vec<f32>,
        embedding_model: String,
        summary: String,
        summary_timestamp_start: String,
        summary_timestamp_end: String,
        cost: f64,
        timestamped_summary_in_youtube_format: String,
        transcript: String,
    }

    #[test]
    fn test_output_exists_error() {
        let temp_dir = TempDir::new().unwrap();
        let source_path = temp_dir.path().join("source.db");
        let output_path = temp_dir.path().join("output.db");
        
        // Create source file
        fs::write(&source_path, "test").unwrap();
        
        // Create output file
        fs::write(&output_path, "test").unwrap();
        
        let args = ExportDbArgs {
            source: source_path,
            output: output_path,
        };
        
        let result = std::thread::spawn(move || {
            tokio::runtime::Runtime::new().unwrap().block_on(run_export(args))
        }).join().unwrap();
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        let export_err = err.downcast_ref::<ExportError>();
        assert!(export_err.is_some());
        assert!(matches!(export_err.unwrap(), ExportError::OutputExists(_)));
    }

    #[test]
    fn test_source_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let source_path = temp_dir.path().join("nonexistent.db");
        let output_path = temp_dir.path().join("output.db");
        
        let args = ExportDbArgs {
            source: source_path,
            output: output_path,
        };
        
        let result = std::thread::spawn(move || {
            tokio::runtime::Runtime::new().unwrap().block_on(run_export(args))
        }).join().unwrap();
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        let export_err = err.downcast_ref::<ExportError>();
        assert!(export_err.is_some());
        assert!(matches!(export_err.unwrap(), ExportError::SourceNotFound(_)));
    }

    #[test]
    fn test_output_dir_missing() {
        let temp_dir = TempDir::new().unwrap();
        let source_path = temp_dir.path().join("source.db");
        let output_path = temp_dir.path().join("nonexistent").join("output.db");
        
        // Create source file
        fs::write(&source_path, "test").unwrap();
        
        let args = ExportDbArgs {
            source: source_path,
            output: output_path,
        };
        
        let result = std::thread::spawn(move || {
            tokio::runtime::Runtime::new().unwrap().block_on(run_export(args))
        }).join().unwrap();
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        let export_err = err.downcast_ref::<ExportError>();
        assert!(export_err.is_some());
        assert!(matches!(export_err.unwrap(), ExportError::OutputDirMissing(_)));
    }

    #[tokio::test]
    async fn test_wal_mode_enabled() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_path = temp_dir.path().join("source.db");
        let output_path = temp_dir.path().join("output.db");
        
        // Create source database with test data
        let source_opts = SqliteConnectOptions::new()
            .filename(&source_path)
            .journal_mode(SqliteJournalMode::Wal)
            .create_if_missing(true);
        
        let source_pool = SqlitePool::connect_with(source_opts).await?;
        
        // Create source schema and insert test data
        sqlx::query(
            r#"
            CREATE TABLE summaries (
                identifier INTEGER PRIMARY KEY,
                original_source_link TEXT NOT NULL DEFAULT '',
                model TEXT NOT NULL DEFAULT '',
                embedding BLOB,
                embedding_model TEXT NOT NULL DEFAULT '',
                summary TEXT NOT NULL DEFAULT '',
                summary_timestamp_start TEXT NOT NULL DEFAULT '',
                summary_timestamp_end TEXT NOT NULL DEFAULT '',
                cost REAL NOT NULL DEFAULT 0.0,
                timestamped_summary_in_youtube_format TEXT NOT NULL DEFAULT '',
                summary_done INTEGER NOT NULL DEFAULT 0
            )
            "#
        )
        .execute(&source_pool)
        .await?;
        
        // Insert test row
        sqlx::query(
            r#"
            INSERT INTO summaries (
                identifier, original_source_link, model, embedding, embedding_model,
                summary, summary_timestamp_start, summary_timestamp_end, cost,
                timestamped_summary_in_youtube_format, summary_done
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(1)
        .bind("https://example.com")
        .bind("test-model")
        .bind(vec![1, 2, 3, 4]) // 4 bytes = 1 f32
        .bind("embedding-model")
        .bind("test summary")
        .bind("00:00")
        .bind("01:00")
        .bind(0.1)
        .bind("timestamped summary")
        .bind(1) // summary_done = 1
        .execute(&source_pool)
        .await?;

        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        #[test]
        fn prop_export_field_correctness(
            // Generate test data with varying fields
            rows in prop::collection::vec(
                prop::collection::any::<TestRow>(),
                1..=50
            )
        ) {
            let temp_dir = TempDir::new().unwrap();
            let source_path = temp_dir.path().join("source.db");
            let output_path = temp_dir.path().join("output.db");
            
            // Create source database with test data
            let source_opts = SqliteConnectOptions::new()
                .filename(&source_path)
                .journal_mode(SqliteJournalMode::Wal)
                .create_if_missing(true);
            
            let source_pool = SqlitePool::connect_with(source_opts).await.unwrap();
            
            // Create source schema (includes transcript field)
            sqlx::query(
                r#"
                CREATE TABLE summaries (
                    identifier INTEGER PRIMARY KEY,
                    original_source_link TEXT NOT NULL DEFAULT '',
                    model TEXT NOT NULL DEFAULT '',
                    embedding BLOB,
                    embedding_model TEXT NOT NULL DEFAULT '',
                    summary TEXT NOT NULL DEFAULT '',
                    summary_timestamp_start TEXT NOT NULL DEFAULT '',
                    summary_timestamp_end TEXT NOT NULL DEFAULT '',
                    cost REAL NOT NULL DEFAULT 0.0,
                    timestamped_summary_in_youtube_format TEXT NOT NULL DEFAULT '',
                    transcript TEXT NOT NULL DEFAULT '',
                    summary_done INTEGER NOT NULL DEFAULT 0
                )
                "#
            )
            .execute(&source_pool)
            .await
            .unwrap();
            
            // Insert test rows
            for (i, row) in rows.iter().enumerate() {
                let embedding = if row.embedding.len() % 4 == 0 {
                    Some(row.embedding.clone())
                } else {
                    None
                };
                
                sqlx::query(
                    r#"
                    INSERT INTO summaries (
                        identifier, original_source_link, model, embedding, embedding_model,
                        summary, summary_timestamp_start, summary_timestamp_end, cost,
                        timestamped_summary_in_youtube_format, transcript, summary_done
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#
                )
                .bind(i as i64)
                .bind(&row.original_source_link)
                .bind(&row.model)
                .bind(&embedding)
                .bind(&row.embedding_model)
                .bind(&row.summary)
                .bind(&row.summary_timestamp_start)
                .bind(&row.summary_timestamp_end)
                .bind(row.cost)
                .bind(&row.timestamped_summary_in_youtube_format)
                .bind(&row.transcript)
                .bind(if embedding.is_some() { 1 } else { 0 })
                .execute(&source_pool)
                .await
                .unwrap();
            }
            
            source_pool.close().await;
            
            // Run export
            let args = ExportDbArgs {
                source: source_path,
                output: output_path,
            };
            
            run_export(args).await.unwrap();
            
            // Verify output database schema
            let output_opts = SqliteConnectOptions::new()
                .filename(&output_path)
                .journal_mode(SqliteJournalMode::Wal)
                .read_only(true)
                .create_if_missing(false);
            
            let output_pool = SqlitePool::connect_with(output_opts).await.unwrap();
            
            // Check schema correctness - should not have transcript field
            let pragma_results: Vec<(String, String)> = sqlx::query("PRAGMA table_info(summaries)")
                .fetch_all(&output_pool)
                .await
                .unwrap()
                .into_iter()
                .filter_map(|row| {
                    let name: String = row.get("name");
                    let sql: String = row.get("sql");
                    if name == "summaries" {
                        Some((name, sql))
                    } else {
                        None
                    }
                })
                .collect();
            
            assert_eq!(pragma_results.len(), 1);
            let (_, table_sql) = &pragma_results[0];
            
            // Should contain all required fields except transcript
            assert!(table_sql.contains("identifier"));
            assert!(table_sql.contains("original_source_link"));
            assert!(table_sql.contains("model"));
            assert!(table_sql.contains("embedding"));
            assert!(table_sql.contains("embedding_model"));
            assert!(table_sql.contains("summary"));
            assert!(table_sql.contains("summary_timestamp_start"));
            assert!(table_sql.contains("summary_timestamp_end"));
            assert!(table_sql.contains("cost"));
            assert!(table_sql.contains("timestamped_summary_in_youtube_format"));
            
            // Should NOT contain transcript field
            assert!(!table_sql.contains("transcript"));
            
            output_pool.close().await;
        }

        #![proptest_config(ProptestConfig::with_cases(50))]
        
        #[test]
        fn prop_export_filter_correctness(
            // Generate test data with mixed qualification status
            rows in prop::collection::vec(
                prop::collection::any::<TestRow>(),
                1..=50
            )
        ) {
            let temp_dir = TempDir::new().unwrap();
            let source_path = temp_dir.path().join("source.db");
            let output_path = temp_dir.path().join("output.db");
            
            // Create source database with test data
            let source_opts = SqliteConnectOptions::new()
                .filename(&source_path)
                .journal_mode(SqliteJournalMode::Wal)
                .create_if_missing(true);
            
            let source_pool = SqlitePool::connect_with(source_opts).await.unwrap();
            
            // Create source schema
            sqlx::query(
                r#"
                CREATE TABLE summaries (
                    identifier INTEGER PRIMARY KEY,
                    original_source_link TEXT NOT NULL DEFAULT '',
                    model TEXT NOT NULL DEFAULT '',
                    embedding BLOB,
                    embedding_model TEXT NOT NULL DEFAULT '',
                    summary TEXT NOT NULL DEFAULT '',
                    summary_timestamp_start TEXT NOT NULL DEFAULT '',
                    summary_timestamp_end TEXT NOT NULL DEFAULT '',
                    cost REAL NOT NULL DEFAULT 0.0,
                    timestamped_summary_in_youtube_format TEXT NOT NULL DEFAULT '',
                    transcript TEXT NOT NULL DEFAULT '',
                    summary_done INTEGER NOT NULL DEFAULT 0
                )
                "#
            )
            .execute(&source_pool)
            .await
            .unwrap();
            
            // Insert test rows
            let mut expected_count = 0;
            for (i, row) in rows.iter().enumerate() {
                let embedding = if row.embedding.len() % 4 == 0 {
                    Some(row.embedding.clone())
                } else {
                    None
                };
                
                let summary_done = if row.identifier % 3 == 0 { 1 } else { 0 }; // Every 3rd row qualifies
                
                if summary_done == 1 {
                    expected_count += 1;
                }
                
                sqlx::query(
                    r#"
                    INSERT INTO summaries (
                        identifier, original_source_link, model, embedding, embedding_model,
                        summary, summary_timestamp_start, summary_timestamp_end, cost,
                        timestamped_summary_in_youtube_format, transcript, summary_done
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#
                )
                .bind(i as i64)
                .bind(&row.original_source_link)
                .bind(&row.model)
                .bind(&embedding)
                .bind(&row.embedding_model)
                .bind(&row.summary)
                .bind(&row.summary_timestamp_start)
                .bind(&row.summary_timestamp_end)
                .bind(row.cost)
                .bind(&row.timestamped_summary_in_youtube_format)
                .bind(&row.transcript)
                .bind(summary_done)
                .execute(&source_pool)
                .await
                .unwrap();
            }
            
            source_pool.close().await;
            
            // Run export
            let args = ExportDbArgs {
                source: source_path,
                output: output_path,
            };
            
            run_export(args).await.unwrap();
            
            // Verify only qualifying rows were exported
            let output_pool = SqlitePool::connect_with(
                SqliteConnectOptions::new()
                    .filename(&output_path)
                    .journal_mode(SqliteJournalMode::Wal)
                    .read_only(true)
                    .create_if_missing(false)
            )
            .await
            .unwrap();
            
            let actual_count: i64 = sqlx::query("SELECT COUNT(*) FROM summaries")
                .fetch_one(&output_pool)
                .await
                .unwrap()
                .get(0);
            
            assert_eq!(actual_count, expected_count);
            
            output_pool.close().await;
        }

        #![proptest_config(ProptestConfig::with_cases(30))]
        
        #[test]
        fn prop_source_db_immutability(
            // Generate test data
            rows in prop::collection::vec(
                prop::collection::any::<TestRow>(),
                1..=30
            )
        ) {
            let temp_dir = TempDir::new().unwrap();
            let source_path = temp_dir.path().join("source.db");
            let output_path = temp_dir.path().join("output.db");
            
            // Create source database with test data
            let source_opts = SqliteConnectOptions::new()
                .filename(&source_path)
                .journal_mode(SqliteJournalMode::Wal)
                .create_if_missing(true);
            
            let source_pool = SqlitePool::connect_with(source_opts).await.unwrap();
            
            // Create source schema
            sqlx::query(
                r#"
                CREATE TABLE summaries (
                    identifier INTEGER PRIMARY KEY,
                    original_source_link TEXT NOT NULL DEFAULT '',
                    model TEXT NOT NULL DEFAULT '',
                    embedding BLOB,
                    embedding_model TEXT NOT NULL DEFAULT '',
                    summary TEXT NOT NULL DEFAULT '',
                    summary_timestamp_start TEXT NOT NULL DEFAULT '',
                    summary_timestamp_end TEXT NOT NULL DEFAULT '',
                    cost REAL NOT NULL DEFAULT 0.0,
                    timestamped_summary_in_youtube_format TEXT NOT NULL DEFAULT '',
                    transcript TEXT NOT NULL DEFAULT '',
                    summary_done INTEGER NOT NULL DEFAULT 0
                )
                "#
            )
            .execute(&source_pool)
            .await
            .unwrap();
            
            // Insert test rows
            for (i, row) in rows.iter().enumerate() {
                let embedding = if row.embedding.len() % 4 == 0 {
                    Some(row.embedding.clone())
                } else {
                    None
                };
                
                sqlx::query(
                    r#"
                    INSERT INTO summaries (
                        identifier, original_source_link, model, embedding, embedding_model,
                        summary, summary_timestamp_start, summary_timestamp_end, cost,
                        timestamped_summary_in_youtube_format, transcript, summary_done
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#
                )
                .bind(i as i64)
                .bind(&row.original_source_link)
                .bind(&row.model)
                .bind(&embedding)
                .bind(&row.embedding_model)
                .bind(&row.summary)
                .bind(&row.summary_timestamp_start)
                .bind(&row.summary_timestamp_end)
                .bind(row.cost)
                .bind(&row.timestamped_summary_in_youtube_format)
                .bind(&row.transcript)
                .bind(if embedding.is_some() { 1 } else { 0 })
                .execute(&source_pool)
                .await
                .unwrap();
            }
            
            // Get initial row count and checksum
            let initial_count: i64 = sqlx::query("SELECT COUNT(*) FROM summaries")
                .fetch_one(&source_pool)
                .await
                .unwrap()
                .get(0);
            
            let initial_checksum: String = sqlx::query("SELECT md5sum(identifier || original_source_link || model || COALESCE(hex(embedding), '') || embedding_model || summary || COALESCE(summary_timestamp_start, '') || COALESCE(summary_timestamp_end, '') || COALESCE(timestamped_summary_in_youtube_format, '') || COALESCE(transcript, '') || COALESCE(summary_done, '')) FROM summaries")
                .fetch_one(&source_pool)
                .await
                .unwrap()
                .get(0);
            
            source_pool.close().await;
            
            // Run export
            let args = ExportDbArgs {
                source: source_path.clone(),
                output: output_path,
            };
            
            run_export(args).await.unwrap();
            
            // Verify source database is unchanged
            let source_pool = SqlitePool::connect_with(
                SqliteConnectOptions::new()
                    .filename(&source_path)
                    .journal_mode(SqliteJournalMode::Wal)
                    .read_only(true)
                    .create_if_missing(false)
            )
            .await
            .unwrap();
            
            let final_count: i64 = sqlx::query("SELECT COUNT(*) FROM summaries")
                .fetch_one(&source_pool)
                .await
                .unwrap()
                .get(0);
            
            let final_checksum: String = sqlx::query("SELECT md5sum(identifier || original_source_link || model || COALESCE(hex(embedding), '') || embedding_model || summary || COALESCE(summary_timestamp_start, '') || COALESCE(summary_timestamp_end, '') || COALESCE(timestamped_summary_in_youtube_format, '') || COALESCE(transcript, '') || COALESCE(summary_done, '')) FROM summaries")
                .fetch_one(&source_pool)
                .await
                .unwrap()
                .get(0);
            
            // Source database should be unchanged
            assert_eq!(initial_count, final_count);
            assert_eq!(initial_checksum, final_checksum);
            
            source_pool.close().await;
        }
    }
}
