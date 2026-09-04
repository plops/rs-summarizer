use regex::Regex;

use super::url_validator::validate_youtube_url;

/// Builds a canonical YouTube URL with a time offset parameter.
fn youtube_url_with_t(video_id: &str, seconds: u32) -> String {
    format!(
        "https://www.youtube.com/watch?v={}&t={}s",
        video_id, seconds
    )
}

use super::url_validator::split_urls;

/// Replaces timestamps in HTML (MM:SS or HH:MM:SS) with anchor tags linking
/// to the given YouTube video at that timestamp offset.
///
/// If the provided URL is not a valid YouTube URL, the HTML is returned unchanged.
pub fn replace_timestamps_in_html(html: &str, youtube_url: &str) -> String {
    let urls = split_urls(youtube_url);
    if urls.is_empty() {
        return html.to_string();
    }

    if urls.len() == 1 {
        let video_id = match validate_youtube_url(&urls[0]) {
            Some(id) => id,
            None => return html.to_string(),
        };
        return replace_timestamps_for_video(html, &video_id);
    }

    // Multiple URLs: parse and map each URL to its video ID.
    let url_mappings: Vec<(String, String)> = urls
        .iter()
        .filter_map(|url| validate_youtube_url(url).map(|id| (url.clone(), id)))
        .collect();

    if url_mappings.is_empty() {
        return html.to_string();
    }

    // Line-by-line scanning to track the active video ID.
    let mut active_video_id = url_mappings[0].1.clone();
    let mut result_lines = Vec::new();

    for line in html.lines() {
        for (url, id) in &url_mappings {
            if line.contains(url) {
                active_video_id = id.clone();
                break;
            }
        }
        let replaced_line = replace_timestamps_for_video(line, &active_video_id);
        result_lines.push(replaced_line);
    }

    result_lines.join("\n")
}

use std::sync::OnceLock;

fn replace_timestamps_for_video(html: &str, video_id: &str) -> String {
    // Match mm:ss or hh:mm:ss where mm and ss are 0-59 (2 digits for seconds).
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| Regex::new(r"\b(?:\d{1,2}:)?[0-5]\d:[0-5]\d\b").unwrap());

    let result = pattern.replace_all(html, |caps: &regex::Captures| {
        let mat = caps.get(0).unwrap();
        let ts_text = mat.as_str();

        // Extra check: Ensure no preceding/following dash, slash, colon, dot, or following letters (e.g. 16:09px)
        let start = mat.start();
        let end = mat.end();
        if let Some(c) = html[..start].chars().last()
            && (c == '-' || c == '/' || c == ':' || c == '.')
        {
            return ts_text.to_string();
        }
        if let Some(c) = html[end..].chars().next()
            && (c == '-' || c == '/' || c == ':' || c == '.' || c.is_alphabetic())
        {
            return ts_text.to_string();
        }

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
        let link = youtube_url_with_t(video_id, total);
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

    #[test]
    fn test_multiple_urls_timestamp_linking() {
        let youtube_urls = "https://www.youtube.com/watch?v=8S4a_LdHhsc https://www.youtube.com/watch?v=Dgj2jivpaJk";
        let html = r#"<h3>Summary for https://www.youtube.com/watch?v=8S4a_LdHhsc</h3>
<p><strong>01:30 Video 1 segment</strong></p>
<h3>Summary for https://www.youtube.com/watch?v=Dgj2jivpaJk</h3>
<p><strong>02:45 Video 2 segment</strong></p>"#;

        let out = replace_timestamps_in_html(html, youtube_urls);
        // Video 1 segment: 1*60 + 30 = 90
        assert!(out.contains("watch?v=8S4a_LdHhsc&t=90s"));
        // Video 2 segment: 2*60 + 45 = 165
        assert!(out.contains("watch?v=Dgj2jivpaJk&t=165s"));
    }

    #[test]
    fn test_timestamp_regex_avoids_ratios_and_css() {
        let youtube = "https://www.youtube.com/watch?v=8S4a_LdHhsc";
        let html = "<div style=\"aspect-ratio: 16:9; width: 16:09px;\">Ratio text 16:9</div>";
        let out = replace_timestamps_in_html(html, youtube);
        assert_eq!(out, html);
    }
}
