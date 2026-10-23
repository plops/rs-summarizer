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

/// Seeds one succeeded row with distinctive sentinels for summary and
/// transcript so tests can prove what leaves the server and what does not.
async fn insert_entries_row(pool: &SqlitePool) -> i64 {
    sqlx::query(
        "INSERT INTO summaries (model, original_source_link, transcript, summary, summary_done, \
            generation_status, cost, timestamps_done, timestamped_summary_in_youtube_format, \
            summary_timestamp_start, rs_summarizer_version) \
         VALUES (?, ?, ?, ?, 1, 'succeeded', ?, 1, ?, ?, ?)",
    )
    .bind("gemini-3.6-flash")
    .bind("https://www.youtube.com/watch?v=9bZkp7q19f0")
    .bind("TRANSCRIPT_SENTINEL_ENTRIES_7")
    .bind("**1:23 EntriesIntro**\n\nSUMMARY_SENTINEL_ENTRIES_7 about the video.")
    .bind(0.038_797_5_f64)
    .bind("*EntriesIntro* 1:23\n\nSUMMARY_SENTINEL_ENTRIES_7 about the video.")
    .bind("2026-10-23T05:00:00Z")
    .bind("1.8.5")
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

#[tokio::test]
async fn entries_overview_shows_metadata_but_no_summary_or_transcript() {
    let pool = migrated_pool().await;
    insert_entries_row(&pool).await;
    let app = build_router(test_state(pool));

    let req = with_connect_info(
        Request::builder()
            .method("GET")
            .uri("/entries")
            .body(Body::empty())
            .unwrap(),
    );
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;

    // Metadata is visible: link, request datetime, model, formatted cost.
    assert!(html.contains("https://www.youtube.com/watch?v=9bZkp7q19f0"));
    assert!(html.contains("2026-10-23T05:00:00Z"));
    assert!(html.contains("gemini-3.6-flash"));
    assert!(html.contains("$0.04"));
    // Lazy-load wiring per row, no eager summary.
    assert!(html.contains("/detail"));
    assert!(html.contains("Details zu #"));
    assert!(
        !html.contains("SUMMARY_SENTINEL_ENTRIES_7"),
        "overview must not render the summary"
    );
    assert!(
        !html.contains("TRANSCRIPT_SENTINEL_ENTRIES_7"),
        "overview must not leak the transcript"
    );
}

#[tokio::test]
async fn entry_detail_shows_summary_but_never_transcript() {
    let pool = migrated_pool().await;
    let id = insert_entries_row(&pool).await;
    let app = build_router(test_state(pool));

    let req = with_connect_info(
        Request::builder()
            .method("GET")
            .uri(format!("/entries/{id}/detail"))
            .body(Body::empty())
            .unwrap(),
    );
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;

    assert!(html.contains("SUMMARY_SENTINEL_ENTRIES_7"));
    assert!(html.contains("gemini-3.6-flash"));
    assert!(html.contains("$0.04"));
    assert!(html.contains(">Source</a>"));
    assert!(html.contains(&format!("rating-container-{id}")));
    assert!(html.contains("Für YouTube kopieren"));
    // Timestamps inside the summary link to YouTube (1:23 -> 83s).
    assert!(html.contains("t=83s"));
    assert!(
        !html.contains("TRANSCRIPT_SENTINEL_ENTRIES_7"),
        "detail must never leak the transcript"
    );
}

#[tokio::test]
async fn entries_handles_bad_page_and_unknown_id() {
    let pool = migrated_pool().await;
    insert_entries_row(&pool).await;
    let app = build_router(test_state(pool));

    // Far-out page renders empty but fine.
    let req = with_connect_info(
        Request::builder()
            .method("GET")
            .uri("/entries?page=999999")
            .body(Body::empty())
            .unwrap(),
    );
    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(!html.contains("SUMMARY_SENTINEL_ENTRIES_7"));

    // Unknown detail id is a clean 404, not a 500.
    let req = with_connect_info(
        Request::builder()
            .method("GET")
            .uri("/entries/999999/detail")
            .body(Body::empty())
            .unwrap(),
    );
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Regression against the production database copy (read-only).
/// Skipped when `summaries.db` is absent.
#[tokio::test]
async fn prod_copy_entries_overview_and_detail() {
    use std::str::FromStr;
    if !std::path::Path::new("summaries.db").exists() {
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

    let overview_req = with_connect_info(
        Request::builder()
            .method("GET")
            .uri("/entries?page=0")
            .body(Body::empty())
            .unwrap(),
    );
    let overview_res = app.clone().oneshot(overview_req).await.unwrap();
    assert_eq!(overview_res.status(), StatusCode::OK);
    let overview_html = body_text(overview_res).await;
    assert!(overview_html.contains("Details zu #"));

    // The whole table is reachable: the last page (50 rows per page) also
    // renders fine, proving pagination covers all ~19.6k rows.
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM summaries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(total > 50, "prod copy should hold many rows, got {total}");
    let last_page = (total as u32 - 1) / 50;
    let tail_req = with_connect_info(
        Request::builder()
            .method("GET")
            .uri(format!("/entries?page={last_page}"))
            .body(Body::empty())
            .unwrap(),
    );
    let tail_res = app.clone().oneshot(tail_req).await.unwrap();
    assert_eq!(tail_res.status(), StatusCode::OK);
    let tail_html = body_text(tail_res).await;
    assert!(tail_html.contains("Details zu #"));
    assert!(!tail_html.contains("Weiter →"));

    let latest: Option<(i64, String)> = sqlx::query_as(
        "SELECT identifier, COALESCE(transcript, '') FROM summaries \
         ORDER BY identifier DESC LIMIT 1",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    if let Some((id, transcript)) = latest {
        let detail_req = with_connect_info(
            Request::builder()
                .method("GET")
                .uri(format!("/entries/{id}/detail"))
                .body(Body::empty())
                .unwrap(),
        );
        let detail_res = app.oneshot(detail_req).await.unwrap();
        assert_eq!(detail_res.status(), StatusCode::OK);
        let detail_html = body_text(detail_res).await;
        // The newest row must render something meaningful …
        assert!(
            detail_html.contains("entry-detail-")
                && (detail_html.contains("Source") || detail_html.contains("rs-summarizer")),
            "detail fragment missing for id {id}"
        );
        // … but never the raw transcript.
        if !transcript.is_empty() {
            let probe: String = transcript.chars().take(48).collect();
            if probe.len() >= 24 {
                assert!(
                    !detail_html.contains(&probe),
                    "detail leaks transcript for id {id}"
                );
            }
        }
    }
}
