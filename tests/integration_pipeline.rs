//! Integration test for the full summarization pipeline.
//! Requires network access, `uvx yt-dlp`, and GEMINI_API_KEY env var.
//!
//! Run with: cargo test --test integration_pipeline -- --ignored
//! (These tests are ignored by default since they require network + API key)

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use rs_summarizer::db;
use rs_summarizer::services::embedding::EmbeddingService;
use rs_summarizer::services::summary::SummaryService;
use rs_summarizer::services::transcript::TranscriptService;
use rs_summarizer::state::{AppState, ModelOption};
use rs_summarizer::tasks;
use rs_summarizer::utils::markdown_converter::convert_markdown_to_youtube_format;

fn get_api_key() -> String {
    std::env::var("GEMINI_API_KEY").expect(
        "GEMINI_API_KEY must be set to run integration tests. \
         Use: GEMINI_API_KEY=$(cat ~/api_key.txt) cargo test --test integration_pipeline -- --ignored"
    )
}

fn test_model() -> ModelOption {
    ModelOption {
        name: "gemma-3-27b-it".to_string(),
        input_price_per_mtoken: 0.0,
        output_price_per_mtoken: 0.0,
        context_window: 128_000,
        rpm_limit: 30,
        rpd_limit: 14400,
    }
}

/// Test transcript download for a known video with auto-captions.
#[tokio::test]
#[ignore]
async fn test_transcript_download() {
    let svc = TranscriptService::new("/dev/shm");
    let result = svc
        .download_transcript("https://www.youtube.com/watch?v=LlzXCE02swU", 99999)
        .await;

    match result {
        Ok(transcript) => {
            assert!(!transcript.is_empty(), "Transcript should not be empty");
            assert!(transcript.len() > 100, "Transcript too short: {} chars", transcript.len());
            // Should have timestamp format HH:MM:SS
            assert!(transcript.contains("00:00:"), "Transcript should contain timestamps");
            println!("Transcript length: {} chars, first 200:\n{}", transcript.len(), &transcript[..transcript.len().min(200)]);
        }
        Err(e) => {
            // If we get rate-limited, that's acceptable in CI
            let err_str = e.to_string();
            if err_str.contains("429") || err_str.contains("bot") || err_str.contains("authentication") {
                println!("SKIPPED: YouTube rate-limited us: {}", err_str);
                return;
            }
            panic!("Transcript download failed: {}", e);
        }
    }
}

/// Test summary generation with a short transcript.
#[tokio::test]
#[ignore]
async fn test_summary_generation() {
    let api_key = get_api_key();
    let svc = SummaryService::new(api_key);
    let model = test_model();

    // Use a short but valid transcript (>30 words)
    let transcript = "00:00:00 Welcome to this video about Rust programming \
        00:00:05 Today we will learn about ownership and borrowing \
        00:00:10 Rust's ownership system is what makes it unique among programming languages \
        00:00:15 Every value in Rust has a single owner at any given time \
        00:00:20 When the owner goes out of scope the value is dropped \
        00:00:25 This prevents memory leaks and data races at compile time \
        00:00:30 Let me show you some examples of how this works in practice";

    // Create an in-memory SQLite database for testing
    let db_pool = db::init_db("sqlite::memory:").await.expect("Failed to init test DB");

    // Insert a test row
    let form = rs_summarizer::models::SubmitForm {
        original_source_link: "https://www.youtube.com/watch?v=test123test".to_string(),
        transcript: Some(transcript.to_string()),
        model: model.name.clone(),
    };
    let id = db::insert_new_summary(&db_pool, &form, "127.0.0.1", "2024-01-01T00:00:00Z")
        .await
        .expect("Failed to insert test row");

    // Generate summary
    let result = svc.generate_summary(&db_pool, id, transcript, &model).await;

    match result {
        Ok(summary_result) => {
            assert!(!summary_result.summary_text.is_empty(), "Summary should not be empty");
            assert!(summary_result.summary_text.len() > 50, "Summary too short");
            assert!(summary_result.cost >= 0.0, "Cost should be non-negative");
            assert!(summary_result.duration_secs > 0.0, "Duration should be positive");
            println!(
                "Summary generated: {} chars, cost: ${:.6}, duration: {:.2}s",
                summary_result.summary_text.len(),
                summary_result.cost,
                summary_result.duration_secs
            );
            println!("First 300 chars:\n{}", &summary_result.summary_text[..summary_result.summary_text.len().min(300)]);

            // Test YouTube format conversion
            let youtube_text = convert_markdown_to_youtube_format(&summary_result.summary_text);
            assert!(!youtube_text.contains("**"), "YouTube format should not contain **");
        }
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("429") || err_str.contains("ResourceExhausted") || err_str.contains("rate") {
                println!("SKIPPED: API rate-limited: {}", err_str);
                return;
            }
            panic!("Summary generation failed: {}", e);
        }
    }
}

