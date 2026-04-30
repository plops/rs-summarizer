use regex::Regex;

use super::url_validator::validate_youtube_url;

/// Builds a canonical YouTube URL with a time offset parameter.
fn youtube_url_with_t(video_id: &str, seconds: u32) -> String {
    format!("https://www.youtube.com/watch?v={}&t={}s", video_id, seconds)
}

/// Replaces timestamps in HTML (MM:SS or HH:MM:SS) with anchor tags linking
/// to the given YouTube video at that timestamp offset.
///
/// If the provided URL is not a valid YouTube URL, the HTML is returned unchanged.
pub fn replace_timestamps_in_html(html: &str, youtube_url: &str) -> String {
    let video_id = match validate_youtube_url(youtube_url) {
        Some(id) => id,
        None => return html.to_string(),
    };

    // Match mm:ss or hh:mm:ss where mm and ss are 0-59, hours optional 1-2 digits.
    let pattern = Regex::new(r"\b(?:\d{1,2}:)?[0-5]?\d:[0-5]\d\b").unwrap();

    let result = pattern.replace_all(html, |caps: &regex::Captures| {
        let ts_text = &caps[0];
        let parts: Vec<&str> = ts_text.split(':').collect();
        let total = if parts.len() == 3 {
            let h: u32 = parts[0].parse().unwrap_or(0);
            let mm: u32 = parts[1].parse().unwrap_or(0);
            let ss: u32 = parts[2].parse().unwrap_or(0);
            h * 3600 + mm * 60 + ss
        } else {
            let mm: u32 = parts[0].parse().unwrap_or(0);
            let ss: u32 = parts[1].parse().unwrap_or(0);
            mm * 60 + ss
        };
        let link = youtube_url_with_t(&video_id, total);
        format!("<a href=\"{}\">{}</a>", link, ts_text)
    });

    result.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mm_ss_replacement() {
        let youtube = "https://www.youtube.com/watch?v=8S4a_LdHhsc";
        let html = "<p><strong>14:58 Paper 1:</strong></p>";
        let out = replace_timestamps_in_html(html, youtube);
        // 14*60 + 58 = 898
        assert!(out.contains("t=898s"));
        assert!(out.contains("<a href=\""));
        assert!(out.contains("14:58"));
    }

    #[test]
    fn test_hh_mm_ss_replacement() {
        let youtube = "https://www.youtube.com/watch?v=8S4a_LdHhsc";
        let html = "<p><strong>01:03:05 Testing:</strong></p>";
        let out = replace_timestamps_in_html(html, youtube);
        // 1*3600 + 3*60 + 5 = 3785
        assert!(out.contains("t=3785s"));
        assert!(out.contains("<a href=\""));
        assert!(out.contains("01:03:05"));
    }

    #[test]
    fn test_multiple_timestamps_and_url_normalization() {
        // Input URL contains an existing time param which should be ignored after normalization
        let youtube = "https://youtu.be/8S4a_LdHhsc?t=100";
        let html = "<p><strong>00:03:48 Debunking:</strong></p>\n<p><strong>14:58 Paper 1:</strong></p>\n<p><strong>01:06:01 Targeting Apoptosis:</strong></p>";
        let out = replace_timestamps_in_html(html, youtube);
        // 00:03:48 -> 3*60 + 48 = 228
        // 14:58 -> 14*60 + 58 = 898
        // 01:06:01 -> 1*3600 + 6*60 + 1 = 3961
        assert_eq!(out.matches("<a href=\"").count(), 3);
        assert!(out.contains("t=228s"));
        assert!(out.contains("t=898s"));
        assert!(out.contains("t=3961s"));
        // Ensure the original t=100s from input url is not present
        assert!(!out.contains("t=100s"));
        // Ensure links point to the canonical watch?v=ID form
        assert!(out.contains("watch?v=8S4a_LdHhsc"));
    }

    #[test]
    fn test_invalid_url_no_change() {
        let bad = "https://example.com/watch?v=xxxx";
        let html = "<div><p><strong>01:00 Sample:</strong></p></div>";
        let out = replace_timestamps_in_html(html, bad);
        // Should be unchanged: no anchor tags and original timestamp text remains
        assert_eq!(out, html);
        assert!(!out.contains("<a href=\""));
        assert!(out.contains("01:00"));
    }
}
