use std::path::PathBuf;
use tokio::process::Command;
use tracing;

use crate::errors::TranscriptError;
use crate::utils::vtt_parser::parse_vtt;

/// Preferred base languages in priority order for subtitle selection.
const PREFERRED_BASES: &[&str] = &["en", "de", "fr", "es", "pt", "it", "nl", "ja", "ko", "zh"];

pub struct TranscriptService {
    temp_dir: PathBuf,
}

impl TranscriptService {
    pub fn new(temp_dir: &str) -> Self {
        Self {
            temp_dir: PathBuf::from(temp_dir),
        }
    }

    /// Downloads and parses a transcript for the given YouTube URL.
    ///
    /// Steps:
    /// 1. List available subtitles via yt-dlp --list-subs
    /// 2. Pick the best language using priority ordering
    /// 3. Download the VTT subtitle file
    /// 4. Parse the VTT into plain text
    /// 5. Clean up temp files
    pub async fn download_transcript(
        &self,
        url: &str,
        identifier: i64,
    ) -> Result<String, TranscriptError> {
        // Step 1: List available subtitles
        let list_output = self.list_subtitles(url).await?;

        // Step 2: Pick best language
        let lang = self
            .pick_best_language(&list_output)
            .ok_or(TranscriptError::NoSubtitles)?;

        tracing::info!(
            identifier = identifier,
            lang = %lang,
            "Selected subtitle language"
        );

        // Step 3: Download VTT file
        let output_template = self
            .temp_dir
            .join(format!("transcript_{}", identifier))
            .to_string_lossy()
            .to_string();

        let download_result = self
            .download_subtitles(url, &lang, &output_template)
            .await;

        // Find the downloaded VTT file (yt-dlp appends lang and extension)
        let vtt_path = self.find_vtt_file(identifier);

        // Ensure cleanup happens regardless of download result
        let _cleanup_guard = TempFileGuard { paths: vtt_path.clone() };

        // Check download result after setting up cleanup
        download_result?;

        // Step 4: Read and parse VTT
        let vtt_path = vtt_path
            .into_iter()
            .find(|p| p.exists())
            .ok_or_else(|| {
                TranscriptError::YtDlpFailed("VTT file not found after download".to_string())
            })?;

        let vtt_content = tokio::fs::read_to_string(&vtt_path)
            .await
            .map_err(|e| TranscriptError::ParseError(format!("Failed to read VTT file: {}", e)))?;

        let transcript = parse_vtt(&vtt_content);

        if transcript.trim().is_empty() {
            return Err(TranscriptError::ParseError(
                "Parsed transcript is empty".to_string(),
            ));
        }

        Ok(transcript)
    }

    /// Invokes yt-dlp --list-subs to get available subtitle languages.
    async fn list_subtitles(&self, url: &str) -> Result<String, TranscriptError> {
        let output = Command::new("uvx")
            .args(["yt-dlp", "--cookies-from-browser", "firefox", "--list-subs", url])
            .output()
            .await
            .map_err(|e| TranscriptError::YtDlpFailed(format!("Failed to execute yt-dlp: {}", e)))?;

        // yt-dlp may exit with non-zero but still produce useful output on stderr/stdout
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Combine stdout and stderr since yt-dlp may output subtitle info to either
        let combined = format!("{}\n{}", stdout, stderr);

        if combined.trim().is_empty() {
            return Err(TranscriptError::YtDlpFailed(
                "yt-dlp produced no output".to_string(),
            ));
        }

        // Check for known error patterns that indicate failure (not just missing subs)
        if !output.status.success() {
            // Check if it's a bot/rate-limit issue vs genuinely no subtitles
            if combined.contains("Sign in to confirm") || combined.contains("bot") {
                return Err(TranscriptError::YtDlpFailed(
                    "YouTube requires authentication. Try again later or use --cookies.".to_string(),
                ));
            }
            if combined.contains("429") || combined.contains("Too Many Requests") {
                return Err(TranscriptError::YtDlpFailed(
                    "YouTube rate limited (429 Too Many Requests). Try again later.".to_string(),
                ));
            }
            // If it failed but has subtitle info in the output, continue parsing
            if !combined.contains("Available subtitles") && !combined.contains("Available automatic captions") {
                return Err(TranscriptError::YtDlpFailed(
                    format!("yt-dlp failed: {}", stderr.trim()),
                ));
            }
        }

        Ok(combined)
    }

