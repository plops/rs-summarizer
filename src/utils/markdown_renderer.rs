use pulldown_cmark::{html, Options, Parser};

/// Renders markdown text to HTML using pulldown-cmark with common extensions enabled.
pub fn render_markdown_to_html(markdown_input: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(markdown_input, options);

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    html_output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_markdown() {
        let result = render_markdown_to_html("# Hello\n\nThis is **bold**.");
        assert!(result.contains("<h1>Hello</h1>"));
        assert!(result.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_strikethrough() {
        let result = render_markdown_to_html("~~deleted~~");
        assert!(result.contains("<del>deleted</del>"));
    }

    #[test]
    fn test_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let result = render_markdown_to_html(md);
        assert!(result.contains("<table>"));
        assert!(result.contains("<td>1</td>"));
    }

    #[test]
    fn test_empty_input() {
        let result = render_markdown_to_html("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_timestamps_preserved() {
        let md = "**00:01:30 Introduction**\n\nSome content here.";
        let result = render_markdown_to_html(md);
        assert!(result.contains("00:01:30"));
        assert!(result.contains("<strong>"));
    }
}
