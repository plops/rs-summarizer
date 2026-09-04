use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode, header},
};
use rs_summarizer::{
    build_router, db, models::SubmitForm, routes::extract_client_ip, state::AppState,
};
use sqlx::SqlitePool;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use tower::ServiceExt;

async fn setup_test_app() -> (axum::Router, SqlitePool) {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let model_options = Arc::new(rs_summarizer::state::get_default_models());
    let state = AppState {
        app_version: rs_summarizer::APP_VERSION,
        db: pool.clone(),
        model_options,
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
    };

    let app = build_router(state);
    (app, pool)
}

#[tokio::test]
async fn test_extract_client_ip_priority() {
    let mut headers = HeaderMap::new();
    let addr: SocketAddr = "192.168.1.10:12345".parse().unwrap();

    // Default socket addr
    assert_eq!(extract_client_ip(&headers, &addr), "192.168.1.10");

    // X-Real-IP
    headers.insert("x-real-ip", "10.0.0.1".parse().unwrap());
    assert_eq!(extract_client_ip(&headers, &addr), "10.0.0.1");

    // X-Forwarded-For takes priority over X-Real-IP
    headers.insert(
        "x-forwarded-for",
        "203.0.113.195, 70.41.3.18".parse().unwrap(),
    );
    assert_eq!(extract_client_ip(&headers, &addr), "203.0.113.195");
}

#[tokio::test]
async fn test_rating_workflow_and_anonymity() {
    let (app, pool) = setup_test_app().await;

    // Insert sample summary
    let form = SubmitForm {
        original_source_link: "https://youtube.com/watch?v=test_rating".to_string(),
        transcript: Some("Test transcript".to_string()),
        model: "gemini-3.6-flash".to_string(),
        google_search_grounding: false,
        url_context: false,
        include_glossary: false,
        output_language: "en".to_string(),
        thinking_level: Default::default(),
    };
    let id = db::insert_new_summary(
        &pool,
        &form,
        "127.0.0.1",
        "2026-01-01T00:00:00Z",
        "test-version",
    )
    .await
    .unwrap();

    let client_ip = "203.0.113.195";

    // 1. Submit rating: Summary 5 stars, Content 4 stars
    let mut req = Request::builder()
        .method("POST")
        .uri(format!("/summaries/{}/rate", id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header("x-forwarded-for", client_ip)
        .body(Body::from("summary_rating=5&content_rating=4"))
        .unwrap();
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(SocketAddr::from((
            [127, 0, 0, 1],
            8080,
        ))));

    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    assert!(body_str.contains("5.0"));
    assert!(body_str.contains("4.0"));
    assert!(body_str.contains("Your rating: 5★"));
    assert!(body_str.contains("Your rating: 4★"));
    // Ensure client IP is NEVER rendered in HTML
    assert!(!body_str.contains(client_ip));

    // 2. Fetch /browse page and verify ratings appear
    let mut browse_req = Request::builder()
        .method("GET")
        .uri("/browse")
        .header("x-forwarded-for", client_ip)
        .body(Body::empty())
        .unwrap();
    browse_req
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(SocketAddr::from((
            [127, 0, 0, 1],
            8080,
        ))));

    let browse_res = app.clone().oneshot(browse_req).await.unwrap();
    assert_eq!(browse_res.status(), StatusCode::OK);

    let browse_bytes = axum::body::to_bytes(browse_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let browse_str = String::from_utf8(browse_bytes.to_vec()).unwrap();

    assert!(browse_str.contains("Summary Rating:"));
    assert!(browse_str.contains("Article Rating:"));
    assert!(browse_str.contains("Your rating: 5★"));
    assert!(!browse_str.contains(client_ip));

    // 3. Upsert rating: change summary_rating to 2 stars
    let mut req_update = Request::builder()
        .method("POST")
        .uri(format!("/summaries/{}/rate", id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header("x-forwarded-for", client_ip)
        .body(Body::from("summary_rating=2"))
        .unwrap();
    req_update
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(SocketAddr::from((
            [127, 0, 0, 1],
            8080,
        ))));

    let res_update = app.clone().oneshot(req_update).await.unwrap();
    assert_eq!(res_update.status(), StatusCode::OK);

    let update_bytes = axum::body::to_bytes(res_update.into_body(), usize::MAX)
        .await
        .unwrap();
    let update_str = String::from_utf8(update_bytes.to_vec()).unwrap();

    assert!(update_str.contains("2.0"));
    assert!(update_str.contains("Your rating: 2★"));
    assert!(update_str.contains("Your rating: 4★")); // content rating preserved
}

#[tokio::test]
async fn test_invalid_rating_values() {
    let (app, pool) = setup_test_app().await;

    let form = SubmitForm {
        original_source_link: "https://youtube.com/watch?v=test_invalid".to_string(),
        transcript: Some("Test".to_string()),
        model: "gemini-3.6-flash".to_string(),
        google_search_grounding: false,
        url_context: false,
        include_glossary: false,
        output_language: "en".to_string(),
        thinking_level: Default::default(),
    };
    let id = db::insert_new_summary(
        &pool,
        &form,
        "127.0.0.1",
        "2026-01-01T00:00:00Z",
        "test-version",
    )
    .await
    .unwrap();

    let mut req = Request::builder()
        .method("POST")
        .uri(format!("/summaries/{}/rate", id))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("summary_rating=6"))
        .unwrap();
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(SocketAddr::from((
            [127, 0, 0, 1],
            8080,
        ))));

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_rating_non_existent_summary() {
    let (app, _pool) = setup_test_app().await;

    let mut req = Request::builder()
        .method("POST")
        .uri("/summaries/999999/rate")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("summary_rating=5"))
        .unwrap();
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(SocketAddr::from((
            [127, 0, 0, 1],
            8080,
        ))));

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