    /// Downloads subtitles in VTT format for the specified language.
    async fn download_subtitles(
        &self,
        url: &str,
        lang: &str,
        output_template: &str,
    ) -> Result<(), TranscriptError> {
        let output = Command::new("uvx")
            .args([
                "yt-dlp",
                "--cookies-from-browser",
                "firefox",
                "--write-sub",
                "--write-auto-sub",
                "--sub-lang",
                lang,
                "--sub-format",
                "vtt",
                "--skip-download",
                "--format",
                "mhtml",
                "-o",
                output_template,
                url,
            ])
            .output()
            .await
            .map_err(|e| {
                TranscriptError::YtDlpFailed(format!("Failed to execute yt-dlp download: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(TranscriptError::YtDlpFailed(format!(
                "yt-dlp subtitle download failed: {}",
                stderr.trim()
            )));
        }

        Ok(())
    }

    /// Finds potential VTT file paths for the given identifier.
    /// yt-dlp creates files like: transcript_<id>.<lang>.vtt
    fn find_vtt_file(&self, identifier: i64) -> Vec<PathBuf> {
        let prefix = format!("transcript_{}", identifier);
        let mut paths = Vec::new();

        // Try to find matching files in temp_dir
        if let Ok(entries) = std::fs::read_dir(&self.temp_dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with(&prefix) && file_name.ends_with(".vtt") {
                    paths.push(entry.path());
                }
            }
        }

        paths
    }

    /// Selects the best subtitle language from yt-dlp --list-subs output.
    ///
    /// Priority ordering:
    /// 1. `-orig` languages matching preferred base order (en, de, fr, es, pt, it, nl, ja, ko, zh)
    /// 2. Any `-orig` language (sorted alphabetically)
    /// 3. Non-orig languages matching preferred base order
    /// 4. Any language with `en` prefix
    /// 5. First language sorted alphabetically
    pub fn pick_best_language(&self, list_output: &str) -> Option<String> {
        let languages = self.parse_language_codes(list_output);

        if languages.is_empty() {
            return None;
        }

        // Category 1: -orig languages matching preferred base order
        for base in PREFERRED_BASES {
            let orig_code = format!("{}-orig", base);
            if languages.contains(&orig_code) {
                return Some(orig_code);
            }
        }

        // Category 2: Any -orig language (sorted)
        let mut orig_langs: Vec<&String> = languages
            .iter()
            .filter(|l| l.ends_with("-orig"))
            .collect();
        orig_langs.sort();
        if let Some(lang) = orig_langs.first() {
            return Some((*lang).clone());
        }

        // Category 3: Non-orig matching preferred base order
        for base in PREFERRED_BASES {
            let base_str = base.to_string();
            if languages.contains(&base_str) {
                return Some(base_str);
            }
        }

        // Category 4: Any en* prefix
        let mut en_langs: Vec<&String> = languages
            .iter()
            .filter(|l| l.starts_with("en"))
            .collect();
        en_langs.sort();
        if let Some(lang) = en_langs.first() {
            return Some((*lang).clone());
        }

        // Category 5: First sorted language
        let mut sorted: Vec<&String> = languages.iter().collect();
        sorted.sort();
        sorted.first().map(|l| (*l).clone())
    }

    /// Parses language codes from yt-dlp --list-subs output.
    ///
    /// The output format looks like:
    /// ```text
    /// [info] Available subtitles for VIDEO_ID:
    /// Language Name                     Formats
    /// en       English                  vtt, ...
    /// en-orig  English (Original)      vtt, ...
    /// de       German                  vtt, ...
    /// ```
    ///
    /// We look for lines that have at least 2 whitespace-separated columns
    /// and where the first column looks like a language code (lowercase letters,
    /// digits, hyphens).
    fn parse_language_codes(&self, list_output: &str) -> Vec<String> {
        let mut languages = Vec::new();
        let mut in_subtitle_section = false;

        for line in list_output.lines() {
            let trimmed = line.trim();

            // Detect the start of a subtitle listing section
            if trimmed.contains("Available subtitles")
                || trimmed.contains("Available automatic captions")
            {
                in_subtitle_section = false; // Reset, next line is header
                continue;
            }

            // Skip the header line (Language Name Formats)
            if trimmed.starts_with("Language") && trimmed.contains("Formats") {
                in_subtitle_section = true;
                continue;
            }

            if !in_subtitle_section {
                continue;
            }

            // Empty line ends the section
            if trimmed.is_empty() {
                in_subtitle_section = false;
                continue;
            }

            // Parse language code from the first column
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                let code = parts[0];
                // Validate it looks like a language code
                if is_language_code(code) {
                    languages.push(code.to_string());
                }
            }
        }

        languages
    }
}

