//! Migration 011 verification: applies the real
//! `migrations/011_update_thinking_and_rpd_usage.sql` text to a scratch
//! database shaped like legacy production data (nullable columns with
//! NULLs, plus `summary_ratings` rows under `ON DELETE CASCADE`) and
//! asserts the rebuild preserves every row, keeps ratings alive, widens
//! the thinking-level CHECK, and creates the new counters table.

use sqlx::SqlitePool;

/// Split migration SQL into executable statements. Full-line `--`
/// comments are dropped BEFORE splitting so semicolons inside comments
/// cannot fracture a statement.
fn split_statements(sql: &str) -> Vec<String> {
    let uncommented = sql
        .lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n");
    uncommented
        .split(';')
        .map(|chunk| chunk.trim().to_string())
        .filter(|stmt| !stmt.is_empty())
        .collect()
}

/// Legacy-shaped `summaries`: same columns as production, but without
/// `NOT NULL` constraints so historical NULLs can be reproduced.
const LEGACY_SCHEMA: &str = "
CREATE TABLE summaries (
    identifier INTEGER PRIMARY KEY,
    model TEXT, transcript TEXT, host TEXT, original_source_link TEXT,
    include_comments INTEGER, include_timestamps INTEGER,
    include_glossary INTEGER, output_language TEXT, summary TEXT,
    summary_done INTEGER, summary_input_tokens INTEGER,
    summary_output_tokens INTEGER, summary_timestamp_start TEXT,
    summary_timestamp_end TEXT, timestamps TEXT, timestamps_done INTEGER,
    timestamps_input_tokens INTEGER, timestamps_output_tokens INTEGER,
    timestamps_timestamp_start TEXT, timestamps_timestamp_end TEXT,
    timestamped_summary_in_youtube_format TEXT, cost FLOAT,
    embedding BLOB, embedding_model TEXT, full_embedding BLOB,
    google_search_grounding BOOLEAN, url_context BOOLEAN,
    thinking TEXT, thinking_tokens INTEGER, thinking_level TEXT,
    generation_status TEXT, generation_attempt INTEGER,
    generation_epoch INTEGER, generation_started_at TEXT,
    generation_updated_at TEXT, next_retry_at TEXT,
    generation_error_code TEXT, generation_error_message TEXT,
    provider_interaction_id TEXT, rs_summarizer_version TEXT, title TEXT
);
CREATE TABLE summary_ratings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    summary_id INTEGER NOT NULL,
    client_ip TEXT NOT NULL,
    summary_rating INTEGER,
    content_rating INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY(summary_id) REFERENCES summaries(identifier) ON DELETE CASCADE
);
";

async fn legacy_pool(dir: &tempfile::TempDir) -> SqlitePool {
    let path = dir.path().join("legacy.db");
    let pool = SqlitePool::connect(&format!("sqlite://{}?mode=rwc", path.display()))
        .await
        .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    for stmt in split_statements(LEGACY_SCHEMA) {
        sqlx::raw_sql(sqlx::AssertSqlSafe(stmt))
            .execute(&mut *conn)
            .await
            .unwrap();
    }
    // One complete row plus one legacy row full of NULLs.
    sqlx::query(
        "INSERT INTO summaries (model, original_source_link, summary, \
         summary_done, thinking_level, generation_status, title, \
         rs_summarizer_version, cost) \
         VALUES ('gemini-3.5-flash-lite', 'https://youtu.be/abc', 'text', \
         1, 'high', 'succeeded', 'Some Title', '1.10.1', 0.001)",
    )
    .execute(&mut *conn)
    .await
    .unwrap();
    sqlx::query("INSERT INTO summaries DEFAULT VALUES")
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::query("INSERT INTO summary_ratings (summary_id, client_ip, summary_rating) VALUES (1, '1.2.3.4', 5)")
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::query("INSERT INTO summary_ratings (summary_id, client_ip, content_rating) VALUES (2, '5.6.7.8', 4)")
        .execute(&mut *conn)
        .await
        .unwrap();
    pool
}

async fn apply_migration_011(pool: &SqlitePool) {
    let sql = std::fs::read_to_string("migrations/011_update_thinking_and_rpd_usage.sql").unwrap();
    assert!(
        sql.starts_with("-- no-transaction"),
        "migration 011 must opt out of the implicit transaction"
    );
    let mut conn = pool.acquire().await.unwrap();
    for stmt in split_statements(&sql) {
        sqlx::raw_sql(sqlx::AssertSqlSafe(stmt.clone()))
            .execute(&mut *conn)
            .await
            .unwrap_or_else(|e| panic!("statement failed: {stmt}\nerror: {e}"));
    }
}

#[tokio::test]
async fn migration_011_preserves_rows_ratings_and_nulls() {
    let _dir = tempfile::tempdir().unwrap();
    let pool = legacy_pool(&_dir).await;
    apply_migration_011(&pool).await;

    // Row counts are unchanged (ratings survive: no CASCADE wipe).
    let summaries: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM summaries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(summaries, 2);
    let ratings: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM summary_ratings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ratings, 2);

    // The legacy NULL row is readable with COALESCE defaults.
    let row = rs_summarizer::db::fetch_summary(&pool, 2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.model, "");
    assert_eq!(row.title, "");
    assert_eq!(
        row.thinking_level,
        rs_summarizer::models::ThinkingPreference::High
    );
    assert_eq!(row.cost, 0.0);

    // The complete row is byte-identical through the rebuild.
    let full = rs_summarizer::db::fetch_summary(&pool, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(full.title, "Some Title");
    assert_eq!(full.rs_summarizer_version, "1.10.1");

    // New CHECK accepts xhigh and still rejects garbage.
    sqlx::query("INSERT INTO summaries (thinking_level) VALUES ('xhigh')")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        sqlx::query("INSERT INTO summaries (thinking_level) VALUES ('bogus')")
            .execute(&pool)
            .await
            .is_err()
    );

    // Indexes and the RPD table exist.
    for index in [
        "idx_dedup_lookup",
        "idx_summaries_model_time",
        "idx_summaries_lang_glossary",
        "idx_summaries_generation_recovery",
        "idx_summaries_title_url",
    ] {
        let found: Option<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'index' AND name = ?")
                .bind(index)
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert_eq!(found.as_deref(), Some(index));
    }
    rs_summarizer::db::record_model_daily_usage(&pool, "2026-11-15", "gemini-3.5-flash-lite").await;
    let counts = rs_summarizer::db::load_model_daily_usage(&pool, "2026-11-15").await;
    assert_eq!(counts.get("gemini-3.5-flash-lite"), Some(&1));

    // Structural health.
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(integrity, "ok");
    let fk_violations: Vec<(String, i64, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(fk_violations.is_empty());
}
