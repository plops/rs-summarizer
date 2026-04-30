//! Integration test for the full summarization pipeline.
//! Requires network access, `uvx yt-dlp`, and GEMINI_API_KEY env var.
//!
//! Run with: cargo test --test integration_pipeline -- --ignored
//! (These tests are ignored by default since they require network + API key)

use rs_summarizer::db;
use rs_summarizer::services::embedding::EmbeddingService;
use rs_summarizer::services::summary::SummaryService;
use rs_summarizer::services::transcript::TranscriptService;
use rs_summarizer::state::ModelOption;
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