/// Test embedding computation.
#[tokio::test]
#[ignore]
async fn test_embedding_computation() {
    let api_key = get_api_key();
    let svc = EmbeddingService::new(api_key, "gemini-embedding-001", 768);

    let text = "This is a test summary about Rust programming and ownership semantics.";

    let result = svc.embed_text(text).await;

    match result {
        Ok(embedding) => {
            assert!(!embedding.is_empty(), "Embedding should not be empty");
            assert_eq!(embedding.len(), 768, "Expected 768 dimensions, got {}", embedding.len());
            // Verify values are reasonable floats
            assert!(embedding.iter().all(|v| v.is_finite()), "All values should be finite");
            // Verify not all zeros
            assert!(embedding.iter().any(|v| *v != 0.0), "Embedding should not be all zeros");
            println!("Embedding computed: {} dimensions, first 5: {:?}", embedding.len(), &embedding[..5]);
        }
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("429") || err_str.contains("ResourceExhausted") || err_str.contains("rate") {
                println!("SKIPPED: API rate-limited: {}", err_str);
                return;
            }
            panic!("Embedding computation failed: {}", e);
        }
    }
}

/// Test cosine similarity computation (no network needed).
#[tokio::test]
#[ignore]
async fn test_cosine_similarity_integration() {
    let a = vec![1.0f32, 0.0, 0.0];
    let b = vec![1.0f32, 0.0, 0.0];
    let sim = EmbeddingService::cosine_similarity(&a, &b);
    assert!((sim - 1.0).abs() < 1e-6, "Identical vectors should have similarity 1.0");

    let c = vec![0.0f32, 1.0, 0.0];
    let sim2 = EmbeddingService::cosine_similarity(&a, &c);
    assert!(sim2.abs() < 1e-6, "Orthogonal vectors should have similarity 0.0");
}

