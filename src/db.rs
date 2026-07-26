use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
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

use crate::models::{RatingStats, SubmitForm, Summary};

/// Insert a new summary row and return the new identifier.
pub async fn insert_new_summary(
    db: &SqlitePool,
    form: &SubmitForm,
    host: &str,
    timestamp_start: &str,
) -> Result<i64, sqlx::Error> {
    let transcript = form.transcript.as_deref().unwrap_or("");

    let result = sqlx::query(
        "INSERT INTO summaries (model, original_source_link, transcript, host, summary_timestamp_start, google_search_grounding, url_context) \
         VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&form.model)
    .bind(&form.original_source_link)
    .bind(transcript)
    .bind(host)
    .bind(timestamp_start)
    .bind(form.google_search_grounding)
    .bind(form.url_context)
    .execute(db)
    .await?;

    Ok(result.last_insert_rowid())
}

/// Fetch a summary by its identifier.
pub async fn fetch_summary(
    db: &SqlitePool,
    identifier: i64,
) -> Result<Option<Summary>, sqlx::Error> {
    let row = sqlx::query_as::<_, Summary>("SELECT * FROM summaries WHERE identifier = ?")
        .bind(identifier)
        .fetch_optional(db)
        .await?;

    Ok(row)
}

/// Update the transcript field for a summary.
pub async fn update_transcript(
    db: &SqlitePool,
    identifier: i64,
    transcript: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE summaries SET transcript = ? WHERE identifier = ?")
        .bind(transcript)
        .bind(identifier)
        .execute(db)
        .await?;

    Ok(())
}

/// Append a chunk to the summary field (for streaming).
pub async fn update_summary_chunk(
    db: &SqlitePool,
    identifier: i64,
    chunk: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE summaries SET summary = summary || ? WHERE identifier = ?")
        .bind(chunk)
        .bind(identifier)
        .execute(db)
        .await?;

    Ok(())
}

/// Overwrite the summary field completely (e.g. for errors).
pub async fn update_summary_full(
    db: &SqlitePool,
    identifier: i64,
    summary: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE summaries SET summary = ? WHERE identifier = ?")
        .bind(summary)
        .bind(identifier)
        .execute(db)
        .await?;

    Ok(())
}

/// Update the model field for a summary.
pub async fn update_model(
    db: &SqlitePool,
    identifier: i64,
    model: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE summaries SET model = ? WHERE identifier = ?")
        .bind(model)
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
    thinking_tokens: i64,
    thinking: &str,
    cost: f64,
    timestamp_end: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE summaries SET summary_done = 1, summary_input_tokens = ?, summary_output_tokens = ?, \
         thinking_tokens = ?, thinking = ?, cost = ?, summary_timestamp_end = ? WHERE identifier = ?"
    )
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(thinking_tokens)
    .bind(thinking)
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
         WHERE identifier = ?",
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
        "SELECT identifier, embedding FROM summaries WHERE embedding IS NOT NULL",
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

/// Fetch a page of summaries for browsing (ordered by id DESC).
pub async fn fetch_browse_page(
    db: &SqlitePool,
    page: u32,
    page_size: u32,
) -> Result<Vec<Summary>, sqlx::Error> {
    let offset = page * page_size;

    let rows = sqlx::query_as::<_, Summary>(
        "SELECT * FROM summaries ORDER BY identifier DESC LIMIT ? OFFSET ?",
    )
    .bind(page_size)
    .bind(offset)
    .fetch_all(db)
    .await?;

    Ok(rows)
}

/// Upsert a summary rating from a client IP address.
pub async fn upsert_rating(
    db: &SqlitePool,
    summary_id: i64,
    client_ip: &str,
    summary_rating: Option<i32>,
    content_rating: Option<i32>,
) -> Result<(), sqlx::Error> {
    if let Some(r) = summary_rating {
        if !(1..=5).contains(&r) {
            return Err(sqlx::Error::Protocol(
                "summary_rating must be between 1 and 5".into(),
            ));
        }
    }
    if let Some(r) = content_rating {
        if !(1..=5).contains(&r) {
            return Err(sqlx::Error::Protocol(
                "content_rating must be between 1 and 5".into(),
            ));
        }
    }

    sqlx::query(
        "INSERT INTO summary_ratings (summary_id, client_ip, summary_rating, content_rating, created_at, updated_at) \
         VALUES (?, ?, ?, ?, datetime('now'), datetime('now')) \
         ON CONFLICT(summary_id, client_ip) DO UPDATE SET \
         summary_rating = COALESCE(excluded.summary_rating, summary_ratings.summary_rating), \
         content_rating = COALESCE(excluded.content_rating, summary_ratings.content_rating), \
         updated_at = datetime('now')"
    )
    .bind(summary_id)
    .bind(client_ip)
    .bind(summary_rating)
    .bind(content_rating)
    .execute(db)
    .await?;

    Ok(())
}

/// Fetch rating statistics (averages, counts, and user's rating) for a summary.
pub async fn fetch_rating_stats(
    db: &SqlitePool,
    summary_id: i64,
    client_ip: Option<&str>,
) -> Result<RatingStats, sqlx::Error> {
    let row = sqlx::query(
        "SELECT \
         COALESCE(AVG(summary_rating), 0.0) AS avg_summary_rating, \
         COUNT(summary_rating) AS count_summary_rating, \
         COALESCE(AVG(content_rating), 0.0) AS avg_content_rating, \
         COUNT(content_rating) AS count_content_rating \
         FROM summary_ratings WHERE summary_id = ?",
    )
    .bind(summary_id)
    .fetch_one(db)
    .await?;

    let avg_summary_rating: f64 = row.get("avg_summary_rating");
    let count_summary_rating: i64 = row.get("count_summary_rating");
    let avg_content_rating: f64 = row.get("avg_content_rating");
    let count_content_rating: i64 = row.get("count_content_rating");

    let (user_summary_rating, user_content_rating) = if let Some(ip) = client_ip {
        if let Some(user_row) = sqlx::query(
            "SELECT summary_rating, content_rating FROM summary_ratings WHERE summary_id = ? AND client_ip = ?"
        )
        .bind(summary_id)
        .bind(ip)
        .fetch_optional(db)
        .await? {
            (user_row.get("summary_rating"), user_row.get("content_rating"))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    Ok(RatingStats {
        avg_summary_rating,
        count_summary_rating,
        avg_content_rating,
        count_content_rating,
        user_summary_rating,
        user_content_rating,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_in_memory_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_rating_upsert_and_stats() {
        let pool = create_in_memory_db().await;

        let form = SubmitForm {
            original_source_link: "https://youtube.com/watch?v=test".to_string(),
            transcript: Some("test transcript".to_string()),
            model: "gemini-3.6-flash".to_string(),
            google_search_grounding: false,
            url_context: false,
        };
        let summary_id = insert_new_summary(&pool, &form, "127.0.0.1", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        // Initial stats should be empty
        let initial_stats = fetch_rating_stats(&pool, summary_id, Some("192.168.1.1"))
            .await
            .unwrap();
        assert_eq!(initial_stats.count_summary_rating, 0);
        assert_eq!(initial_stats.count_content_rating, 0);
        assert_eq!(initial_stats.user_summary_rating, None);

        // Add first rating from IP 1
        upsert_rating(&pool, summary_id, "192.168.1.1", Some(5), Some(4))
            .await
            .unwrap();

        let stats_ip1 = fetch_rating_stats(&pool, summary_id, Some("192.168.1.1"))
            .await
            .unwrap();
        assert_eq!(stats_ip1.count_summary_rating, 1);
        assert_eq!(stats_ip1.count_content_rating, 1);
        assert!((stats_ip1.avg_summary_rating - 5.0).abs() < 1e-6);
        assert!((stats_ip1.avg_content_rating - 4.0).abs() < 1e-6);
        assert_eq!(stats_ip1.user_summary_rating, Some(5));
        assert_eq!(stats_ip1.user_content_rating, Some(4));

        // Add rating from IP 2
        upsert_rating(&pool, summary_id, "192.168.1.2", Some(3), Some(2))
            .await
            .unwrap();

        let stats_ip2 = fetch_rating_stats(&pool, summary_id, Some("192.168.1.2"))
            .await
            .unwrap();
        assert_eq!(stats_ip2.count_summary_rating, 2);
        assert_eq!(stats_ip2.count_content_rating, 2);
        assert!((stats_ip2.avg_summary_rating - 4.0).abs() < 1e-6); // (5+3)/2 = 4.0
        assert!((stats_ip2.avg_content_rating - 3.0).abs() < 1e-6); // (4+2)/2 = 3.0
        assert_eq!(stats_ip2.user_summary_rating, Some(3));

        // Upsert IP 1 to change rating
        upsert_rating(&pool, summary_id, "192.168.1.1", Some(1), None)
            .await
            .unwrap();

        let stats_ip1_updated = fetch_rating_stats(&pool, summary_id, Some("192.168.1.1"))
            .await
            .unwrap();
        assert_eq!(stats_ip1_updated.count_summary_rating, 2);
        assert_eq!(stats_ip1_updated.user_summary_rating, Some(1));
        // Content rating for IP 1 remains 4 due to COALESCE
        assert_eq!(stats_ip1_updated.user_content_rating, Some(4));
    }

    #[tokio::test]
    async fn test_rating_range_validation() {
        let pool = create_in_memory_db().await;

        let err0 = upsert_rating(&pool, 1, "127.0.0.1", Some(0), Some(3)).await;
        assert!(err0.is_err());

        let err6 = upsert_rating(&pool, 1, "127.0.0.1", Some(4), Some(6)).await;
        assert!(err6.is_err());
    }
}
