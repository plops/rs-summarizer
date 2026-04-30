//! Integration test for the transcript download pipeline.
//! Requires network access and `uvx yt-dlp` to be available.
//!
//! Run with: cargo test --test integration_transcript -- --ignored
//! (These tests are ignored by default since they require network access)

use std::path::PathBuf;
use tokio::process::Command;

/// Test that yt-dlp can list subtitles for a known video.
#[tokio::test]
#[ignore] // requires network
async fn test_list_subtitles_real_video() {
    let output = Command::new("uvx")
        .args(["yt-dlp", "--cookies-from-browser", "firefox", "--list-subs", "https://www.youtube.com/watch?v=LlzXCE02swU"])
        .output()
        .await
        .expect("Failed to run uvx yt-dlp");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    // This video has auto-generated captions including English
    assert!(
        combined.contains("en") && combined.contains("vtt"),
        "Expected English auto-captions with vtt format, got:\n{}",
        &combined[..combined.len().min(500)]
    );
}

/// Test that yt-dlp can download auto-generated subtitles.
#[tokio::test]
#[ignore] // requires network
async fn test_download_auto_subtitles() {
    let temp_dir = PathBuf::from("/dev/shm");
    let output_template = temp_dir.join("integration_test_transcript").to_string_lossy().to_string();

    // Clean up any previous test files
    cleanup_test_files(&temp_dir, "integration_test_transcript").await;

    let output = Command::new("uvx")
        .args([
            "yt-dlp",
            "--cookies-from-browser", "firefox",
            "--write-sub",
            "--write-auto-sub",
            "--sub-lang", "en",
            "--sub-format", "vtt",
            "--skip-download",
            "--format", "mhtml",
            "-o", &output_template,
            "https://www.youtube.com/watch?v=LlzXCE02swU",
        ])
        .output()
        .await
        .expect("Failed to run uvx yt-dlp");

    assert!(output.status.success(), "yt-dlp failed: {}", String::from_utf8_lossy(&output.stderr));

    // Find the downloaded VTT file
    let vtt_path = find_vtt_file(&temp_dir, "integration_test_transcript");
    assert!(!vtt_path.is_empty(), "No VTT file found after download");

    // Read and verify content
    let content = tokio::fs::read_to_string(&vtt_path[0]).await.expect("Failed to read VTT");
    assert!(content.starts_with("WEBVTT"), "File doesn't start with WEBVTT header");
    assert!(content.len() > 1000, "VTT file seems too small: {} bytes", content.len());

    // Clean up
    cleanup_test_files(&temp_dir, "integration_test_transcript").await;
}

/// Test the full transcript service pipeline (download + parse).
#[tokio::test]
#[ignore] // requires network
async fn test_full_transcript_pipeline() {
    // We can't easily import from the main crate in integration tests without
    // making it a library. Instead, replicate the key logic here.
    let temp_dir = PathBuf::from("/dev/shm");
    let output_template = temp_dir.join("pipeline_test_transcript").to_string_lossy().to_string();

    // Clean up
    cleanup_test_files(&temp_dir, "pipeline_test_transcript").await;

    // Step 1: List subs
    let list_output = Command::new("uvx")
        .args(["yt-dlp", "--list-subs", "https://www.youtube.com/watch?v=LlzXCE02swU"])
        .output()
        .await
        .expect("Failed to list subs");

    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&list_output.stdout),
        String::from_utf8_lossy(&list_output.stderr)
    );
    assert!(combined.contains("en"), "No English subtitles found");

    // Step 2: Download
    let dl_output = Command::new("uvx")
        .args([
            "yt-dlp",
            "--cookies-from-browser", "firefox",
            "--write-sub",
            "--write-auto-sub",
            "--sub-lang", "en",
            "--sub-format", "vtt",
            "--skip-download",
            "--format", "mhtml",
            "-o", &output_template,
            "https://www.youtube.com/watch?v=LlzXCE02swU",
        ])
        .output()
        .await
        .expect("Failed to download subs");

    assert!(dl_output.status.success());

    // Step 3: Find and read VTT
    let vtt_files = find_vtt_file(&temp_dir, "pipeline_test_transcript");
    assert!(!vtt_files.is_empty(), "No VTT file found");

    let content = tokio::fs::read_to_string(&vtt_files[0]).await.unwrap();
    assert!(content.contains("-->"), "VTT file has no timing cues");

    // Step 4: Verify it has actual caption content (not just headers)
    let lines: Vec<&str> = content.lines().collect();
    assert!(lines.len() > 10, "VTT file has too few lines: {}", lines.len());

    // Clean up
    cleanup_test_files(&temp_dir, "pipeline_test_transcript").await;
}

fn find_vtt_file(temp_dir: &PathBuf, prefix: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(entries) = std::fs::read_dir(temp_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(prefix) && name.ends_with(".vtt") {
                paths.push(entry.path());
            }
        }
    }
    paths
}

async fn cleanup_test_files(temp_dir: &PathBuf, prefix: &str) {
    if let Ok(entries) = std::fs::read_dir(temp_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(prefix) {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }
}