/// Test the full pipeline: transcript → summary → YouTube format → embedding.
#[tokio::test]
#[ignore]
async fn test_full_pipeline_end_to_end() {
    let api_key = get_api_key();

    // Step 1: Download transcript
    let transcript_svc = TranscriptService::new("/dev/shm");
    let transcript = match transcript_svc
        .download_transcript("https://www.youtube.com/watch?v=LlzXCE02swU", 88888)
        .await
    {
        Ok(t) => t,
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("429") || err_str.contains("bot") || err_str.contains("authentication") {
                println!("SKIPPED: YouTube rate-limited: {}", err_str);
                return;
            }
            panic!("Transcript download failed: {}", e);
        }
    };
    println!("Step 1 OK: Transcript downloaded ({} chars)", transcript.len());

    // Step 2: Generate summary
    let summary_svc = SummaryService::new(api_key.clone());
    let model = test_model();
    let db_pool = db::init_db("sqlite::memory:").await.expect("Failed to init DB");

    let form = rs_summarizer::models::SubmitForm {
        original_source_link: "https://www.youtube.com/watch?v=LlzXCE02swU".to_string(),
        transcript: Some(transcript.clone()),
        model: model.name.clone(),
    };
    let id = db::insert_new_summary(&db_pool, &form, "127.0.0.1", "2024-01-01T00:00:00Z")
        .await
        .expect("Failed to insert");

    let summary_result = match summary_svc.generate_summary(&db_pool, id, &transcript, &model).await {
        Ok(r) => r,
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("429") || err_str.contains("ResourceExhausted") {
                println!("SKIPPED: Gemini rate-limited: {}", err_str);
                return;
            }
            panic!("Summary generation failed: {}", e);
        }
    };
    println!("Step 2 OK: Summary generated ({} chars, ${:.6})", summary_result.summary_text.len(), summary_result.cost);

    // Step 3: Convert to YouTube format
    let youtube_text = convert_markdown_to_youtube_format(&summary_result.summary_text);
    assert!(!youtube_text.contains("**"), "YouTube format should not contain **");
    println!("Step 3 OK: YouTube format conversion done");

    // Step 4: Compute embedding
    let embedding_svc = EmbeddingService::new(api_key, "gemini-embedding-001", 768);
    let embedding = match embedding_svc.embed_text(&summary_result.summary_text).await {
        Ok(e) => e,
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("429") || err_str.contains("ResourceExhausted") {
                println!("SKIPPED: Embedding rate-limited: {}", err_str);
                return;
            }
            panic!("Embedding failed: {}", e);
        }
    };
    assert_eq!(embedding.len(), 768);
    println!("Step 4 OK: Embedding computed ({} dimensions)", embedding.len());

    println!("\n=== FULL PIPELINE SUCCESS ===");
}


/// Helper to build an AppState for integration tests with an in-memory DB.
async fn build_test_app_state() -> AppState {
    let api_key = get_api_key();
    let db_pool = db::init_db("sqlite::memory:")
        .await
        .expect("Failed to init test DB");

    let model_options = Arc::new(vec![test_model()]);
    let model_counts = Arc::new(RwLock::new(HashMap::new()));
    let last_reset_day = Arc::new(RwLock::new(None));

    AppState {
        db: db_pool,
        model_options,
        model_counts,
        last_reset_day,
        gemini_api_key: api_key,
    }
}

/// Test that summary_done transitions from false to true after process_summary completes.
/// This verifies the bug fix where the spinner would never disappear.
#[tokio::test]
#[ignore]
async fn test_summary_done_flag_transitions() {
    let app = build_test_app_state().await;

    // Use a short transcript that's valid (>30 words) to keep the test fast
    let transcript = "00:00:00 Welcome to this video about Rust programming \
        00:00:05 Today we will learn about ownership and borrowing \
        00:00:10 Rust's ownership system is what makes it unique among programming languages \
        00:00:15 Every value in Rust has a single owner at any given time \
        00:00:20 When the owner goes out of scope the value is dropped \
        00:00:25 This prevents memory leaks and data races at compile time \
        00:00:30 Let me show you some examples of how this works in practice";

    let form = rs_summarizer::models::SubmitForm {
        original_source_link: "https://www.youtube.com/watch?v=test_done_flag".to_string(),
        transcript: Some(transcript.to_string()),
        model: test_model().name,
    };
    let id = db::insert_new_summary(&app.db, &form, "127.0.0.1", "2024-01-01T00:00:00Z")
        .await
        .expect("Failed to insert");

    // Verify initial state: summary_done should be false
    let row = db::fetch_summary(&app.db, id).await.unwrap().unwrap();
    assert!(!row.summary_done, "summary_done should start as false");
    assert!(row.summary.is_empty(), "summary should start empty");

    // Run the full background task
    tasks::process_summary(app.db.clone(), id, app.clone()).await;

    // Verify final state: summary_done should be true
    let row = db::fetch_summary(&app.db, id).await.unwrap().unwrap();
    assert!(row.summary_done, "summary_done should be true after process_summary completes");
    assert!(!row.summary.is_empty(), "summary should not be empty after processing");
    assert!(row.summary_input_tokens > 0, "input_tokens should be recorded");
    assert!(row.summary_output_tokens > 0, "output_tokens should be recorded");
    assert!(!row.summary_timestamp_end.is_empty(), "timestamp_end should be set");
    assert!(row.cost >= 0.0, "cost should be non-negative");

    println!("summary_done flag correctly transitions: false → true");
    println!("  summary length: {} chars", row.summary.len());
    println!("  input_tokens: {}", row.summary_input_tokens);
    println!("  output_tokens: {}", row.summary_output_tokens);
    println!("  cost: ${:.6}", row.cost);
    println!("  timestamp_end: {}", row.summary_timestamp_end);
}

