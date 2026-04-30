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
    ];

    for pattern in &patterns {
        let re = Regex::new(pattern).ok()?;
        if let Some(captures) = re.captures(url) {
            if let Some(id_match) = captures.get(1) {
                return Some(id_match.as_str().to_string());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
