use std::net::ToSocketAddrs;
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
        tracing::info!(
            identifier = identifier,
            url = %url,
            "Downloading transcript for video"
        );

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

        let download_result = self.download_subtitles(url, &lang, &output_template).await;

        // Find the downloaded VTT file (yt-dlp appends lang and extension)
        let vtt_path = self.find_vtt_file(identifier);

        // Ensure cleanup happens regardless of download result
        let _cleanup_guard = TempFileGuard {
            paths: vtt_path.clone(),
        };

        // Check download result after setting up cleanup
        download_result?;

        // Step 4: Read and parse VTT
        let vtt_path = vtt_path.into_iter().find(|p| p.exists()).ok_or_else(|| {
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

        let size_bytes = transcript.len();
        let word_count = transcript.split_whitespace().count();
        tracing::info!(
            identifier = identifier,
            url = %url,
            size_bytes = size_bytes,
            word_count = word_count,
            "Downloaded transcript successfully"
        );

        Ok(transcript)
    }

    /// Invokes yt-dlp --list-subs to get available subtitle languages.
    async fn list_subtitles(&self, url: &str) -> Result<String, TranscriptError> {
        let mut args = base_uvx_args();
        args.extend(cookie_args());
        args.extend(extractor_args());
        args.push("--list-subs".to_string());
        args.push(url.to_string());

        let cmd_str = format!("uvx {}", args.join(" "));
        tracing::info!(cmd = %cmd_str, "Executing yt-dlp to list subtitles");

        let output = Command::new("uvx")
            .args(&args)
            .output()
            .await
            .map_err(|e| {
                TranscriptError::YtDlpFailed(format!(
                    "Failed to execute yt-dlp (cmd: `{}`): {}",
                    cmd_str, e
                ))
            })?;

        // yt-dlp may exit with non-zero but still produce useful output on stderr/stdout
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Combine stdout and stderr since yt-dlp may output subtitle info to either
        let combined = format!("{}\n{}", stdout, stderr);

        if combined.trim().is_empty() {
            return Err(TranscriptError::YtDlpFailed(format!(
                "yt-dlp produced no output (cmd: `{}`)",
                cmd_str
            )));
        }

        // Check for known error patterns that indicate failure (not just missing subs)
        if !output.status.success() {
            if (combined.contains("Connection refused") && combined.contains("4416"))
                || (combined.contains("pot:bgutil") && combined.contains("Max retries exceeded"))
            {
                return Err(TranscriptError::YtDlpFailed(format!(
                    "Failed to connect to PO Token Provider on port 4416. Ensure docker container 'ytdlp-pot-provider' is running and POT_PROVIDER_URL is configured correctly. (cmd: `{}`)",
                    cmd_str
                )));
            }
            if combined.contains("The page needs to be reloaded")
                || combined.contains("page needs to be reloaded")
            {
                return Err(TranscriptError::YtDlpFailed(format!(
                    "YouTube session expired ('The page needs to be reloaded'). Please restart Firefox and visit YouTube to refresh cookies, or provide a cookies.txt file. (cmd: `{}`)",
                    cmd_str
                )));
            }
            // Check if it's a bot/rate-limit issue vs genuinely no subtitles
            if combined.contains("Sign in to confirm") || combined.contains("bot") {
                return Err(TranscriptError::YtDlpFailed(format!(
                    "YouTube requires authentication or detected a bot. Please ensure the PO Token Provider is running and refresh Firefox cookies if needed. (cmd: `{}`)",
                    cmd_str
                )));
            }
            if combined.contains("429") || combined.contains("Too Many Requests") {
                return Err(TranscriptError::YtDlpFailed(format!(
                    "YouTube rate limited (429 Too Many Requests). Please restart Firefox to refresh cookies or try again later. (cmd: `{}`)",
                    cmd_str
                )));
            }
            // If it failed but has subtitle info in the output, continue parsing
            if !combined.contains("Available subtitles")
                && !combined.contains("Available automatic captions")
            {
                return Err(TranscriptError::YtDlpFailed(format!(
                    "yt-dlp failed: {} (note: please restart Firefox to refresh cookies if needed) (cmd: `{}`)",
                    stderr.trim(),
                    cmd_str
                )));
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
        let mut args = base_uvx_args();
        args.extend(cookie_args());
        args.extend(extractor_args());
        args.extend([
            "--write-sub".to_string(),
            "--write-auto-sub".to_string(),
            "--sub-lang".to_string(),
            lang.to_string(),
            "--sub-format".to_string(),
            "vtt".to_string(),
            "--skip-download".to_string(),
            "--format".to_string(),
            "mhtml".to_string(),
            "-o".to_string(),
            output_template.to_string(),
            url.to_string(),
        ]);

        let cmd_str = format!("uvx {}", args.join(" "));
        tracing::info!(cmd = %cmd_str, "Executing yt-dlp to download subtitles");

        let output = Command::new("uvx")
            .args(&args)
            .output()
            .await
            .map_err(|e| {
                TranscriptError::YtDlpFailed(format!(
                    "Failed to execute yt-dlp download (cmd: `{}`): {}",
                    cmd_str, e
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let stderr_trimmed = stderr.trim();

            if (stderr_trimmed.contains("Connection refused") && stderr_trimmed.contains("4416"))
                || (stderr_trimmed.contains("pot:bgutil")
                    && stderr_trimmed.contains("Max retries exceeded"))
            {
                return Err(TranscriptError::YtDlpFailed(format!(
                    "Failed to connect to PO Token Provider on port 4416. Ensure docker container 'ytdlp-pot-provider' is running and POT_PROVIDER_URL is configured correctly. (cmd: `{}`)",
                    cmd_str
                )));
            }
            if stderr_trimmed.contains("The page needs to be reloaded")
                || stderr_trimmed.contains("page needs to be reloaded")
            {
                return Err(TranscriptError::YtDlpFailed(format!(
                    "YouTube session expired ('The page needs to be reloaded'). Please restart Firefox and visit YouTube to refresh cookies, then try again. (cmd: `{}`)",
                    cmd_str
                )));
            }
            if stderr_trimmed.contains("Sign in to confirm") || stderr_trimmed.contains("bot") {
                return Err(TranscriptError::YtDlpFailed(format!(
                    "YouTube requires authentication or detected a bot. Please ensure the PO Token Provider is running and refresh Firefox cookies if needed. (cmd: `{}`)",
                    cmd_str
                )));
            }
            if stderr_trimmed.contains("429") || stderr_trimmed.contains("Too Many Requests") {
                return Err(TranscriptError::YtDlpFailed(format!(
                    "YouTube rate limited (429 Too Many Requests). Please restart Firefox to refresh cookies or try again later. (cmd: `{}`)",
                    cmd_str
                )));
            }

            return Err(TranscriptError::YtDlpFailed(format!(
                "yt-dlp subtitle download failed: {} (note: please restart Firefox to refresh cookies if needed) (cmd: `{}`)",
                stderr_trimmed, cmd_str
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
        let mut orig_langs: Vec<&String> =
            languages.iter().filter(|l| l.ends_with("-orig")).collect();
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
        let mut en_langs: Vec<&String> = languages.iter().filter(|l| l.starts_with("en")).collect();
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
        && s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
}

/// Returns the uvx and plugin arguments to run yt-dlp.
/// Uses bgutil-ytdlp-pot-provider by default unless DISABLE_POT_PROVIDER is set to true/1.
fn base_uvx_args() -> Vec<String> {
    let disable_pot = std::env::var("DISABLE_POT_PROVIDER")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if disable_pot {
        vec!["yt-dlp".to_string()]
    } else {
        vec![
            "--with".to_string(),
            "bgutil-ytdlp-pot-provider".to_string(),
            "yt-dlp".to_string(),
        ]
    }
}

/// Resolves the base URL for the PO Token Provider HTTP server.
/// Checks environment variables POT_PROVIDER_URL, BGUTIL_POT_PROVIDER_URL, and YTDLP_POT_PROVIDER_URL.
/// If unset, checks whether `host.docker.internal` resolves (common Docker container setup).
fn resolve_pot_provider_url() -> Option<String> {
    if let Ok(url) = std::env::var("POT_PROVIDER_URL")
        .or_else(|_| std::env::var("BGUTIL_POT_PROVIDER_URL"))
        .or_else(|_| std::env::var("YTDLP_POT_PROVIDER_URL"))
    {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    // If no explicit env var is set, check if host.docker.internal resolves
    if let Ok(mut addrs) = ("host.docker.internal", 4416).to_socket_addrs()
        && addrs.next().is_some()
    {
        return Some("http://host.docker.internal:4416".to_string());
    }

    None
}

/// Returns the extractor arguments for YouTube player client, PO token provider base URL,
/// and any custom extractor arguments.
fn extractor_args() -> Vec<String> {
    let mut args = Vec::new();

    // Player client: default to mweb
    let player_client = std::env::var("YTDLP_PLAYER_CLIENT").unwrap_or_else(|_| "mweb".to_string());
    let player_client = player_client.trim();
    if !player_client.is_empty() {
        args.extend([
            "--extractor-args".to_string(),
            format!("youtube:player_client={}", player_client),
        ]);
    }

    // POT provider base URL
    if let Some(pot_url) = resolve_pot_provider_url() {
        let trimmed = pot_url.trim();
        if !trimmed.is_empty() {
            args.extend([
                "--extractor-args".to_string(),
                format!("youtubepot-bgutilhttp:base_url={}", trimmed),
            ]);
        }
    }

    // Custom extractor args passthrough
    if let Ok(extra) = std::env::var("YTDLP_EXTRACTOR_ARGS") {
        let trimmed = extra.trim();
        if !trimmed.is_empty() {
            args.extend(["--extractor-args".to_string(), trimmed.to_string()]);
        }
    }

    args
}

/// Returns the cookie arguments for yt-dlp.
/// Checks environment variables YTDLP_COOKIES, COOKIES_FILE, or a local cookies.txt file.
/// If browser cookies are requested via YTDLP_COOKIES_FROM_BROWSER or if Firefox profile paths exist,
/// adds `--cookies-from-browser firefox`. Otherwise omits cookie flags (relying on PO Token provider).
fn cookie_args() -> Vec<String> {
    if let Ok(path) = std::env::var("YTDLP_COOKIES")
        && !path.trim().is_empty()
        && std::path::Path::new(path.trim()).exists()
    {
        return vec!["--cookies".to_string(), path.trim().to_string()];
    }
    if let Ok(path) = std::env::var("COOKIES_FILE")
        && !path.trim().is_empty()
        && std::path::Path::new(path.trim()).exists()
    {
        return vec!["--cookies".to_string(), path.trim().to_string()];
    }
    if std::path::Path::new("cookies.txt").exists() {
        return vec!["--cookies".to_string(), "cookies.txt".to_string()];
    }
    if let Ok(browser) = std::env::var("YTDLP_COOKIES_FROM_BROWSER")
        && !browser.trim().is_empty()
    {
        return vec![
            "--cookies-from-browser".to_string(),
            browser.trim().to_string(),
        ];
    }
    // Check if firefox profile path exists on host or container
    if let Ok(home) = std::env::var("HOME") {
        let home_path = std::path::Path::new(&home);
        if home_path.join(".mozilla/firefox").exists()
            || home_path.join(".config/mozilla/firefox").exists()
            || home_path
                .join("snap/firefox/common/.mozilla/firefox")
                .exists()
            || home_path
                .join(".var/app/org.mozilla.firefox/.mozilla/firefox")
                .exists()
        {
            return vec!["--cookies-from-browser".to_string(), "firefox".to_string()];
        }
    }
    Vec::new()
}

/// RAII guard that cleans up temporary files when dropped.
struct TempFileGuard {
    paths: Vec<PathBuf>,
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        for path in &self.paths {
            if path.exists()
                && let Err(e) = std::fs::remove_file(path)
            {
                tracing::warn!(path = %path.display(), error = %e, "Failed to clean up temp file");
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

    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_cookie_args_defaults() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe { std::env::set_var("YTDLP_COOKIES_FROM_BROWSER", "firefox") };
        let args = cookie_args();
        assert_eq!(
            args,
            vec!["--cookies-from-browser".to_string(), "firefox".to_string()]
        );
        unsafe { std::env::remove_var("YTDLP_COOKIES_FROM_BROWSER") };
    }

    #[test]
    fn test_base_uvx_args_default() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe { std::env::remove_var("DISABLE_POT_PROVIDER") };
        let args = base_uvx_args();
        assert_eq!(
            args,
            vec![
                "--with".to_string(),
                "bgutil-ytdlp-pot-provider".to_string(),
                "yt-dlp".to_string()
            ]
        );
    }

    #[test]
    fn test_base_uvx_args_disabled() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe { std::env::set_var("DISABLE_POT_PROVIDER", "1") };
        let args = base_uvx_args();
        assert_eq!(args, vec!["yt-dlp".to_string()]);
        unsafe { std::env::remove_var("DISABLE_POT_PROVIDER") };
    }

    #[test]
    fn test_extractor_args_default() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("YTDLP_PLAYER_CLIENT");
            std::env::remove_var("YTDLP_EXTRACTOR_ARGS");
        }
        let args = extractor_args();
        assert!(args.contains(&"--extractor-args".to_string()));
        assert!(args.contains(&"youtube:player_client=mweb".to_string()));
    }

    #[test]
    fn test_extractor_args_custom_pot_url_and_player_client() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::set_var("POT_PROVIDER_URL", "http://192.168.1.50:4416");
            std::env::set_var("YTDLP_PLAYER_CLIENT", "android");
            std::env::set_var("YTDLP_EXTRACTOR_ARGS", "youtube:skip=hls");
        }

        let args = extractor_args();
        assert!(args.contains(&"youtube:player_client=android".to_string()));
        assert!(
            args.contains(&"youtubepot-bgutilhttp:base_url=http://192.168.1.50:4416".to_string())
        );
        assert!(args.contains(&"youtube:skip=hls".to_string()));

        unsafe {
            std::env::remove_var("POT_PROVIDER_URL");
            std::env::remove_var("YTDLP_PLAYER_CLIENT");
            std::env::remove_var("YTDLP_EXTRACTOR_ARGS");
        }
    }

    #[test]
    fn test_resolve_pot_provider_url_env_precedence() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("POT_PROVIDER_URL");
            std::env::remove_var("BGUTIL_POT_PROVIDER_URL");
            std::env::remove_var("YTDLP_POT_PROVIDER_URL");
        }

        unsafe { std::env::set_var("BGUTIL_POT_PROVIDER_URL", "http://bgutil-host:4416") };
        assert_eq!(
            resolve_pot_provider_url(),
            Some("http://bgutil-host:4416".to_string())
        );

        unsafe { std::env::set_var("POT_PROVIDER_URL", "http://pot-host:4416") };
        assert_eq!(
            resolve_pot_provider_url(),
            Some("http://pot-host:4416".to_string())
        );

        unsafe {
            std::env::remove_var("POT_PROVIDER_URL");
            std::env::remove_var("BGUTIL_POT_PROVIDER_URL");
        }
    }
}