/// Test that timestamps_done is also set after the full pipeline.
#[tokio::test]
#[ignore]
async fn test_timestamps_done_after_pipeline() {
    let app = build_test_app_state().await;

    let transcript = "00:00:00 Welcome to this video about Rust programming \
        00:00:05 Today we will learn about ownership and borrowing \
        00:00:10 Rust's ownership system is what makes it unique among programming languages \
        00:00:15 Every value in Rust has a single owner at any given time \
        00:00:20 When the owner goes out of scope the value is dropped \
        00:00:25 This prevents memory leaks and data races at compile time \
        00:00:30 Let me show you some examples of how this works in practice";

    let form = rs_summarizer::models::SubmitForm {
        original_source_link: "https://www.youtube.com/watch?v=test_timestamps".to_string(),
        transcript: Some(transcript.to_string()),
        model: test_model().name,
    };
    let id = db::insert_new_summary(&app.db, &form, "127.0.0.1", "2024-01-01T00:00:00Z")
        .await
        .expect("Failed to insert");

    // Run the full pipeline
    tasks::process_summary(app.db.clone(), id, app.clone()).await;

    let row = db::fetch_summary(&app.db, id).await.unwrap().unwrap();
    assert!(row.summary_done, "summary_done should be true");
    assert!(row.timestamps_done, "timestamps_done should be true after pipeline");
    assert!(
        !row.timestamped_summary_in_youtube_format.is_empty(),
        "YouTube format text should be populated"
    );
    // YouTube format should not contain markdown bold markers
    assert!(
        !row.timestamped_summary_in_youtube_format.contains("**"),
        "YouTube format should not contain ** markdown bold"
    );

    println!("timestamps_done correctly set after pipeline");
    println!("  YouTube format length: {} chars", row.timestamped_summary_in_youtube_format.len());
}

/// Test that process_summary handles errors gracefully and still sets summary_done=true.
/// This ensures the frontend stops polling even on failure.
#[tokio::test]
#[ignore]
async fn test_error_sets_summary_done() {
    let app = build_test_app_state().await;

    // Use a transcript that's too short (<30 words) to trigger a validation error
    let short_transcript = "This is too short to summarize.";

    let form = rs_summarizer::models::SubmitForm {
        original_source_link: "https://www.youtube.com/watch?v=test_error".to_string(),
        transcript: Some(short_transcript.to_string()),
        model: test_model().name,
    };
    let id = db::insert_new_summary(&app.db, &form, "127.0.0.1", "2024-01-01T00:00:00Z")
        .await
        .expect("Failed to insert");

    // Run the pipeline — should fail due to short transcript
    tasks::process_summary(app.db.clone(), id, app.clone()).await;

    // Even on error, summary_done should be true so polling stops
    let row = db::fetch_summary(&app.db, id).await.unwrap().unwrap();
    assert!(
        row.summary_done,
        "summary_done should be true even on error (so spinner disappears)"
    );
    // The summary field should contain the error message
    assert!(
        !row.summary.is_empty(),
        "summary should contain error message on failure"
    );

    println!("Error handling correctly sets summary_done=true");
    println!("  Error message in summary: {}", &row.summary[..row.summary.len().min(100)]);
}

