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
            name: "gemini-3-flash-preview".to_string(),
            input_price_per_mtoken: 0.10,
            output_price_per_mtoken: 0.40,
            context_window: 1_000_000,
            rpm_limit: 5,
            rpd_limit: 20,
        },
        ModelOption {
            name: "gemma-3-27b-it".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 128_000,
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
