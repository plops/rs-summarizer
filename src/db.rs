use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

/// Initialize the SQLite connection pool with WAL mode and run migrations.
pub async fn init_db(database_url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

use crate::models::{Summary, SubmitForm};

/// Insert a new summary row and return the new identifier.
pub async fn insert_new_summary(
    db: &SqlitePool,
    form: &SubmitForm,
    host: &str,
    timestamp_start: &str,
) -> Result<i64, sqlx::Error> {
    let transcript = form.transcript.as_deref().unwrap_or("");

    let result = sqlx::query(
        "INSERT INTO summaries (model, original_source_link, transcript, host, summary_timestamp_start) \
         VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&form.model)
    .bind(&form.original_source_link)
    .bind(transcript)
    .bind(host)
    .bind(timestamp_start)
    .execute(db)
    .await?;

    Ok(result.last_insert_rowid())
}

/// Fetch a summary by its identifier.
pub async fn fetch_summary(db: &SqlitePool, identifier: i64) -> Result<Option<Summary>, sqlx::Error> {
    let row = sqlx::query_as::<_, Summary>(
        "SELECT * FROM summaries WHERE identifier = ?"
    )
    .bind(identifier)
    .fetch_optional(db)
    .await?;

    Ok(row)
}

/// Update the transcript field for a summary.
pub async fn update_transcript(db: &SqlitePool, identifier: i64, transcript: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE summaries SET transcript = ? WHERE identifier = ?")
        .bind(transcript)
        .bind(identifier)
        .execute(db)
        .await?;

    Ok(())
}

/// Append a chunk to the summary field (for streaming).
pub async fn update_summary_chunk(db: &SqlitePool, identifier: i64, chunk: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE summaries SET summary = summary || ? WHERE identifier = ?")
        .bind(chunk)
        .bind(identifier)
        .execute(db)
        .await?;

    Ok(())
}

/// Mark summary as done with token counts, cost, and end timestamp.
pub async fn mark_summary_done(
    db: &SqlitePool,
    identifier: i64,
    input_tokens: i64,
    output_tokens: i64,
    cost: f64,
    timestamp_end: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE summaries SET summary_done = 1, summary_input_tokens = ?, summary_output_tokens = ?, \
         cost = ?, summary_timestamp_end = ? WHERE identifier = ?"
    )
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(cost)
    .bind(timestamp_end)
    .bind(identifier)
    .execute(db)
    .await?;

    Ok(())
}

/// Mark timestamps as done and store the YouTube-formatted summary.
pub async fn mark_timestamps_done(
    db: &SqlitePool,
    identifier: i64,
    youtube_format: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE summaries SET timestamps_done = 1, timestamped_summary_in_youtube_format = ? \
         WHERE identifier = ?"
    )
    .bind(youtube_format)
    .bind(identifier)
    .execute(db)
    .await?;

    Ok(())
}

/// Store embedding blob for a summary.
pub async fn store_embedding(
    db: &SqlitePool,
    identifier: i64,
    embedding: &[u8],
    embedding_model: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE summaries SET embedding = ?, embedding_model = ? WHERE identifier = ?")
        .bind(embedding)
        .bind(embedding_model)
        .bind(identifier)
        .execute(db)
        .await?;

    Ok(())
}

/// Fetch all embeddings (identifier + blob) for similarity search.
pub async fn fetch_all_embeddings(db: &SqlitePool) -> Result<Vec<(i64, Vec<u8>)>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (i64, Vec<u8>)>(
        "SELECT identifier, embedding FROM summaries WHERE embedding IS NOT NULL"
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

/// Fetch a page of summaries for browsing (20 per page, ordered by id DESC).
pub async fn fetch_browse_page(db: &SqlitePool, page: u32) -> Result<Vec<Summary>, sqlx::Error> {
    let offset = page * 20;

    let rows = sqlx::query_as::<_, Summary>(
        "SELECT * FROM summaries ORDER BY identifier DESC LIMIT 20 OFFSET ?"
    )
    .bind(offset)
    .fetch_all(db)
    .await?;

    Ok(rows)
}
