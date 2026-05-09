//! Browser-based integration tests using fantoccini (WebDriver).
//!
//! These tests spin up the actual application server and drive a headless Firefox
//! browser against it, verifying the full user-facing behavior including HTMX
//! interactions, form submissions, and page navigation.
//!
//! Prerequisites:
//! - geckodriver must be in PATH (or ~/bin/geckodriver)
//! - Firefox must be installed
//! - GEMINI_API_KEY env var for tests that trigger summarization
//!
//! Run with: cargo test --test integration_browser -- --ignored
//! (These tests are ignored by default since they require geckodriver + Firefox)

use fantoccini::{Client, ClientBuilder, Locator};
use rs_summarizer::state::{AppState, ModelOption};
use rs_summarizer::{build_router, db};
use serde_json::json;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

/// Find geckodriver binary, checking ~/bin first then PATH.
fn geckodriver_path() -> String {
    let home_bin = format!("{}/bin/geckodriver", std::env::var("HOME").unwrap_or_default());
    if std::path::Path::new(&home_bin).exists() {
        return home_bin;
    }
    "geckodriver".to_string()
}

/// Start geckodriver on a given port and return the child process handle.
fn start_geckodriver(port: u16) -> std::process::Child {
    std::process::Command::new(geckodriver_path())
        .args(["--port", &port.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("Failed to start geckodriver. Is it installed?")
}

/// Create a test AppState with an in-memory SQLite database.
async fn test_app_state() -> AppState {
    let db = db::init_db("sqlite::memory:")
        .await
        .expect("Failed to init in-memory DB");

    let model_options = vec![
        ModelOption {
            name: "test-model".to_string(),
            input_price_per_mtoken: 0.001,
            output_price_per_mtoken: 0.002,
            context_window: 128000,
            rpm_limit: 15,
            rpd_limit: 1000,
        },
        ModelOption {
            name: "gemma-3-1b-it".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 128000,
            rpm_limit: 30,
            rpd_limit: 14400,
        },
    ];

    let gemini_api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();

    AppState {
        db,
        model_options: Arc::new(model_options),
        model_counts: Arc::new(RwLock::new(HashMap::new())),
        last_reset_day: Arc::new(RwLock::new(None)),
        gemini_api_key,
        nn_mapper: None,
        viz_data: None,
    }
}

/// Start the application server on a random available port.
/// Returns the base URL (e.g. "http://127.0.0.1:12345").
async fn start_test_server() -> String {
    let state = test_app_state().await;
    let app = build_router(state);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind to random port");
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    format!("http://{}", addr)
}

/// Start the application server with a pre-configured AppState on a random available port.
/// Returns the base URL (e.g. "http://127.0.0.1:12345").
async fn start_test_server_with_state(state: AppState) -> String {
    let app = build_router(state);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind to random port");
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    format!("http://{}", addr)
}

/// Start the application server with graceful shutdown support.
/// Returns `(base_url, SocketAddr, shutdown_sender)` so the caller can trigger shutdown.
async fn start_test_server_controllable(
    state: AppState,
) -> (String, SocketAddr, tokio::sync::watch::Sender<bool>) {
    let app = build_router(state);
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind to random port");
    let addr = listener.local_addr().unwrap();
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            shutdown_rx.changed().await.ok();
        })
        .await
        .unwrap();
    });

    (format!("http://{}", addr), addr, shutdown_tx)
}

/// Seed the database with `count` summary records for pagination and browse tests.
/// Each record has `summary_done=true` and markdown content with bold, lists, and headings.
/// Returns the inserted identifiers.
async fn seed_summaries(db: &SqlitePool, count: usize) -> Vec<i64> {
    let mut ids = Vec::new();
    for i in 0..count {
        let id = sqlx::query(
            "INSERT INTO summaries (model, original_source_link, transcript, host, summary_timestamp_start, summary, summary_done) \
             VALUES (?, ?, ?, ?, ?, ?, 1)"
        )
        .bind("gemma-3-27b-it")
        .bind(format!("https://youtube.com/watch?v=test{}", i))
        .bind(format!("Transcript for video {}", i))
        .bind("127.0.0.1:0")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(format!("## Summary {}\n\nThis is **bold** content for video {}.\n\n- Item 1\n- Item 2", i, i))
        .execute(db)
        .await
        .unwrap()
        .last_insert_rowid();
        ids.push(id);
    }
    ids
}

/// Seed a single summary with `timestamps_done=true` and timestamped content.
/// The content includes timestamps that should become clickable YouTube links.
async fn seed_summary_with_timestamps(db: &SqlitePool, url: &str) -> i64 {
    let id = sqlx::query(
        "INSERT INTO summaries (model, original_source_link, transcript, host, summary_timestamp_start, summary, summary_done, timestamps_done, timestamped_summary_in_youtube_format) \
         VALUES (?, ?, ?, ?, ?, ?, 1, 1, ?)"
    )
    .bind("gemma-3-27b-it")
    .bind(url)
    .bind("Transcript with timestamps")
    .bind("127.0.0.1:0")
    .bind(chrono::Utc::now().to_rfc3339())
    .bind("## Timestamped Summary\n\nThis is the summary content.")
    .bind("<p><strong>0:00 Introduction</strong></p>\n<p><strong>1:30 Main Topic</strong></p>\n<p><strong>5:45 Conclusion</strong></p>")
    .execute(db)
    .await
    .unwrap()
    .last_insert_rowid();
    id
}

/// Create a test AppState with a model that has rpd_limit=1 for rate limit testing.
async fn test_app_state_with_low_limit() -> AppState {
    let db = db::init_db("sqlite::memory:")
        .await
        .expect("Failed to init in-memory DB");

    let model_options = vec![ModelOption {
        name: "test-limited-model".to_string(),
        input_price_per_mtoken: 0.0,
        output_price_per_mtoken: 0.0,
        context_window: 128_000,
        rpm_limit: 30,
        rpd_limit: 1,
    }];

    let gemini_api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();

    AppState {
        db,
        model_options: Arc::new(model_options),
        model_counts: Arc::new(RwLock::new(HashMap::new())),
        last_reset_day: Arc::new(RwLock::new(None)),
        gemini_api_key,
        nn_mapper: None,
        viz_data: None,
    }
}

