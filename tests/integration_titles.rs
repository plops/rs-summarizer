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

async fn insert_row(pool: &SqlitePool, link: &str, title: &str) -> i64 {
    sqlx::query(
        "INSERT INTO summaries (model, original_source_link, summary, summary_done, \
            generation_status, cost, summary_timestamp_start, title) \
         VALUES (?, ?, ?, 1, 'succeeded', ?, ?, ?)",
    )
    .bind("gemini-3.6-flash")
    .bind(link)
    .bind("Summary body.")
    .bind(0.01_f64)
    .bind("2026-10-24T05:00:00Z")
    .bind(title)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

#[tokio::test]
async fn untitled_rows_fall_back_to_link_without_heading() {
    let pool = migrated_pool().await;
    let id = insert_row(&pool, "https://www.youtube.com/watch?v=9bZkp7q19f0", "").await;
    let app = build_router(test_state(pool));

    let overview_req = with_connect_info(
        Request::builder()
            .method("GET")
            .uri("/entries")
            .body(Body::empty())
            .unwrap(),
    );
    let overview_res = app.clone().oneshot(overview_req).await.unwrap();
    assert_eq!(overview_res.status(), StatusCode::OK);
    let overview_html = body_text(overview_res).await;
    // No title heading anywhere on the untitled overview; link still shown.
    assert!(!overview_html.contains("<h3>"));
    assert!(overview_html.contains("9bZkp7q19f0"));

    let detail_req = with_connect_info(
        Request::builder()
            .method("GET")
            .uri(format!("/entries/{id}/detail"))
            .body(Body::empty())
            .unwrap(),
    );
    let detail_res = app.clone().oneshot(detail_req).await.unwrap();
    assert_eq!(detail_res.status(), StatusCode::OK);
    let detail_html = body_text(detail_res).await;
    assert!(!detail_html.contains("<h3>"));
    assert!(detail_html.contains("Summary body."));

    let browse_req = with_connect_info(
        Request::builder()
            .method("GET")
            .uri("/browse")
            .body(Body::empty())
            .unwrap(),
    );
    let browse_res = app.oneshot(browse_req).await.unwrap();
    assert_eq!(browse_res.status(), StatusCode::OK);
    let browse_html = body_text(browse_res).await;
    assert!(!browse_html.contains("<h3>"));
}

/// The backfill worker touches no network once the daily budget is spent.
#[tokio::test]
async fn backfill_stops_when_budget_is_exhausted() {
    let pool = migrated_pool().await;
    insert_row(&pool, "https://www.youtube.com/watch?v=9bZkp7q19f0", "").await;
    // Spend the whole YouTube allowance for today.
    for _ in 0..100 {
        rs_summarizer::db::title_budget_consume(&pool, rs_summarizer::tasks::TITLE_SOURCE_YOUTUBE)
            .await
            .unwrap();
    }
    let state = test_state(pool.clone());
    let (yt, hn) = rs_summarizer::tasks::title_backfill_once(&pool, &state, 10).await;
    assert_eq!((yt, hn), (0, 0));
    // Row stays untitled for another day.
    let row = rs_summarizer::db::fetch_summary(&pool, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.title, "");
}

/// Unclassifiable URLs are skipped without any fetch attempt (no network).
#[tokio::test]
async fn backfill_skips_unclassifiable_urls() {
    let pool = migrated_pool().await;
    insert_row(&pool, "https://example.com/some-article", "").await;
    let state = test_state(pool.clone());
    let (yt, hn) = rs_summarizer::tasks::title_backfill_once(&pool, &state, 10).await;
    assert_eq!((yt, hn), (0, 0));
    // No budget spent on URLs we cannot title.
    assert_eq!(
        rs_summarizer::db::title_budget_remaining(
            &pool,
            rs_summarizer::tasks::TITLE_SOURCE_YOUTUBE,
            100
        )
        .await
        .unwrap(),
        100
    );
}

/// Live check against the real Hacker News API (single cheap item request).
/// Skipped gracefully when the network is unavailable.
#[tokio::test]
async fn live_hn_story_title_matches_known_post() {
    let svc = rs_summarizer::services::hacker_news::HackerNewsService::new();
    // HN item 8863: the classic "Dropbox" launch post, title is stable.
    match svc.fetch_story_title(8863).await {
        Ok(title) => {
            assert!(
                title.to_lowercase().contains("dropbox"),
                "unexpected HN title: {title}"
            );
        }
        Err(e) => {
            eprintln!("skipping live HN check (no network?): {e}");
        }
    }
}
