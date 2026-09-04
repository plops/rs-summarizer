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

use crate::generation::{GenerationStatus, PublicErrorCode};
use crate::models::{RatingStats, SubmitForm, Summary};

/// Insert a new summary row and return the new identifier.
pub async fn insert_new_summary(
    db: &SqlitePool,
    form: &SubmitForm,
    host: &str,
    timestamp_start: &str,
) -> Result<i64, sqlx::Error> {
    let transcript = form.transcript.as_deref().unwrap_or("");
    let lang = if form.output_language.is_empty() {
        "en"
    } else {
        &form.output_language
    };

    let result = sqlx::query(
        "INSERT INTO summaries (model, original_source_link, transcript, host, summary_timestamp_start, google_search_grounding, url_context, include_glossary, output_language, thinking_level) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&form.model)
    .bind(&form.original_source_link)
    .bind(transcript)
    .bind(host)
    .bind(timestamp_start)
    .bind(form.google_search_grounding)
    .bind(form.url_context)
    .bind(form.include_glossary)
    .bind(lang)
    .bind(form.thinking_level)
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

/// Append only if the caller still owns this generation epoch. This prevents a
/// late stream from a cancelled attempt corrupting a retry's body.
pub async fn append_summary_chunk_for_epoch(
    db: &SqlitePool,
    identifier: i64,
    epoch: i64,
    chunk: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query("UPDATE summaries SET summary = summary || ?, generation_updated_at = ? WHERE identifier = ? AND generation_epoch = ? AND generation_status = 'running'")
        .bind(chunk).bind(chrono::Utc::now().to_rfc3339()).bind(identifier).bind(epoch).execute(db).await?.rows_affected() == 1)
}

/// Atomically transition a row, optionally beginning a fresh display epoch.
pub async fn transition_generation(
    db: &SqlitePool,
    identifier: i64,
    expected: GenerationStatus,
    next: GenerationStatus,
    error: Option<(PublicErrorCode, &str)>,
    next_retry_at: Option<&str>,
    fresh_epoch: bool,
) -> Result<bool, sqlx::Error> {
    debug_assert!(expected.can_transition_to(next));
    let now = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query("UPDATE summaries SET generation_status = ?, generation_updated_at = ?, generation_started_at = CASE WHEN ? THEN ? ELSE generation_started_at END, generation_epoch = generation_epoch + CASE WHEN ? THEN 1 ELSE 0 END, generation_attempt = generation_attempt + CASE WHEN ? THEN 1 ELSE 0 END, summary = CASE WHEN ? THEN '' ELSE summary END, summary_done = CASE WHEN ? IN ('succeeded','failed','partial_failed') THEN 1 ELSE 0 END, generation_error_code = COALESCE(?, ''), generation_error_message = COALESCE(?, ''), next_retry_at = COALESCE(?, '') WHERE identifier = ? AND generation_status = ?")
        .bind(next.as_str()).bind(&now).bind(fresh_epoch).bind(&now).bind(fresh_epoch).bind(fresh_epoch).bind(fresh_epoch).bind(next.as_str()).bind(error.map(|e| e.0.as_str())).bind(error.map(|e| e.1)).bind(next_retry_at).bind(identifier).bind(expected.as_str()).execute(db).await?;
    Ok(result.rows_affected() == 1)
}

pub async fn retry_generation(db: &SqlitePool, identifier: i64) -> Result<bool, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    Ok(sqlx::query("UPDATE summaries SET generation_status='queued', generation_epoch=generation_epoch+1, generation_attempt=generation_attempt+1, generation_updated_at=?, generation_started_at='', next_retry_at='', generation_error_code='', generation_error_message='', summary='', summary_done=0 WHERE identifier=? AND generation_status IN ('failed','partial_failed')")
        .bind(now).bind(identifier).execute(db).await?.rows_affected() == 1)
}