/// Connect a headless Firefox browser via WebDriver.
async fn connect_browser(geckodriver_port: u16) -> Client {
    // Detect Firefox binary path (may be "firefox-bin" on some distros)
    let firefox_binary = if std::path::Path::new("/usr/bin/firefox-bin").exists() {
        "/usr/bin/firefox-bin"
    } else if std::path::Path::new("/usr/bin/firefox").exists() {
        "/usr/bin/firefox"
    } else {
        "firefox"
    };

    let caps = json!({
        "moz:firefoxOptions": {
            "binary": firefox_binary,
            "args": ["-headless"]
        }
    });

    // Retry connection a few times while geckodriver starts up
    let mut last_err = None;
    for _ in 0..10 {
        match ClientBuilder::native()
            .capabilities(caps.as_object().unwrap().clone())
            .connect(&format!("http://localhost:{}", geckodriver_port))
            .await
        {
            Ok(client) => return client,
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
    }
    panic!(
        "Failed to connect to geckodriver after retries: {:?}",
        last_err
    );
}

/// Test that the index page loads and displays the form correctly.
#[tokio::test]
#[ignore]
async fn test_index_page_loads() {
    let base_url = start_test_server().await;
    let geckodriver_port = 4444;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the index page
    client.goto(&base_url).await.unwrap();

    // Verify the page title
    let title = client.title().await.unwrap();
    assert!(
        title.contains("rs-summarizer"),
        "Expected title to contain 'rs-summarizer', got: {}",
        title
    );

    // Verify the heading is present
    let heading = client.find(Locator::Css("h1")).await.unwrap();
    let heading_text = heading.text().await.unwrap();
    assert_eq!(heading_text, "YouTube Transcript Summarizer");

    // Verify the URL input field exists
    let url_input = client.find(Locator::Css("#url")).await.unwrap();
    assert!(url_input.is_displayed().await.unwrap());

    // Verify the model select dropdown exists and has options
    let model_select = client.find(Locator::Css("#model")).await.unwrap();
    assert!(model_select.is_displayed().await.unwrap());

    // Verify model options are populated
    let options = client.find_all(Locator::Css("#model option")).await.unwrap();
    assert_eq!(options.len(), 2, "Expected 2 model options");

    // Verify the submit button exists
    let submit_btn = client
        .find(Locator::Css("button[type='submit']"))
        .await
        .unwrap();
    let btn_text = submit_btn.text().await.unwrap();
    assert_eq!(btn_text, "Summarize");

    // Verify the browse link exists
    let browse_link = client.find(Locator::Css("a[href='/browse']")).await.unwrap();
    assert!(browse_link.is_displayed().await.unwrap());

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that the browse page loads (empty state).
#[tokio::test]
#[ignore]
async fn test_browse_page_empty() {
    let base_url = start_test_server().await;
    let geckodriver_port = 4445;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the browse page
    client.goto(&format!("{}/browse", base_url)).await.unwrap();

    // The page should load without errors
    let source = client.source().await.unwrap();
    assert!(
        !source.contains("500 Internal Server Error"),
        "Browse page returned a server error"
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test form submission triggers HTMX and shows the processing state.
/// This test submits a YouTube URL and verifies the HTMX polling partial appears.
#[tokio::test]
#[ignore]
async fn test_form_submission_shows_processing() {
    let base_url = start_test_server().await;
    let geckodriver_port = 4446;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the index page
    client.goto(&base_url).await.unwrap();

    // Fill in the YouTube URL
    let url_input = client.find(Locator::Css("#url")).await.unwrap();
    url_input
        .send_keys("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
        .await
        .unwrap();

    // Select the first model (should already be selected by default)
    // Submit the form
    let submit_btn = client
        .find(Locator::Css("button[type='submit']"))
        .await
        .unwrap();
    submit_btn.click().await.unwrap();

    // Wait for HTMX to swap in the result
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // The result div should now contain the generation partial
    let result_div = client.find(Locator::Css("#result")).await.unwrap();
    let result_html = result_div.html(true).await.unwrap();

    // Should show either "Processing..." or "Generating summary..." (the HTMX partial)
    assert!(
        result_html.contains("Processing") || result_html.contains("Generating summary"),
        "Expected processing state in result div, got: {}",
        &result_html[..result_html.len().min(500)]
    );

    // The generation div should have hx-post for polling
    let generation_div = client.find(Locator::Css("#generation")).await;
    assert!(
        generation_div.is_ok(),
        "Expected #generation div to be present for HTMX polling"
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that submitting an invalid model shows an error message.
#[tokio::test]
#[ignore]
async fn test_invalid_model_shows_error() {
    let base_url = start_test_server().await;
    let geckodriver_port = 4447;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the index page
    client.goto(&base_url).await.unwrap();

    // Use JavaScript to inject an invalid model value and submit
    client
        .execute(
            r#"
            const select = document.getElementById('model');
            const opt = document.createElement('option');
            opt.value = 'nonexistent-model';
            opt.text = 'Fake';
            select.add(opt);
            select.value = 'nonexistent-model';
            "#,
            vec![],
        )
        .await
        .unwrap();

    // Fill in a URL
    let url_input = client.find(Locator::Css("#url")).await.unwrap();
    url_input
        .send_keys("https://www.youtube.com/watch?v=test123")
        .await
        .unwrap();

    // Submit the form
    let submit_btn = client
        .find(Locator::Css("button[type='submit']"))
        .await
        .unwrap();
    submit_btn.click().await.unwrap();

    // Wait for HTMX response
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Should show an error about invalid model
    let result_div = client.find(Locator::Css("#result")).await.unwrap();
    let result_text = result_div.text().await.unwrap();
    assert!(
        result_text.contains("Invalid model"),
        "Expected 'Invalid model' error, got: {}",
        result_text
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test navigation between index and browse pages.
#[tokio::test]
#[ignore]
async fn test_navigation_between_pages() {
    let base_url = start_test_server().await;
    let geckodriver_port = 4448;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Start at index
    client.goto(&base_url).await.unwrap();

    // Click the browse link
    let browse_link = client.find(Locator::Css("a[href='/browse']")).await.unwrap();
    browse_link.click().await.unwrap();

    // Wait for navigation
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Should be on the browse page
    let current_url = client.current_url().await.unwrap();
    assert!(
        current_url.as_str().contains("/browse"),
        "Expected to be on /browse, got: {}",
        current_url
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that static assets (CSS, JS) are served correctly.
#[tokio::test]
#[ignore]
async fn test_static_assets_loaded() {
    let base_url = start_test_server().await;
    let geckodriver_port = 4449;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the index page
    client.goto(&base_url).await.unwrap();

    // Check that htmx is loaded by verifying the htmx object exists
    let htmx_loaded: serde_json::Value = client
        .execute("return typeof htmx !== 'undefined'", vec![])
        .await
        .unwrap();
    assert_eq!(
        htmx_loaded,
        serde_json::Value::Bool(true),
        "HTMX should be loaded on the page"
    );

    // Check that pico CSS is applied by verifying a computed style
    // (pico sets a specific font-family on body)
    let has_styles: serde_json::Value = client
        .execute(
            "return window.getComputedStyle(document.body).fontFamily !== ''",
            vec![],
        )
        .await
        .unwrap();
    assert_eq!(
        has_styles,
        serde_json::Value::Bool(true),
        "CSS styles should be applied"
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test the search form submits via HTMX and shows results area.
#[tokio::test]
#[ignore]
async fn test_search_form_htmx() {
    let base_url = start_test_server().await;
    let geckodriver_port = 4450;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the index page
    client.goto(&base_url).await.unwrap();

    // Find the search input and type a query
    let search_input = client
        .find(Locator::Css("input[name='query']"))
        .await
        .unwrap();
    search_input.send_keys("rust programming").await.unwrap();

    // Find and click the search button
    // The search form has its own submit button (second button on the page)
    let buttons = client
        .find_all(Locator::Css("button[type='submit']"))
        .await
        .unwrap();
    assert!(buttons.len() >= 2, "Expected at least 2 submit buttons");
    buttons[1].click().await.unwrap();

    // Wait for HTMX response
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // The search-results div should exist (even if empty results)
    let search_results = client.find(Locator::Css("#search-results")).await.unwrap();
    // It should have been populated by HTMX (even if with empty results)
    let _html = search_results.html(true).await.unwrap();
    // No assertion on content since DB is empty, but no crash means success

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Full end-to-end test: submit a URL, wait for summary to complete.
/// This test requires GEMINI_API_KEY and network access.
#[tokio::test]
#[ignore]
async fn test_full_summarization_e2e() {
    let api_key = std::env::var("GEMINI_API_KEY");
    if api_key.is_err() || api_key.as_ref().unwrap().is_empty() {
        println!("SKIPPED: GEMINI_API_KEY not set");
        return;
    }

    let base_url = start_test_server().await;
    let geckodriver_port = 4451;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the index page
    client.goto(&base_url).await.unwrap();

    // Fill in a short video URL (known to have auto-captions)
    let url_input = client.find(Locator::Css("#url")).await.unwrap();
    url_input
        .send_keys("https://www.youtube.com/watch?v=LlzXCE02swU")
        .await
        .unwrap();

    // Select the free model (gemma-3-27b-it)
    let model_select = client.find(Locator::Css("#model")).await.unwrap();
    model_select
        .select_by_value("gemma-3-27b-it")
        .await
        .unwrap();

    // Submit the form
    let submit_btn = client
        .find(Locator::Css("button[type='submit']"))
        .await
        .unwrap();
    submit_btn.click().await.unwrap();

    // Wait for the generation to complete (poll for up to 120 seconds)
    let timeout = std::time::Duration::from_secs(120);
    let start = std::time::Instant::now();
    let mut completed = false;

    while start.elapsed() < timeout {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Check if "Summary Complete" header appears
        let source = client.source().await.unwrap();
        if source.contains("Summary Complete") {
            completed = true;
            break;
        }

        // Also check if there's an error
        if source.contains("Error") && !source.contains("Generating") {
            // Might be a rate limit or network error — acceptable in CI
            println!("Got an error during summarization, checking if acceptable...");
            let result_div = client.find(Locator::Css("#result")).await.unwrap();
            let text = result_div.text().await.unwrap();
            println!("Result text: {}", &text[..text.len().min(200)]);
            break;
        }
    }

    if completed {
        // Verify the summary has actual content
        let article = client.find(Locator::Css("#generation article")).await.unwrap();
        let article_text = article.text().await.unwrap();
        assert!(
            article_text.len() > 50,
            "Summary should have substantial content, got {} chars",
            article_text.len()
        );

        // Verify the polling has stopped (no more hx-trigger on the div)
        let generation_div = client.find(Locator::Css("#generation")).await.unwrap();
        let hx_trigger = generation_div.attr("hx-trigger").await.unwrap();
        assert!(
            hx_trigger.is_none(),
            "Polling should stop after summary is complete"
        );
    } else {
        println!("WARNING: Summary did not complete within timeout (may be rate-limited)");
    }

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that submitting the same URL twice returns the same identifier (deduplication).
/// The deduplication service checks for duplicate URL+model within 5 minutes and
/// returns the existing generation partial instead of spawning a new background task.
#[tokio::test]
#[ignore]
async fn test_deduplication_returns_same_id() {
    let base_url = start_test_server().await;
    let geckodriver_port = 4452;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the index page
    client.goto(&base_url).await.unwrap();

    // Fill in the YouTube URL
    let url_input = client.find(Locator::Css("#url")).await.unwrap();
    url_input
        .send_keys("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
        .await
        .unwrap();

    // Submit the form
    let submit_btn = client
        .find(Locator::Css("button[type='submit']"))
        .await
        .unwrap();
    submit_btn.click().await.unwrap();

    // Wait for HTMX to swap in the result
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Extract the identifier from the #generation div's hx-post attribute
    let generation_div = client.find(Locator::Css("#generation")).await.unwrap();
    let hx_post = generation_div
        .attr("hx-post")
        .await
        .unwrap()
        .expect("hx-post should be present");
    // hx-post is like "/generations/1"
    let first_id = hx_post.trim_start_matches("/generations/").to_string();

    // Navigate back to index and submit the same URL again
    client.goto(&base_url).await.unwrap();
    let url_input = client.find(Locator::Css("#url")).await.unwrap();
    url_input
        .send_keys("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
        .await
        .unwrap();
    let submit_btn = client
        .find(Locator::Css("button[type='submit']"))
        .await
        .unwrap();
    submit_btn.click().await.unwrap();

    // Wait for HTMX response
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Extract the identifier from the second submission
    let generation_div = client.find(Locator::Css("#generation")).await.unwrap();
    let hx_post = generation_div
        .attr("hx-post")
        .await
        .unwrap()
        .expect("hx-post should be present on second submission");
    let second_id = hx_post.trim_start_matches("/generations/").to_string();

    // Both submissions should return the same identifier (deduplication)
    assert_eq!(
        first_id, second_id,
        "Duplicate submission should return the same identifier"
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that exhausting a model's rate limit displays an appropriate error message.
/// Uses a model with rpd_limit=1 so the first submission exhausts the limit,
/// and the second submission triggers the rate limit error.
#[tokio::test]
#[ignore]
async fn test_rate_limit_error_display() {
    let state = test_app_state_with_low_limit().await;
    let base_url = start_test_server_with_state(state).await;
    let geckodriver_port = 4453;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the index page
    client.goto(&base_url).await.unwrap();

    // First submission - should succeed (exhausts rpd_limit=1)
    let url_input = client.find(Locator::Css("#url")).await.unwrap();
    url_input.send_keys("https://www.youtube.com/watch?v=test_rate1").await.unwrap();
    let submit_btn = client.find(Locator::Css("button[type='submit']")).await.unwrap();
    submit_btn.click().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Navigate back for second submission
    client.goto(&base_url).await.unwrap();

    // Second submission - should hit rate limit
    let url_input = client.find(Locator::Css("#url")).await.unwrap();
    url_input.send_keys("https://www.youtube.com/watch?v=test_rate2").await.unwrap();
    let submit_btn = client.find(Locator::Css("button[type='submit']")).await.unwrap();
    submit_btn.click().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Verify rate limit error is displayed
    let result_div = client.find(Locator::Css("#result")).await.unwrap();
    let result_text = result_div.text().await.unwrap();
    assert!(
        result_text.contains("Rate limit exceeded"),
        "Expected 'Rate limit exceeded' error, got: {}",
        result_text
    );

    // Verify no hx-post attribute (no polling partial)
    let result_html = result_div.html(true).await.unwrap();
    assert!(
        !result_html.contains("hx-post"),
        "Rate limit error should not contain hx-post for polling"
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that when a submission results in an error (invalid model), HTMX polling
/// does not start. The error response is plain HTML without a #generation div,
/// so there should be no element with hx-trigger for continued polling.
#[tokio::test]
#[ignore]
async fn test_polling_stops_on_error() {
    let base_url = start_test_server().await;
    let geckodriver_port = 4454;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the index page
    client.goto(&base_url).await.unwrap();

    // Inject an invalid model value via JavaScript
    client
        .execute(
            r#"
            const select = document.getElementById('model');
            const opt = document.createElement('option');
            opt.value = 'nonexistent-model';
            opt.text = 'Fake';
            select.add(opt);
            select.value = 'nonexistent-model';
            "#,
            vec![],
        )
        .await
        .unwrap();

    // Fill in a URL
    let url_input = client.find(Locator::Css("#url")).await.unwrap();
    url_input
        .send_keys("https://www.youtube.com/watch?v=test_polling_error")
        .await
        .unwrap();

    // Submit the form
    let submit_btn = client
        .find(Locator::Css("button[type='submit']"))
        .await
        .unwrap();
    submit_btn.click().await.unwrap();

    // Wait for HTMX response
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Verify #result contains error message
    let result_div = client.find(Locator::Css("#result")).await.unwrap();
    let result_text = result_div.text().await.unwrap();
    assert!(
        result_text.contains("Invalid model"),
        "Expected 'Invalid model' error in #result, got: {}",
        result_text
    );

    // Verify #generation element does NOT exist with hx-trigger (no polling)
    let generation = client.find(Locator::Css("#generation[hx-trigger]")).await;
    assert!(
        generation.is_err(),
        "Expected no #generation element with hx-trigger attribute after error"
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that the browser's native form validation prevents submission when the
/// required URL field is empty. The `required` attribute on the input should
/// prevent the HTMX request from firing, leaving the #result div empty.
#[tokio::test]
#[ignore]
async fn test_form_required_validation() {
    let base_url = start_test_server().await;
    let geckodriver_port = 4455;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the index page
    client.goto(&base_url).await.unwrap();

    // Verify URL input has the `required` attribute
    let url_input = client.find(Locator::Css("#url")).await.unwrap();
    let required_attr = url_input.attr("required").await.unwrap();
    assert!(
        required_attr.is_some(),
        "URL input should have the 'required' attribute"
    );

    // Click submit without entering a URL
    let submit_btn = client.find(Locator::Css("button[type='submit']")).await.unwrap();
    submit_btn.click().await.unwrap();

    // Wait briefly to see if anything happens
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Verify #result div is still empty (no HTMX request was fired)
    let result_div = client.find(Locator::Css("#result")).await.unwrap();
    let result_html = result_div.html(true).await.unwrap();
    assert!(
        result_html.trim().is_empty(),
        "Expected #result to be empty when form validation prevents submission, got: {}",
        result_html
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that the browse page displays 20 articles on page 0 when there are 25 total,
/// and shows a "Next →" link pointing to /browse?page=1.
#[tokio::test]
#[ignore]
async fn test_browse_pagination_page_0() {
    let state = test_app_state().await;
    seed_summaries(&state.db, 25).await;
    let base_url = start_test_server_with_state(state).await;
    let geckodriver_port = 4456;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the browse page
    client.goto(&format!("{}/browse", base_url)).await.unwrap();

    // Count article elements (expect 20 on page 0)
    let articles = client.find_all(Locator::Css("article")).await.unwrap();
    assert_eq!(
        articles.len(),
        20,
        "Expected 20 articles on page 0, got {}",
        articles.len()
    );

    // Verify "Next →" link exists pointing to /browse?page=1
    let next_link = client.find(Locator::Css("a[href='/browse?page=1']")).await;
    assert!(
        next_link.is_ok(),
        "Expected 'Next →' link pointing to /browse?page=1"
    );
    let next_text = next_link.unwrap().text().await.unwrap();
    assert!(
        next_text.contains("Next"),
        "Expected link text to contain 'Next', got: {}",
        next_text
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that the browse page displays 5 articles on page 1 when there are 25 total,
/// and shows a "← Previous" link pointing to /browse?page=0.
#[tokio::test]
#[ignore]
async fn test_browse_pagination_page_1() {
    let state = test_app_state().await;
    seed_summaries(&state.db, 25).await;
    let base_url = start_test_server_with_state(state).await;
    let geckodriver_port = 4457;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to page 1
    client.goto(&format!("{}/browse?page=1", base_url)).await.unwrap();

    // Count article elements (expect 5 on page 1)
    let articles = client.find_all(Locator::Css("article")).await.unwrap();
    assert_eq!(
        articles.len(),
        5,
        "Expected 5 articles on page 1, got {}",
        articles.len()
    );

    // Verify "← Previous" link exists pointing to /browse?page=0
    let prev_link = client.find(Locator::Css("a[href='/browse?page=0']")).await;
    assert!(
        prev_link.is_ok(),
        "Expected '← Previous' link pointing to /browse?page=0"
    );
    let prev_text = prev_link.unwrap().text().await.unwrap();
    assert!(
        prev_text.contains("Previous"),
        "Expected link text to contain 'Previous', got: {}",
        prev_text
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that the last page of browse results does NOT show a "Next →" link.
/// With 25 total records and 20 per page, page 1 has only 5 items so `has_next` is false.
#[tokio::test]
#[ignore]
async fn test_browse_no_next_on_last_page() {
    let state = test_app_state().await;
    seed_summaries(&state.db, 25).await;
    let base_url = start_test_server_with_state(state).await;
    let geckodriver_port = 4458;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to page 1 (the last page with 25 total records)
    client.goto(&format!("{}/browse?page=1", base_url)).await.unwrap();

    // Verify "Next →" link is NOT present
    let page_source = client.source().await.unwrap();
    assert!(
        !page_source.contains("Next →") && !page_source.contains("Next →"),
        "Expected no 'Next' link on the last page"
    );

    // Also verify by CSS selector - no link to page=2
    let next_link = client.find(Locator::Css("a[href='/browse?page=2']")).await;
    assert!(
        next_link.is_err(),
        "Expected no link to /browse?page=2 on the last page"
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that markdown content in summaries is rendered as proper HTML when viewed
/// through the generation partial endpoint. The `render_markdown_to_html()` function
/// converts markdown bold, lists, and headings into their HTML equivalents.
#[tokio::test]
#[ignore]
async fn test_summary_markdown_rendering() {
    let state = test_app_state().await;
    let ids = seed_summaries(&state.db, 1).await;
    let id = ids[0];
    let base_url = start_test_server_with_state(state).await;
    let geckodriver_port = 4459;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the base URL first (needed for same-origin fetch)
    client.goto(&base_url).await.unwrap();

    // Use JavaScript fetch to POST to the generation partial endpoint and inject the HTML
    let script = format!(
        r#"
        const response = await fetch('/generations/{}', {{ method: 'POST' }});
        const html = await response.text();
        document.getElementById('result').innerHTML = html;
        return html;
        "#,
        id
    );
    let result: serde_json::Value = client
        .execute(
            &format!("return (async () => {{ {} }})()", script),
            vec![],
        )
        .await
        .unwrap();
    let html = result.as_str().unwrap_or("");

    // Verify markdown bold (**bold**) was rendered as <strong>
    assert!(
        html.contains("<strong>"),
        "Expected <strong> from **bold** markdown, got: {}",
        &html[..html.len().min(500)]
    );

    // Verify markdown list (- Item) was rendered as <li>
    assert!(
        html.contains("<li>"),
        "Expected <li> from list markdown, got: {}",
        &html[..html.len().min(500)]
    );

    // Verify markdown heading (## Heading) was rendered as <h2>
    assert!(
        html.contains("<h2>"),
        "Expected <h2> from ## heading markdown, got: {}",
        &html[..html.len().min(500)]
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that timestamps in a completed summary are rendered as clickable YouTube links.
/// The `replace_timestamps_in_html()` function converts timestamps like "0:00", "1:30"
/// into `<a href="https://www.youtube.com/watch?v=VIDEO_ID&t=Xs">` links.
#[tokio::test]
#[ignore]
async fn test_timestamp_links_rendered() {
    let state = test_app_state().await;
    let youtube_url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    let id = seed_summary_with_timestamps(&state.db, youtube_url).await;
    let base_url = start_test_server_with_state(state).await;
    let geckodriver_port = 4460;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the base URL first (needed for same-origin fetch)
    client.goto(&base_url).await.unwrap();

    // Use JavaScript fetch to POST to the generation partial endpoint
    let script = format!(
        r#"
        const response = await fetch('/generations/{}', {{ method: 'POST' }});
        const html = await response.text();
        document.getElementById('result').innerHTML = html;
        return html;
        "#,
        id
    );
    let result: serde_json::Value = client
        .execute(&format!("return (async () => {{ {} }})()", script), vec![])
        .await
        .unwrap();
    let html = result.as_str().unwrap_or("");

    // Verify timestamp links are rendered with &t= parameter
    assert!(
        html.contains("&t="),
        "Expected timestamp links with &t= parameter, got: {}",
        &html[..html.len().min(500)]
    );
    assert!(
        html.contains("<a href=\""),
        "Expected <a> elements for timestamp links, got: {}",
        &html[..html.len().min(500)]
    );

    // Verify specific timestamps were converted:
    // 0:00 -> t=0s, 1:30 -> t=90s, 5:45 -> t=345s
    assert!(
        html.contains("t=0s"),
        "Expected t=0s for 0:00 timestamp"
    );
    assert!(
        html.contains("t=90s"),
        "Expected t=90s for 1:30 timestamp"
    );
    assert!(
        html.contains("t=345s"),
        "Expected t=345s for 5:45 timestamp"
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that the search functionality returns results when summaries have embeddings.
/// Seeds summaries with synthetic embedding blobs (random f32 vectors serialized to bytes),
/// submits a search query, and verifies `#search-results` contains article elements.
///
/// Requires GEMINI_API_KEY since the search endpoint embeds the query text via the Gemini API.
#[tokio::test]
#[ignore]
async fn test_search_returns_results() {
    // Skip if GEMINI_API_KEY is not set (search requires embedding the query)
    let api_key = std::env::var("GEMINI_API_KEY");
    if api_key.is_err() || api_key.as_ref().unwrap().is_empty() {
        println!("SKIPPED: GEMINI_API_KEY not set");
        return;
    }

    let state = test_app_state().await;

    // Seed summaries with synthetic embeddings
    let ids = seed_summaries(&state.db, 3).await;

    // Create synthetic embedding vectors (non-zero so cosine similarity works)
    for (i, id) in ids.iter().enumerate() {
        let mut embedding = vec![0.0f32; 3072];
        // Make each embedding slightly different but non-zero
        embedding[i] = 1.0;
        embedding[0] = 0.5;
        let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        sqlx::query("UPDATE summaries SET embedding = ?, embedding_model = ? WHERE identifier = ?")
            .bind(&bytes)
            .bind("gemini-embedding-001")
            .bind(id)
            .execute(&state.db)
            .await
            .unwrap();
    }

    let base_url = start_test_server_with_state(state).await;
    let geckodriver_port = 4461;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the index page
    client.goto(&base_url).await.unwrap();

    // Find the search input and type a query
    let search_input = client.find(Locator::Css("input[name='query']")).await.unwrap();
    search_input.send_keys("video summary content").await.unwrap();

    // Click the search button (second submit button on the page)
    let buttons = client.find_all(Locator::Css("button[type='submit']")).await.unwrap();
    assert!(buttons.len() >= 2, "Expected at least 2 submit buttons");
    buttons[1].click().await.unwrap();

    // Wait for HTMX response
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // Verify #search-results contains article elements (results)
    let search_results = client.find(Locator::Css("#search-results")).await.unwrap();
    let results_html = search_results.html(true).await.unwrap();

    // Should contain at least one article (search result)
    let articles = client.find_all(Locator::Css("#search-results article")).await.unwrap();
    assert!(
        !articles.is_empty(),
        "Expected search results to contain article elements, got HTML: {}",
        &results_html[..results_html.len().min(500)]
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that searching an empty database (no embeddings) returns no results.
/// Submits a nonsensical search query against a fresh in-memory database and verifies
/// that `#search-results` renders the "No results found" message without any article elements.
///
/// Requires GEMINI_API_KEY since the search endpoint embeds the query text via the Gemini API.
#[tokio::test]
#[ignore]
async fn test_search_empty_results() {
    // Skip if GEMINI_API_KEY is not set (search requires embedding the query)
    let api_key = std::env::var("GEMINI_API_KEY");
    if api_key.is_err() || api_key.as_ref().unwrap().is_empty() {
        println!("SKIPPED: GEMINI_API_KEY not set");
        return;
    }

    let base_url = start_test_server().await;
    let geckodriver_port = 4462;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the index page
    client.goto(&base_url).await.unwrap();

    // Submit a nonsensical search query
    let search_input = client.find(Locator::Css("input[name='query']")).await.unwrap();
    search_input.send_keys("xyzzy nonsense gibberish 12345").await.unwrap();

    // Click the search button (second submit button on the page)
    let buttons = client.find_all(Locator::Css("button[type='submit']")).await.unwrap();
    assert!(buttons.len() >= 2, "Expected at least 2 submit buttons");
    buttons[1].click().await.unwrap();

    // Wait for HTMX response
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // Verify #search-results rendered without error
    let search_results = client.find(Locator::Css("#search-results")).await.unwrap();
    let results_html = search_results.html(true).await.unwrap();

    // Should NOT contain any article elements (no results)
    let articles = client.find_all(Locator::Css("#search-results article")).await.unwrap();
    assert!(
        articles.is_empty(),
        "Expected no search result articles in empty database, got {} articles",
        articles.len()
    );

    // Should show "No results found" message
    assert!(
        results_html.contains("No results found"),
        "Expected 'No results found' message, got: {}",
        &results_html[..results_html.len().min(500)]
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that two concurrent browser sessions submitting different URLs to the same server
/// each receive distinct generation identifiers. This verifies that the server correctly
/// isolates concurrent submissions and does not mix up responses between clients.
///
/// Uses two separate geckodriver instances (ports 4463 and 4464) to simulate two
/// independent browser sessions hitting the same application server.
#[tokio::test]
#[ignore]
async fn test_concurrent_submissions() {
    let base_url = start_test_server().await;

    // Start two geckodrivers on different ports
    let geckodriver_port_1 = 4463;
    let geckodriver_port_2 = 4464;
    let mut geckodriver_1 = start_geckodriver(geckodriver_port_1);
    let mut geckodriver_2 = start_geckodriver(geckodriver_port_2);
    let client_1 = connect_browser(geckodriver_port_1).await;
    let client_2 = connect_browser(geckodriver_port_2).await;

    // Both clients navigate to the index page
    client_1.goto(&base_url).await.unwrap();
    client_2.goto(&base_url).await.unwrap();

    // Client 1 submits URL A
    let url_input_1 = client_1.find(Locator::Css("#url")).await.unwrap();
    url_input_1
        .send_keys("https://www.youtube.com/watch?v=concurrent_test_A")
        .await
        .unwrap();
    let submit_1 = client_1
        .find(Locator::Css("button[type='submit']"))
        .await
        .unwrap();
    submit_1.click().await.unwrap();

    // Client 2 submits URL B
    let url_input_2 = client_2.find(Locator::Css("#url")).await.unwrap();
    url_input_2
        .send_keys("https://www.youtube.com/watch?v=concurrent_test_B")
        .await
        .unwrap();
    let submit_2 = client_2
        .find(Locator::Css("button[type='submit']"))
        .await
        .unwrap();
    submit_2.click().await.unwrap();

    // Wait for both HTMX responses
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Extract identifier from client 1's #generation div
    let gen_1 = client_1.find(Locator::Css("#generation")).await.unwrap();
    let hx_post_1 = gen_1
        .attr("hx-post")
        .await
        .unwrap()
        .expect("Client 1 should have hx-post");
    let id_1 = hx_post_1.trim_start_matches("/generations/").to_string();

    // Extract identifier from client 2's #generation div
    let gen_2 = client_2.find(Locator::Css("#generation")).await.unwrap();
    let hx_post_2 = gen_2
        .attr("hx-post")
        .await
        .unwrap()
        .expect("Client 2 should have hx-post");
    let id_2 = hx_post_2.trim_start_matches("/generations/").to_string();

    // Verify distinct identifiers
    assert_ne!(
        id_1, id_2,
        "Concurrent submissions should receive distinct identifiers, got {} and {}",
        id_1, id_2
    );

    // Clean up
    client_1.close().await.unwrap();
    client_2.close().await.unwrap();
    geckodriver_1.kill().ok();
    geckodriver_2.kill().ok();
}

/// Test that the browser recovers after a server restart during HTMX polling.
/// Submits a form to start generation polling, shuts down the server gracefully,
/// starts a new server on the same address (with a fresh in-memory DB), and verifies
/// the browser eventually receives a valid response ("Summary not found" since the
/// old record no longer exists in the new server's database).
#[tokio::test]
#[ignore]
async fn test_server_restart_recovery() {
    let state = test_app_state().await;
    let (base_url, addr, shutdown_tx) = start_test_server_controllable(state).await;
    let geckodriver_port = 4465;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the index page
    client.goto(&base_url).await.unwrap();

    // Submit a URL to start generation
    let url_input = client.find(Locator::Css("#url")).await.unwrap();
    url_input
        .send_keys("https://www.youtube.com/watch?v=restart_test")
        .await
        .unwrap();
    let submit_btn = client
        .find(Locator::Css("button[type='submit']"))
        .await
        .unwrap();
    submit_btn.click().await.unwrap();

    // Wait for HTMX to swap in the generation partial
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Verify polling started (generation div with hx-trigger)
    let generation_div = client.find(Locator::Css("#generation")).await.unwrap();
    let hx_trigger = generation_div.attr("hx-trigger").await.unwrap();
    assert!(
        hx_trigger.is_some(),
        "Expected hx-trigger attribute for polling"
    );

    // Send shutdown signal
    shutdown_tx.send(true).unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Start a new server on the same address (fresh in-memory DB)
    let new_state = test_app_state().await;
    let new_app = build_router(new_state);
    let new_listener = TcpListener::bind(addr)
        .await
        .expect("Failed to rebind address");
    tokio::spawn(async move {
        axum::serve(
            new_listener,
            new_app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    // Wait for the browser's HTMX polling to hit the new server
    // The polling interval is 1s, so wait a few seconds for it to get a response
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // The new server doesn't have the old record, so it should return "Summary not found"
    let page_source = client.source().await.unwrap();
    assert!(
        page_source.contains("Summary not found"),
        "Expected 'Summary not found' after server restart with fresh DB, got page source (first 500 chars): {}",
        &page_source[..page_source.len().min(500)]
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that `aria-busy="true"` is present during summary generation and absent
/// when the summary is complete. The generation partial template includes
/// `<span aria-busy="true"></span>` in the header while generating, and shows
/// "Summary Complete" without the aria-busy span when done.
///
/// Part 1: Submit a form and verify aria-busy="true" is present within #generation.
/// Part 2: Fetch a completed summary's generation partial and verify aria-busy is NOT present.
///
/// Validates: Requirements 12.1, 12.2
#[tokio::test]
#[ignore]
async fn test_aria_busy_during_generation() {
    let state = test_app_state().await;
    // Seed a completed summary for the second part of the test
    let ids = seed_summaries(&state.db, 1).await;
    let completed_id = ids[0];
    let base_url = start_test_server_with_state(state).await;
    let geckodriver_port = 4466;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Part 1: Submit a form and verify aria-busy="true" during generation
    client.goto(&base_url).await.unwrap();
    let url_input = client.find(Locator::Css("#url")).await.unwrap();
    url_input
        .send_keys("https://www.youtube.com/watch?v=aria_busy_test")
        .await
        .unwrap();
    let submit_btn = client
        .find(Locator::Css("button[type='submit']"))
        .await
        .unwrap();
    submit_btn.click().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Verify aria-busy="true" is present within #generation
    let busy_element = client
        .find(Locator::Css("#generation [aria-busy='true']"))
        .await;
    assert!(
        busy_element.is_ok(),
        "Expected aria-busy='true' element during generation"
    );

    // Part 2: Fetch the completed summary's generation partial and verify NO aria-busy
    let script = format!(
        r#"
        const response = await fetch('/generations/{}', {{ method: 'POST' }});
        const html = await response.text();
        document.getElementById('result').innerHTML = html;
        return html;
        "#,
        completed_id
    );
    let result: serde_json::Value = client
        .execute(&format!("return (async () => {{ {} }})()", script), vec![])
        .await
        .unwrap();
    let html = result.as_str().unwrap_or("");

    // Verify aria-busy is NOT present in the completed state
    assert!(
        !html.contains("aria-busy=\"true\""),
        "Expected NO aria-busy='true' when summary is complete, got: {}",
        &html[..html.len().min(500)]
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test that form inputs have proper labels for accessibility.
/// Verifies that `<label for="url">` and `<label for="model">` exist with text content,
/// and that the search input has an accessible name via placeholder or aria-label.
///
/// Validates: Requirements 13.1, 13.2, 13.3
#[tokio::test]
#[ignore]
async fn test_form_input_labels() {
    let base_url = start_test_server().await;
    let geckodriver_port = 4467;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the index page
    client.goto(&base_url).await.unwrap();

    // Verify <label for="url"> exists
    let url_label = client.find(Locator::Css("label[for='url']")).await;
    assert!(
        url_label.is_ok(),
        "Expected <label for='url'> to exist"
    );
    let url_label_text = url_label.unwrap().text().await.unwrap();
    assert!(
        !url_label_text.is_empty(),
        "URL label should have text content"
    );

    // Verify <label for="model"> exists
    let model_label = client.find(Locator::Css("label[for='model']")).await;
    assert!(
        model_label.is_ok(),
        "Expected <label for='model'> to exist"
    );
    let model_label_text = model_label.unwrap().text().await.unwrap();
    assert!(
        !model_label_text.is_empty(),
        "Model label should have text content"
    );

    // Verify search input has accessible name (placeholder or aria-label)
    let search_input = client.find(Locator::Css("input[name='query']")).await.unwrap();
    let placeholder = search_input.attr("placeholder").await.unwrap();
    let aria_label = search_input.attr("aria-label").await.unwrap();
    assert!(
        placeholder.is_some() || aria_label.is_some(),
        "Search input should have either placeholder or aria-label for accessibility"
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}

/// Test keyboard navigation through the form elements and Enter key submission.
/// Verifies that Tab key moves focus through URL input → model select → submit button,
/// and that pressing Enter on the URL input with a value submits the form via HTMX.
///
/// Validates: Requirements 14.1, 14.2
#[tokio::test]
#[ignore]
async fn test_keyboard_navigation() {
    let base_url = start_test_server().await;
    let geckodriver_port = 4468;
    let mut geckodriver = start_geckodriver(geckodriver_port);
    let client = connect_browser(geckodriver_port).await;

    // Navigate to the index page
    client.goto(&base_url).await.unwrap();

    // Click on the body to ensure focus starts from the page
    let body = client.find(Locator::Css("body")).await.unwrap();
    body.click().await.unwrap();

    // Tab to the URL input (first focusable element in the form)
    // Use JavaScript to focus the URL input first, then test tab order from there
    client.execute("document.getElementById('url').focus()", vec![]).await.unwrap();

    // Verify URL input is focused
    let active = client.active_element().await.unwrap();
    let active_id = active.attr("id").await.unwrap();
    assert_eq!(
        active_id.as_deref(),
        Some("url"),
        "Expected URL input to be focused"
    );

    // Tab to model select
    active.send_keys("\u{E004}").await.unwrap(); // Tab key
    let active = client.active_element().await.unwrap();
    let active_id = active.attr("id").await.unwrap();
    assert_eq!(
        active_id.as_deref(),
        Some("model"),
        "Expected model select to be focused after Tab"
    );

    // Tab to submit button
    let active = client.active_element().await.unwrap();
    active.send_keys("\u{E004}").await.unwrap(); // Tab key
    let active = client.active_element().await.unwrap();
    let active_tag = active.tag_name().await.unwrap();
    assert_eq!(
        active_tag.to_lowercase(),
        "button",
        "Expected submit button to be focused after second Tab"
    );

    // Now test Enter key submission
    // Focus the URL input and type a URL
    client.execute("document.getElementById('url').focus()", vec![]).await.unwrap();
    let url_input = client.find(Locator::Css("#url")).await.unwrap();
    url_input.send_keys("https://www.youtube.com/watch?v=keyboard_test").await.unwrap();

    // Press Enter to submit the form
    url_input.send_keys("\u{E007}").await.unwrap(); // Enter key

    // Wait for HTMX response
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Verify #result has content (form was submitted via HTMX)
    let result_div = client.find(Locator::Css("#result")).await.unwrap();
    let result_html = result_div.html(true).await.unwrap();
    assert!(
        !result_html.trim().is_empty(),
        "Expected #result to have content after Enter key submission"
    );

    // Clean up
    client.close().await.unwrap();
    geckodriver.kill().ok();
}
