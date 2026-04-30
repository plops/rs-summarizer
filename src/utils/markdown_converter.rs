use regex::Regex;

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
/// 4. Convert `## Heading` at start of text to `*Heading*`
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
    text = text.replace("**", "*");

    // Reposition punctuation adjacent to * bold markers
    text = text.replace("*:", ":*");
    text = text.replace("*,", ",*");
    text = text.replace("*;", ";*");
    text = text.replace("*.", ".*");

    // Markdown title starting with ## converted to bold text
    // Note: ^ matches start of string only (not multiline), matching Python behavior
    let heading_re = Regex::new(r"^##\s*(.*)").unwrap();
    text = heading_re.replace(&text, "*$1*").to_string();

    // Find any text that looks like a URL and replace the dot before TLD with -dot-
    let url_re = Regex::new(
        r"((?:https?://)?(?:www\.)?\S+)\.(com|org|de|us|gov|net|edu|info|io|co\.uk|ca|fr|au|jp|ru|ch|it|nl|se|es|br|mx|in|kr)"
    ).unwrap();
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
    fn test_heading_only_at_start() {
        // ## not at start of string should not be converted
        let result = convert_markdown_to_youtube_format("Hello\n## Not a heading");
        assert_eq!("Hello\n## Not a heading", result);
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
