use regex::Regex;

/// Validates various YouTube URL formats and extracts the 11-character video ID.
/// Returns `Some(video_id)` if the URL matches a recognized YouTube pattern,
/// or `None` if the URL is invalid or doesn't match.
///
/// Supported formats:
/// - `https://www.youtube.com/watch?v=ID`
/// - `https://m.youtube.com/watch?v=ID`
/// - `https://youtube.com/watch?v=ID`
/// - `https://www.youtube.com/live/ID`
/// - `https://www.youtube.com/shorts/ID`
/// - `https://youtu.be/ID`
/// - `https://www.youtu.be/ID`
///
/// Only HTTPS URLs are accepted. The video ID must be exactly 11 characters
/// from the set [A-Za-z0-9_-].
pub fn validate_youtube_url(url: &str) -> Option<String> {
    let patterns = [
        // Standard watch URL (www or m subdomain optional)
        r"^https://(?:(?:www|m)\.)?youtube\.com/watch\?v=([A-Za-z0-9_-]{11}).*",
        // Live URL (www or m subdomain optional)
        r"^https://(?:(?:www|m)\.)?youtube\.com/live/([A-Za-z0-9_-]{11}).*",
        // Short URL youtu.be (www subdomain optional, no m.)
        r"^https://(?:www\.)?youtu\.be/([A-Za-z0-9_-]{11}).*",
        // Shorts URL (www or m subdomain optional)
        r"^https://(?:(?:www|m)\.)?youtube\.com/shorts/([A-Za-z0-9_-]{11}).*",
        // Raw 11-character video ID
        r"^([A-Za-z0-9_-]{11})$",
    ];

    for pattern in &patterns {
        let re = Regex::new(pattern).ok()?;
        if let Some(captures) = re.captures(url.trim()) {
            if let Some(id_match) = captures.get(1) {
                return Some(id_match.as_str().to_string());
            }
        }
    }

    None
}

/// Normalizes a YouTube URL or a raw video ID to a canonical https://www.youtube.com/watch?v=ID format.
pub fn normalize_youtube_url(url: &str) -> Option<String> {
    validate_youtube_url(url).map(|id| format!("https://www.youtube.com/watch?v={}", id))
}

/// Splits a string of space/newline/tab/comma-separated URLs or video IDs.
pub fn split_urls(input: &str) -> Vec<String> {
    input
        .split(|c: char| c.is_whitespace() || c == ',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Validates Hacker News URL formats or item IDs and extracts the item ID.
pub fn validate_hn_url(url: &str) -> Option<u64> {
    let patterns = [
        r"^(?:https?://)?(?:news\.)?ycombinator\.com/item\?id=([0-9]+).*",
        r"^(?:https?://)?(?:news\.)?ycombinator\.com/item/([0-9]+).*",
        r"^item\?id=([0-9]+).*",
        r"^([0-9]{6,10})$",
    ];

    let trimmed = url.trim();
    for pattern in &patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(captures) = re.captures(trimmed) {
                if let Some(id_match) = captures.get(1) {
                    if let Ok(id) = id_match.as_str().parse::<u64>() {
                        return Some(id);
                    }
                }
            }
        }
    }

    None
}