/// Test that process_summary handles invalid model name gracefully.
#[tokio::test]
#[ignore]
async fn test_invalid_model_sets_summary_done() {
    let app = build_test_app_state().await;

    let transcript = "00:00:00 Welcome to this video about Rust programming \
        00:00:05 Today we will learn about ownership and borrowing \
        00:00:10 Rust's ownership system is what makes it unique among programming languages \
        00:00:15 Every value in Rust has a single owner at any given time \
        00:00:20 When the owner goes out of scope the value is dropped \
        00:00:25 This prevents memory leaks and data races at compile time \
        00:00:30 Let me show you some examples of how this works in practice";

    let form = rs_summarizer::models::SubmitForm {
        original_source_link: "https://www.youtube.com/watch?v=test_bad_model".to_string(),
        transcript: Some(transcript.to_string()),
        model: "nonexistent-model-xyz".to_string(), // model not in app.model_options
    };
    let id = db::insert_new_summary(&app.db, &form, "127.0.0.1", "2024-01-01T00:00:00Z")
        .await
        .expect("Failed to insert");

    // Run the pipeline — should fail due to unknown model
    tasks::process_summary(app.db.clone(), id, app.clone()).await;

    let row = db::fetch_summary(&app.db, id).await.unwrap().unwrap();
    assert!(
        row.summary_done,
        "summary_done should be true even with invalid model"
    );
    assert!(
        !row.summary.is_empty(),
        "summary should contain error message"
    );

    println!("Invalid model error correctly sets summary_done=true");
    println!("  Error: {}", &row.summary[..row.summary.len().min(100)]);
}

/// Test the full lifecycle as the frontend would see it:
/// submit → poll (summary_done=false) → poll (summary growing) → poll (summary_done=true, stop).
/// This simulates the HTMX polling behavior.
#[tokio::test]
#[ignore]
async fn test_polling_lifecycle_simulation() {
    let app = build_test_app_state().await;

    let transcript = "00:00:00 Welcome to this video about Rust programming \
        00:00:05 Today we will learn about ownership and borrowing \
        00:00:10 Rust's ownership system is what makes it unique among programming languages \
        00:00:15 Every value in Rust has a single owner at any given time \
        00:00:20 When the owner goes out of scope the value is dropped \
        00:00:25 This prevents memory leaks and data races at compile time \
        00:00:30 Let me show you some examples of how this works in practice";

    let form = rs_summarizer::models::SubmitForm {
        original_source_link: "https://www.youtube.com/watch?v=test_polling".to_string(),
        transcript: Some(transcript.to_string()),
        model: test_model().name,
    };
    let id = db::insert_new_summary(&app.db, &form, "127.0.0.1", "2024-01-01T00:00:00Z")
        .await
        .expect("Failed to insert");

    // Simulate what the frontend sees: spawn the task and poll
    let app_clone = app.clone();
    let db_clone = app.db.clone();
    let handle = tokio::spawn(async move {
        tasks::process_summary(db_clone, id, app_clone).await;
    });

    // Poll until summary_done=true (simulating HTMX every 1s polling)
    let mut polls = 0;
    let max_polls = 120; // 2 minutes max
    let mut saw_partial_summary = false;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        polls += 1;

        let row = db::fetch_summary(&app.db, id).await.unwrap().unwrap();

        if !row.summary.is_empty() && !row.summary_done {
            saw_partial_summary = true;
        }

        if row.summary_done {
            println!("Polling lifecycle complete after {} polls", polls);
            println!("  Saw partial summary during streaming: {}", saw_partial_summary);
            println!("  Final summary length: {} chars", row.summary.len());
            println!("  timestamps_done: {}", row.timestamps_done);
            assert!(!row.summary.is_empty(), "Final summary should not be empty");
            break;
        }

        if polls >= max_polls {
            panic!(
                "Polling timed out after {} polls. summary_done never became true! \
                 This is the spinner bug — summary_done is not being set.",
                max_polls
            );
        }
    }

    // Wait for the task to fully complete
    handle.await.expect("Background task panicked");

    // Final verification
    let row = db::fetch_summary(&app.db, id).await.unwrap().unwrap();
    assert!(row.summary_done, "summary_done should be true");
    assert!(row.timestamps_done, "timestamps_done should be true");
    assert!(!row.summary_timestamp_end.is_empty(), "end timestamp should be set");
}
