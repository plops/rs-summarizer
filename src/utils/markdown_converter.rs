use regex::Regex;
use std::sync::OnceLock;

/// Converts markdown-formatted text to YouTube comment format.
///
/// YouTube comments only support `*word*` for bold text (not `**word**`).
/// Punctuation like colons, commas, semicolons, and periods cannot be inside
/// bold markers (e.g., `*Description:*` must be written as `*Description*:`).
/// YouTube also censors comments containing links, so URLs have their dots
/// replaced with `-dot-`.
///
/// Transformations applied (in order):
/// 1. Reposition punctuation adjacent to `**` bold markers
/// 2. Convert `**` to `*`
/// 3. Reposition punctuation adjacent to `*` bold markers
/// 4. Convert `## Heading` (including multiline) to `*Heading*`
/// 5. Replace dots in URLs with `-dot-`
pub fn convert_markdown_to_youtube_format(text: &str) -> String {
    let mut text = text.to_string();

    // Adapt the markdown to YouTube formatting
    // Reposition punctuation adjacent to ** bold markers
    text = text.replace("**:", ":**");
    text = text.replace("**,", ",**");
    text = text.replace("**;", ";**");
    text = text.replace("**.", ".**");

    // Convert ** to *
    while text.contains("**") {
        text = text.replace("**", "*");
    }

    // Reposition punctuation adjacent to * bold markers
    text = text.replace("*:", ":*");
    text = text.replace("*,", ",*");
    text = text.replace("*;", ";*");
    text = text.replace("*.", ".*");

    // Markdown title starting with ## converted to bold text
    static HEADING_RE: OnceLock<Regex> = OnceLock::new();
    let heading_re = HEADING_RE.get_or_init(|| Regex::new(r"(?m)^##\s*(.*)").unwrap());
    text = heading_re.replace_all(&text, "*$1*").to_string();

    // Find any text that looks like a URL and replace the dot before TLD with -dot-
    static URL_RE: OnceLock<Regex> = OnceLock::new();
    let url_re = URL_RE.get_or_init(|| {
        Regex::new(
            r"((?:https?://)?(?:www\.)?\S+)\.(com|org|de|us|gov|net|edu|info|io|co\.uk|ca|fr|au|jp|ru|ch|it|nl|se|es|br|mx|in|kr)",
        )
        .unwrap()
    });
    text = url_re.replace_all(&text, "$1-dot-$2").to_string();

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_markdown_to_youtube_format() {
        let input = "**Title:**\nLet's **go** to http://www.google.com/search?q=hello.";
        let expected = "*Title:*\nLet's *go* to http://www.google-dot-com/search?q=hello.";
        let result = convert_markdown_to_youtube_format(input);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_double_asterisk_to_single() {
        let result = convert_markdown_to_youtube_format("**bold**");
        assert_eq!("*bold*", result);
    }

    #[test]
    fn test_heading_conversion() {
        let result = convert_markdown_to_youtube_format("## My Heading");
        assert_eq!("*My Heading*", result);
    }

    #[test]
    fn test_heading_multiline() {
        // ## after newline should also be converted with multiline flag
        let result = convert_markdown_to_youtube_format("Hello\n## Second heading");
        assert_eq!("Hello\n*Second heading*", result);
    }

    #[test]
    fn test_url_dot_replacement() {
        let result = convert_markdown_to_youtube_format("Visit https://example.com today");
        assert_eq!("Visit https://example-dot-com today", result);
    }

    #[test]
    fn test_punctuation_repositioning_colon() {
        let result = convert_markdown_to_youtube_format("**Word:**");
        assert_eq!("*Word:*", result);
    }

    #[test]
    fn test_punctuation_repositioning_comma() {
        let result = convert_markdown_to_youtube_format("**Word,** next");
        assert_eq!("*Word,* next", result);
    }

    #[test]
    fn test_no_urls_unchanged() {
        let result = convert_markdown_to_youtube_format("Just plain text here");
        assert_eq!("Just plain text here", result);
    }
}