/// Normalizes a Hacker News URL or item ID to canonical https://news.ycombinator.com/item?id=ID format.
pub fn normalize_hn_url(url: &str) -> Option<String> {
    validate_hn_url(url).map(|id| format!("https://news.ycombinator.com/item?id={}", id))
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParsedSource {
    YouTube(String),
    HackerNews(u64, String),
    Unknown(String),
}

/// Identifies whether a given URL or ID is a YouTube link/ID or a Hacker News link/ID.
pub fn parse_source_url(url: &str) -> ParsedSource {
    let trimmed = url.trim();
    if trimmed.contains("ycombinator.com") || trimmed.starts_with("item?id=") {
        if let Some(hn_id) = validate_hn_url(url) {
            return ParsedSource::HackerNews(hn_id, format!("https://news.ycombinator.com/item?id={}", hn_id));
        }
    }

    if let Some(yt_norm) = normalize_youtube_url(url) {
        return ParsedSource::YouTube(yt_norm);
    }

    if let Some(hn_id) = validate_hn_url(url) {
        return ParsedSource::HackerNews(hn_id, format!("https://news.ycombinator.com/item?id={}", hn_id));
    }

    ParsedSource::Unknown(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_hn_url() {
        assert_eq!(validate_hn_url("https://news.ycombinator.com/item?id=40000000"), Some(40000000));
        assert_eq!(validate_hn_url("http://news.ycombinator.com/item?id=123456"), Some(123456));
        assert_eq!(validate_hn_url("news.ycombinator.com/item?id=789012"), Some(789012));
        assert_eq!(validate_hn_url("item?id=999999"), Some(999999));
        assert_eq!(validate_hn_url("40000000"), Some(40000000));
        assert_eq!(validate_hn_url("https://example.com/not-hn"), None);
    }

    #[test]
    fn test_normalize_hn_url() {
        assert_eq!(
            normalize_hn_url("item?id=40000000"),
            Some("https://news.ycombinator.com/item?id=40000000".to_string())
        );
    }

    #[test]
    fn test_parse_source_url() {
        assert_eq!(
            parse_source_url("https://news.ycombinator.com/item?id=40000000"),
            ParsedSource::HackerNews(40000000, "https://news.ycombinator.com/item?id=40000000".to_string())
        );
        assert_eq!(
            parse_source_url("https://www.youtube.com/watch?v=Dgj2jivpaJk"),
            ParsedSource::YouTube("https://www.youtube.com/watch?v=Dgj2jivpaJk".to_string())
        );
        assert_eq!(
            parse_source_url("Dgj2jivpaJk"),
            ParsedSource::YouTube("https://www.youtube.com/watch?v=Dgj2jivpaJk".to_string())
        );
        assert_eq!(
            parse_source_url("invalid_random_string"),
            ParsedSource::Unknown("invalid_random_string".to_string())
        );
    }

    #[test]
    fn test_live_url() {
        assert_eq!(
            Some("0123456789a".to_string()),
            validate_youtube_url("https://www.youtube.com/live/0123456789a")
        );
    }

    #[test]
    fn test_live_url_with_params() {
        assert_eq!(
            Some("0123456789a".to_string()),
            validate_youtube_url("https://www.youtube.com/live/0123456789a&abc=123")
        );
    }

    #[test]
    fn test_watch_url_with_params() {
        assert_eq!(
            Some("_123456789a".to_string()),
            validate_youtube_url("https://www.youtube.com/watch?v=_123456789a&abc=123")
        );
    }

    #[test]
    fn test_watch_url_no_subdomain() {
        assert_eq!(
            Some("_123456789a".to_string()),
            validate_youtube_url("https://youtube.com/watch?v=_123456789a&abc=123")
        );
    }

    #[test]
    fn test_youtu_be_with_www() {
        assert_eq!(
            Some("-123456789a".to_string()),
            validate_youtube_url("https://www.youtu.be/-123456789a&abc=123")
        );
    }

    #[test]
    fn test_youtu_be_no_subdomain() {
        assert_eq!(
            Some("-123456789a".to_string()),
            validate_youtube_url("https://youtu.be/-123456789a&abc=123")
        );
    }

    #[test]
    fn test_http_rejected() {
        assert_eq!(
            None,
            validate_youtube_url("http://www.youtube.com/live/0123456789a")
        );
    }

    #[test]
    fn test_mobile_watch_url() {
        assert_eq!(
            Some("QbnkIdw0HJQ".to_string()),
            validate_youtube_url("https://m.youtube.com/watch?v=QbnkIdw0HJQ")
        );
    }

    #[test]
    fn test_standard_watch_url() {
        assert_eq!(
            Some("Dgj2jivpaJk".to_string()),
            validate_youtube_url("https://www.youtube.com/watch?v=Dgj2jivpaJk")
        );
    }

    #[test]
    fn test_shorts_url() {
        assert_eq!(
            Some("Dgj2jivpaJk".to_string()),
            validate_youtube_url("https://www.youtube.com/shorts/Dgj2jivpaJk")
        );
    }

    #[test]
    fn test_raw_id() {
        assert_eq!(
            Some("_Qeur243coc".to_string()),
            validate_youtube_url("_Qeur243coc")
        );
    }

    #[test]
    fn test_normalize_youtube_url() {
        assert_eq!(
            Some("https://www.youtube.com/watch?v=_Qeur243coc".to_string()),
            normalize_youtube_url("_Qeur243coc")
        );
        assert_eq!(
            Some("https://www.youtube.com/watch?v=Dgj2jivpaJk".to_string()),
            normalize_youtube_url("https://www.youtube.com/watch?v=Dgj2jivpaJk")
        );
        assert_eq!(
            None,
            normalize_youtube_url("not-an-id")
        );
    }

    #[test]
    fn test_split_urls() {
        let input = "https://www.youtube.com/watch?v=123, _Qeur243coc\nhttps://youtu.be/abc   xyz12345678";
        let res = split_urls(input);
        assert_eq!(res, vec![
            "https://www.youtube.com/watch?v=123",
            "_Qeur243coc",
            "https://youtu.be/abc",
            "xyz12345678"
        ]);
    }
}