pub async fn recover_stale_generations(
    db: &SqlitePool,
    stale_before: &str,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("UPDATE summaries SET generation_status='queued', generation_updated_at=?, generation_error_code='network_interrupted', generation_error_message='A previous generation was interrupted and will be retried.' WHERE generation_status='running' AND generation_updated_at < ?")
        .bind(chrono::Utc::now().to_rfc3339()).bind(stale_before).execute(db).await?.rows_affected())
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
#[allow(clippy::too_many_arguments)]
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
        "UPDATE summaries SET summary_done = 1, generation_status = 'succeeded', generation_error_code = '', generation_error_message = '', summary_input_tokens = ?, summary_output_tokens = ?, \
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
    if let Some(r) = summary_rating
        && !(1..=5).contains(&r)
    {
        return Err(sqlx::Error::Protocol(
            "summary_rating must be between 1 and 5".into(),
        ));
    }
    if let Some(r) = content_rating
        && !(1..=5).contains(&r)
    {
        return Err(sqlx::Error::Protocol(
            "content_rating must be between 1 and 5".into(),
        ));
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
            include_glossary: false,
            output_language: "en".to_string(),
            thinking_level: Default::default(),
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

    #[tokio::test]
    async fn thinking_level_defaults_is_constrained_and_round_trips() {
        let pool = create_in_memory_db().await;
        let default_identifier = sqlx::query("INSERT INTO summaries (model) VALUES (?)")
            .bind("gemini-3.8-flash")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();
        let default: String =
            sqlx::query_scalar("SELECT thinking_level FROM summaries WHERE identifier = ?")
                .bind(default_identifier)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(default, "high");

        let invalid = sqlx::query("INSERT INTO summaries (model, thinking_level) VALUES (?, ?)")
            .bind("gemini-3.8-flash")
            .bind("maximum")
            .execute(&pool)
            .await;
        assert!(invalid.is_err());

        for preference in crate::models::ThinkingPreference::ALL {
            let form = SubmitForm {
                original_source_link: format!("https://example.com/{preference}"),
                transcript: None,
                model: "gemini-3.8-flash".to_string(),
                google_search_grounding: false,
                url_context: false,
                include_glossary: false,
                output_language: "en".to_string(),
                thinking_level: preference,
            };
            let identifier = insert_new_summary(&pool, &form, "test", "2026-01-01T00:00:00Z")
                .await
                .unwrap();
            assert_eq!(
                fetch_summary(&pool, identifier)
                    .await
                    .unwrap()
                    .unwrap()
                    .thinking_level,
                preference
            );
        }
    }

    #[tokio::test]
    async fn thinking_level_migration_upgrades_a_legacy_schema() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE summaries (identifier INTEGER PRIMARY KEY, model TEXT NOT NULL DEFAULT '')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO summaries (model) VALUES ('gemini-3.7-flash')")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::raw_sql(include_str!("../migrations/006_add_thinking_level.sql"))
            .execute(&pool)
            .await
            .unwrap();

        let level: String = sqlx::query_scalar("SELECT thinking_level FROM summaries")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(level, "high");
    }

    #[tokio::test]
    async fn generation_lifecycle_backfills_and_epoch_guards_chunks() {
        let pool = create_in_memory_db().await;
        let queued_id = sqlx::query("INSERT INTO summaries (model) VALUES ('m')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();
        assert_eq!(
            fetch_summary(&pool, queued_id)
                .await
                .unwrap()
                .unwrap()
                .generation_status,
            "queued"
        );
        assert!(
            transition_generation(
                &pool,
                queued_id,
                GenerationStatus::Queued,
                GenerationStatus::Running,
                None,
                None,
                true
            )
            .await
            .unwrap()
        );
        let row = fetch_summary(&pool, queued_id).await.unwrap().unwrap();
        assert!(
            append_summary_chunk_for_epoch(&pool, queued_id, row.generation_epoch, "first")
                .await
                .unwrap()
        );
        assert!(
            !append_summary_chunk_for_epoch(&pool, queued_id, row.generation_epoch + 1, "late")
                .await
                .unwrap()
        );
        assert!(
            transition_generation(
                &pool,
                queued_id,
                GenerationStatus::Running,
                GenerationStatus::PartialFailed,
                Some((PublicErrorCode::ProviderAborted, "stopped")),
                None,
                false
            )
            .await
            .unwrap()
        );
        assert!(retry_generation(&pool, queued_id).await.unwrap());
        let retried = fetch_summary(&pool, queued_id).await.unwrap().unwrap();
        assert_eq!(retried.generation_status, "queued");
        assert!(retried.summary.is_empty());

        // Apply the additive migration to an actual pre-007 fixture.
        let legacy = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql(include_str!("../migrations/001_initial.sql"))
            .execute(&legacy)
            .await
            .unwrap();
        sqlx::query("INSERT INTO summaries (model, summary_done) VALUES ('old', 1)")
            .execute(&legacy)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../migrations/007_add_generation_lifecycle.sql"
        ))
        .execute(&legacy)
        .await
        .unwrap();
        let backfilled: String = sqlx::query_scalar("SELECT generation_status FROM summaries")
            .fetch_one(&legacy)
            .await
            .unwrap();
        assert_eq!(backfilled, "succeeded");
    }
}