/// Checks if a string looks like a valid language code.
/// Language codes are letters, digits, and hyphens (e.g., "en", "en-orig", "zh-Hans", "en-US").
fn is_language_code(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphabetic() || c.is_ascii_digit() || c == '-')
        && s.chars().next().map_or(false, |c| c.is_ascii_lowercase())
}

/// RAII guard that cleans up temporary files when dropped.
struct TempFileGuard {
    paths: Vec<PathBuf>,
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        for path in &self.paths {
            if path.exists() {
                if let Err(e) = std::fs::remove_file(path) {
                    tracing::warn!(path = %path.display(), error = %e, "Failed to clean up temp file");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pick_best_language_orig_preferred() {
        let svc = TranscriptService::new("/dev/shm");
        let output = r#"[info] Available subtitles for VIDEO_ID:
Language Name                     Formats
de       German                  vtt, ttml, srv3, srv2, srv1, json3
en-orig  English (Original)      vtt, ttml, srv3, srv2, srv1, json3
fr       French                  vtt, ttml, srv3, srv2, srv1, json3
"#;
        assert_eq!(svc.pick_best_language(output), Some("en-orig".to_string()));
    }

    #[test]
    fn test_pick_best_language_de_orig_over_non_orig() {
        let svc = TranscriptService::new("/dev/shm");
        let output = r#"[info] Available subtitles for VIDEO_ID:
Language Name                     Formats
en       English                  vtt, ttml, srv3, srv2, srv1, json3
de-orig  German (Original)       vtt, ttml, srv3, srv2, srv1, json3
fr       French                  vtt, ttml, srv3, srv2, srv1, json3
"#;
        // de-orig is preferred base + orig, so it wins over en (non-orig preferred base)
        assert_eq!(svc.pick_best_language(output), Some("de-orig".to_string()));
    }

    #[test]
    fn test_pick_best_language_non_orig_preferred_base() {
        let svc = TranscriptService::new("/dev/shm");
        let output = r#"[info] Available subtitles for VIDEO_ID:
Language Name                     Formats
fr       French                  vtt, ttml, srv3, srv2, srv1, json3
de       German                  vtt, ttml, srv3, srv2, srv1, json3
ja       Japanese                vtt, ttml, srv3, srv2, srv1, json3
"#;
        // No -orig languages, so pick first preferred base match: de (index 1) before fr (index 2)
        // Actually: preferred order is en, de, fr, es, pt, it, nl, ja, ko, zh
        // de is at index 1, fr is at index 2 → de wins
        assert_eq!(svc.pick_best_language(output), Some("de".to_string()));
    }

    #[test]
    fn test_pick_best_language_en_prefix_fallback() {
        let svc = TranscriptService::new("/dev/shm");
        let output = r#"[info] Available subtitles for VIDEO_ID:
Language Name                     Formats
en-US    English (US)            vtt, ttml, srv3, srv2, srv1, json3
ru       Russian                 vtt, ttml, srv3, srv2, srv1, json3
"#;
        // en-US is not in preferred bases exactly, but matches en* prefix
        // Actually "en" is in preferred bases, but "en-US" is not exactly "en"
        // Category 3 checks exact match, so "en-US" != "en"
        // Category 4: en* prefix → en-US
        assert_eq!(svc.pick_best_language(output), Some("en-US".to_string()));
    }

    #[test]
    fn test_pick_best_language_first_sorted_fallback() {
        let svc = TranscriptService::new("/dev/shm");
        let output = r#"[info] Available subtitles for VIDEO_ID:
Language Name                     Formats
ru       Russian                 vtt, ttml, srv3, srv2, srv1, json3
ar       Arabic                  vtt, ttml, srv3, srv2, srv1, json3
"#;
        // No orig, no preferred base, no en* → first sorted: ar
        assert_eq!(svc.pick_best_language(output), Some("ar".to_string()));
    }

    #[test]
    fn test_pick_best_language_no_subtitles() {
        let svc = TranscriptService::new("/dev/shm");
        let output = "[info] No subtitles available\n";
        assert_eq!(svc.pick_best_language(output), None);
    }

    #[test]
    fn test_pick_best_language_empty_output() {
        let svc = TranscriptService::new("/dev/shm");
        assert_eq!(svc.pick_best_language(""), None);
    }

    #[test]
    fn test_pick_best_language_any_orig_sorted() {
        let svc = TranscriptService::new("/dev/shm");
        let output = r#"[info] Available subtitles for VIDEO_ID:
Language Name                     Formats
xx-orig  Unknown (Original)      vtt, ttml, srv3, srv2, srv1, json3
ab-orig  Another (Original)      vtt, ttml, srv3, srv2, srv1, json3
ru       Russian                 vtt, ttml, srv3, srv2, srv1, json3
"#;
        // No preferred base -orig, so pick any -orig sorted: ab-orig < xx-orig
        assert_eq!(svc.pick_best_language(output), Some("ab-orig".to_string()));
    }

    #[test]
    fn test_is_language_code() {
        assert!(is_language_code("en"));
        assert!(is_language_code("en-orig"));
        assert!(is_language_code("zh-hans"));
        assert!(is_language_code("en-us"));
        assert!(is_language_code("en-US"));
        assert!(is_language_code("zh-Hans"));
        assert!(!is_language_code(""));
        assert!(!is_language_code("123"));
        assert!(!is_language_code("-en"));
        assert!(!is_language_code("Language")); // starts with uppercase
    }

    #[test]
    fn test_parse_language_codes() {
        let svc = TranscriptService::new("/dev/shm");
        let output = r#"[info] Available subtitles for VIDEO_ID:
Language Name                     Formats
en       English                  vtt, ttml, srv3, srv2, srv1, json3
en-orig  English (Original)      vtt, ttml, srv3, srv2, srv1, json3
de       German                  vtt, ttml, srv3, srv2, srv1, json3
"#;
        let codes = svc.parse_language_codes(output);
        assert_eq!(codes, vec!["en", "en-orig", "de"]);
    }

    #[test]
    fn test_parse_language_codes_with_auto_captions() {
        let svc = TranscriptService::new("/dev/shm");
        let output = r#"[info] Available automatic captions for VIDEO_ID:
Language Name                     Formats
en       English                  vtt, ttml, srv3, srv2, srv1, json3
de       German                  vtt, ttml, srv3, srv2, srv1, json3

[info] Available subtitles for VIDEO_ID:
Language Name                     Formats
en-orig  English (Original)      vtt, ttml, srv3, srv2, srv1, json3
"#;
        let codes = svc.parse_language_codes(output);
        // Should capture from both sections
        assert!(codes.contains(&"en".to_string()));
        assert!(codes.contains(&"de".to_string()));
        assert!(codes.contains(&"en-orig".to_string()));
    }
}
