use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use rs_summarizer::{build_router, state::AppState};
use sqlx::SqlitePool;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use tower::ServiceExt;

fn test_state(pool: SqlitePool) -> AppState {
    AppState {
        app_version: rs_summarizer::APP_VERSION,
        db: pool.clone(),
        model_options: Arc::new(rs_summarizer::state::get_default_models()),
        model_counts: Arc::new(RwLock::new(HashMap::new())),
        last_reset_day: Arc::new(RwLock::new(None)),
        gemini_api_key: "test_key".to_string(),
        #[cfg(feature = "nn-mapper")]
        nn_mapper: None,
        viz_data: None,
        model_locks: Arc::new(RwLock::new(HashMap::new())),
        dedup_service: rs_summarizer::services::deduplication::DeduplicationService::new(
            std::time::Duration::from_secs(300),
        ),
        download_limiter: Arc::new(
            rs_summarizer::services::download_limiter::DownloadLimiter::from_env(),
        ),
    }
}

fn with_connect_info(req: Request<Body>) -> Request<Body> {
    let mut req = req;
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(SocketAddr::from((
            [127, 0, 0, 1],
            8080,
        ))));
    req
}

async fn body_text(response: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

async fn migrated_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

async fn insert_succeeded_summary(pool: &SqlitePool) -> i64 {
    sqlx::query(
        "INSERT INTO summaries (model, original_source_link, summary, summary_done, \
            generation_status, cost, timestamps_done, timestamped_summary_in_youtube_format, \
            summary_timestamp_start, rs_summarizer_version) \
         VALUES (?, ?, ?, 1, 'succeeded', ?, 1, ?, ?, ?)",
    )
    .bind("gemini-3.6-flash")
    .bind("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
    .bind("**1:23 Intro**\n\nBody text about the video.")
    .bind(0.038_797_5_f64)
    .bind("*Intro* 1:23\n\nBody text about the video.")
    .bind("2026-10-06T05:00:00Z")
    .bind("1.8.0")
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

#[tokio::test]
async fn browse_renders_summary_once_with_copy_button() {
    let pool = migrated_pool().await;
    insert_succeeded_summary(&pool).await;
    let app = build_router(test_state(pool));

    let req = with_connect_info(
        Request::builder()
            .method("GET")
            .uri("/browse")
            .body(Body::empty())
            .unwrap(),
    );
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;

    // Body text appears twice: once visibly in the summary and once in the
    // hidden clipboard payload. The visible duplicate footer is gone.
    assert_eq!(html.matches("Body text about the video.").count(), 2);
    assert!(!html.contains("<footer>"));
    // Timestamps inside the summary link to YouTube (1:23 -> 83s).
    assert!(
        html.contains("t=83s"),
        "summary timestamps must link: {html}"
    );
    // Copy button with hidden clipboard payload and feedback script.
    assert!(html.contains("Für YouTube kopieren"));
    assert!(html.contains("data-clipboard"));
    assert!(html.contains("Kopiert! ✓"));
    // Metadata: model and formatted cost, no six-decimal raw cost.
    assert!(html.contains("gemini-3.6-flash"));
    assert!(html.contains("$0.04"));
    assert!(!html.contains("0.038798"));
    // Source link and rating component are present.
    assert!(html.contains(">Source</a>"));
    assert!(html.contains("rating-container-1"));
    // Model and cost stay in the header; the clipboard payload carries them
    // as a footer at the bottom.
    let body_pos = html.find("Body text about the video.").unwrap();
    assert!(html.find("gemini-3.6-flash").unwrap() < body_pos);
    assert!(html.find("$0.04").unwrap() < body_pos);
    let clipboard_pos = html.find("data-clipboard").unwrap();
    assert!(clipboard_pos < html.rfind("gemini-3.6-flash").unwrap());
    assert!(clipboard_pos < html.rfind("(cost: $0.04)").unwrap());
    let version_line = format!("rocketrecap-dot-com v{}", rs_summarizer::APP_VERSION);
    assert!(clipboard_pos < html.rfind(&version_line).unwrap());
}

#[tokio::test]
async fn generation_partial_succeeded_has_source_rating_and_copy_button() {
    let pool = migrated_pool().await;
    let id = insert_succeeded_summary(&pool).await;
    let app = build_router(test_state(pool));

    let req = with_connect_info(
        Request::builder()
            .method("POST")
            .uri(format!("/generations/{id}"))
            .body(Body::empty())
            .unwrap(),
    );
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;

    assert!(html.contains("Summary Complete"));
    assert!(html.contains("gemini-3.6-flash"));
    assert!(html.contains("$0.04"));
    assert!(html.contains("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
    assert!(html.contains(">Source</a>"));
    assert!(html.contains(&format!("rating-container-{id}")));
    assert!(html.contains("Für YouTube kopieren"));
    assert!(html.contains("data-clipboard"));
    assert!(html.contains("t=83s"));
    assert!(!html.contains("<footer>"));
    assert!(!html.contains("hx-trigger=\"every 1s\""));
    // Model and cost stay at the top; the clipboard payload carries them as
    // a footer at the bottom.
    let body_pos = html.find("Body text about the video.").unwrap();
    assert!(html.find("gemini-3.6-flash").unwrap() < body_pos);
    assert!(html.find("$0.04").unwrap() < body_pos);
    let clipboard_pos = html.find("data-clipboard").unwrap();
    assert!(clipboard_pos < html.rfind("gemini-3.6-flash").unwrap());
    assert!(clipboard_pos < html.rfind("(cost: $0.04)").unwrap());
    let version_line = format!("rocketrecap-dot-com v{}", rs_summarizer::APP_VERSION);
    assert!(clipboard_pos < html.rfind(&version_line).unwrap());
}

#[tokio::test]
async fn generation_partial_retry_wait_shows_friendly_time() {
    let pool = migrated_pool().await;
    let retry_at = (chrono::Utc::now() + chrono::Duration::seconds(90)).to_rfc3339();
    let id = sqlx::query(
        "INSERT INTO summaries (model, original_source_link, summary, generation_status, \
            next_retry_at, summary_timestamp_start) \
         VALUES (?, ?, ?, 'retry_wait', ?, ?)",
    )
    .bind("gemini-3.6-flash")
    .bind("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
    .bind("partial draft")
    .bind(&retry_at)
    .bind("2026-10-06T05:00:00Z")
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let app = build_router(test_state(pool));

    let req = with_connect_info(
        Request::builder()
            .method("POST")
            .uri(format!("/generations/{id}"))
            .body(Body::empty())
            .unwrap(),
    );
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;

    assert!(
        html.contains("Wiederholung in ca."),
        "friendly retry notice missing: {html}"
    );
    assert!(
        !html.contains(&retry_at),
        "raw ISO timestamp must not leak: {html}"
    );
    assert!(html.contains("hx-trigger=\"every 1s\""));
}

#[tokio::test]
async fn generation_partial_failed_shows_actionable_error() {
    let pool = migrated_pool().await;
    let id = sqlx::query(
        "INSERT INTO summaries (model, original_source_link, generation_status, \
            generation_error_code, generation_error_message, summary_timestamp_start) \
         VALUES (?, ?, 'failed', 'internal', ?, ?)",
    )
    .bind("gemini-3.6-flash")
    .bind("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
    .bind("Für dieses Video sind keine Untertitel verfügbar. Du kannst das Transkript unter ‚Erweiterte Optionen‘ manuell einfügen.")
    .bind("2026-10-06T05:00:00Z")
    .execute(&pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let app = build_router(test_state(pool));

    let req = with_connect_info(
        Request::builder()
            .method("POST")
            .uri(format!("/generations/{id}"))
            .body(Body::empty())
            .unwrap(),
    );
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;

    assert!(html.contains("keine Untertitel verfügbar"));
    assert!(html.contains("Retry summary"));
}

/// Legacy production rows contain NULLs in old columns; both /browse and the
/// generation partial must still render them (synthetic legacy schema).
#[tokio::test]
async fn legacy_null_rows_render_without_error() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::raw_sql(
        "CREATE TABLE summaries (\
            identifier INTEGER PRIMARY KEY, model TEXT, transcript TEXT, host TEXT, \
            original_source_link TEXT, include_comments INTEGER, include_timestamps INTEGER, \
            include_glossary INTEGER, output_language TEXT, summary TEXT, summary_done INTEGER, \
            generation_status TEXT, generation_attempt INTEGER, generation_epoch INTEGER, \
            generation_started_at TEXT, generation_updated_at TEXT, next_retry_at TEXT, \
            generation_error_code TEXT, generation_error_message TEXT, \
            provider_interaction_id TEXT, summary_input_tokens INTEGER, \
            summary_output_tokens INTEGER, summary_timestamp_start TEXT, \
            summary_timestamp_end TEXT, timestamps TEXT, timestamps_done INTEGER, \
            timestamps_input_tokens INTEGER, timestamps_output_tokens INTEGER, \
            timestamps_timestamp_start TEXT, timestamps_timestamp_end TEXT, \
            timestamped_summary_in_youtube_format TEXT, cost FLOAT, embedding BLOB, \
            embedding_model TEXT, full_embedding BLOB, google_search_grounding BOOLEAN, \
            url_context BOOLEAN, thinking TEXT, thinking_tokens INTEGER, thinking_level TEXT, \
            rs_summarizer_version TEXT)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO summaries (identifier, model, original_source_link, generation_status, \
            generation_attempt, generation_epoch, generation_started_at, generation_updated_at, \
            next_retry_at, generation_error_code, generation_error_message, \
            provider_interaction_id, google_search_grounding, url_context, thinking, \
            thinking_tokens, thinking_level, rs_summarizer_version) \
         VALUES (1, 'legacy-model', 'https://www.youtube.com/watch?v=legacy', 'succeeded', \
            0, 0, '', '', '', '', '', '', 0, 0, '', 0, 'high', '')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let app = build_router(test_state(pool));

    let browse_req = with_connect_info(
        Request::builder()
            .method("GET")
            .uri("/browse")
            .body(Body::empty())
            .unwrap(),
    );
    let browse_res = app.clone().oneshot(browse_req).await.unwrap();
    assert_eq!(browse_res.status(), StatusCode::OK);
    let browse_html = body_text(browse_res).await;
    assert!(browse_html.contains("legacy-model"));
    assert!(!browse_html.contains("cost:"));

    let gen_req = with_connect_info(
        Request::builder()
            .method("POST")
            .uri("/generations/1")
            .body(Body::empty())
            .unwrap(),
    );
    let gen_res = app.oneshot(gen_req).await.unwrap();
    assert_eq!(gen_res.status(), StatusCode::OK);
    let gen_html = body_text(gen_res).await;
    assert!(gen_html.contains("Summary Complete"));
}

/// Regression against the production database copy (read-only, no migrations).
/// Skipped when `summaries.db` is absent; the synthetic legacy test above
/// covers that case.
#[tokio::test]
async fn prod_copy_browse_and_generation_render() {
    use std::str::FromStr;
    let db_path = std::path::Path::new("summaries.db");
    if !db_path.exists() {
        eprintln!("skipping prod-copy regression: summaries.db not present");
        return;
    }
    let options = sqlx::sqlite::SqliteConnectOptions::from_str("sqlite:summaries.db")
        .unwrap()
        .read_only(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let app = build_router(test_state(pool.clone()));

    let browse_req = with_connect_info(
        Request::builder()
            .method("GET")
            .uri("/browse?page=0")
            .body(Body::empty())
            .unwrap(),
    );
    let browse_res = app.clone().oneshot(browse_req).await.unwrap();
    assert_eq!(browse_res.status(), StatusCode::OK);
    let browse_html = body_text(browse_res).await;
    assert!(
        browse_html.contains("Für YouTube kopieren"),
        "first browse page must offer the copy button"
    );
    assert!(!browse_html.contains("<footer>"));

    let succeeded: Option<i64> = sqlx::query_scalar(
        "SELECT identifier FROM summaries WHERE generation_status = 'succeeded' \
         ORDER BY identifier DESC LIMIT 1",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    if let Some(id) = succeeded {
        let gen_req = with_connect_info(
            Request::builder()
                .method("POST")
                .uri(format!("/generations/{id}"))
                .body(Body::empty())
                .unwrap(),
        );
        let gen_res = app.oneshot(gen_req).await.unwrap();
        assert_eq!(gen_res.status(), StatusCode::OK);
        let gen_html = body_text(gen_res).await;
        assert!(gen_html.contains("Summary Complete"));
        assert!(gen_html.contains(">Source</a>"));
    }
}
