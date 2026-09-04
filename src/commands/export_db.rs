use crate::errors::ExportError;
use anyhow::Result;
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
};
use std::path::PathBuf;

pub struct ExportDbArgs {
    pub source: PathBuf,
    pub output: PathBuf,
    pub include_embeddings: bool,
    pub compress: bool,
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
    if args.compress {
        let mut zstd_output = args.output.clone();
        if let Some(ext) = zstd_output.extension() {
            let new_ext = format!("{}.zst", ext.to_string_lossy());
            zstd_output.set_extension(new_ext);
        } else {
            zstd_output.set_extension("zst");
        }
        if zstd_output.exists() {
            return Err(ExportError::OutputExists(zstd_output).into());
        }
    }

    // 3. Validate output directory exists
    if let Some(parent) = args.output.parent()
        && !parent.exists()
    {
        return Err(ExportError::OutputDirMissing(parent.to_path_buf()).into());
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
        "#,
    )
    .execute(&output_pool)
    .await?;

    // 7. Copy rows with WHERE summary_done = 1
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
        WHERE summary_done = 1
        "#,
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
        let embedding = if args.include_embeddings {
            row.get::<Option<Vec<u8>>, _>("embedding")
        } else {
            None
        };
        let embedding_model = if args.include_embeddings {
            row.get::<String, _>("embedding_model")
        } else {
            String::new()
        };

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
            "#,
        )
        .bind(row.get::<i64, _>("identifier"))
        .bind(row.get::<String, _>("original_source_link"))
        .bind(row.get::<String, _>("model"))
        .bind(embedding)
        .bind(embedding_model)
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
    println!(
        "Exported {} rows to {}",
        exported_count,
        args.output.display()
    );
    println!("Output file size: {} bytes", file_size);

    // Close pools
    source_pool.close().await;
    output_pool.close().await;

    // 12. If compress option is enabled, run zstd to compress the database
    if args.compress {
        println!("Compressing output file with zstd...");
        let status = std::process::Command::new("zstd")
            .arg("--rm")
            .arg("-f")
            .arg(&args.output)
            .status()?;
        if !status.success() {
            return Err(anyhow::anyhow!(
                "zstd compression failed with status: {:?}",
                status
            ));
        }
        let mut zstd_output = args.output.clone();
        if let Some(ext) = zstd_output.extension() {
            let new_ext = format!("{}.zst", ext.to_string_lossy());
            zstd_output.set_extension(new_ext);
        } else {
            zstd_output.set_extension("zst");
        }
        println!(
            "Successfully compressed database to {}",
            zstd_output.display()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{SqlitePool, sqlite::SqliteJournalMode};
    use std::fs;
    use tempfile::TempDir;

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
            include_embeddings: false,
            compress: false,
        };

        let result = std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(run_export(args))
        })
        .join()
        .unwrap();

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
            include_embeddings: false,
            compress: false,
        };

        let result = std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(run_export(args))
        })
        .join()
        .unwrap();

        assert!(result.is_err());
        let err = result.unwrap_err();
        let export_err = err.downcast_ref::<ExportError>();
        assert!(export_err.is_some());
        assert!(matches!(
            export_err.unwrap(),
            ExportError::SourceNotFound(_)
        ));
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
            include_embeddings: false,
            compress: false,
        };

        let result = std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(run_export(args))
        })
        .join()
        .unwrap();

        assert!(result.is_err());
        let err = result.unwrap_err();
        let export_err = err.downcast_ref::<ExportError>();
        assert!(export_err.is_some());
        assert!(matches!(
            export_err.unwrap(),
            ExportError::OutputDirMissing(_)
        ));
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
            "#,
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
            "#,
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

        // Close source pool connection so the exporter can open it
        source_pool.close().await;

        let args = ExportDbArgs {
            source: source_path,
            output: output_path.clone(),
            include_embeddings: true,
            compress: false,
        };

        run_export(args).await?;

        assert!(output_path.exists());

        // Open output db and verify row
        let output_opts = SqliteConnectOptions::new()
            .filename(&output_path)
            .read_only(true);
        let output_pool = SqlitePool::connect_with(output_opts).await?;
        let count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM summaries")
            .fetch_one(&output_pool)
            .await?;
        assert_eq!(count, 1);
        output_pool.close().await;

        Ok(())
    }
}
