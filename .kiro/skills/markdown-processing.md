---
inclusion: manual
---

# Markdown Processing Pipeline

## Overview

rs-summarizer has two markdown processing stages with different purposes:

1. **Markdown → YouTube format** (`convert_markdown_to_youtube_format`) — for text storage and YouTube comment compatibility
2. **Markdown → HTML** (`render_markdown_to_html`) — for web display via pulldown-cmark

## Stage 1: YouTube Format Conversion

File: `src/utils/markdown_converter.rs`

Converts markdown to YouTube's limited formatting (only `*bold*` supported):

### Transformations (applied in order)

1. Reposition punctuation adjacent to `**` markers: `**:` → `:**`, `**,` → `,**`, etc.
2. Convert `**` to `*` (YouTube bold)
3. Reposition punctuation adjacent to `*` markers: `*:` → `:*`, etc.
4. Convert `## Heading` at start of text to `*Heading*`
5. Replace dots in URLs before TLDs with `-dot-` (avoids YouTube link censoring)

### Supported TLDs for dot replacement

com, org, de, us, gov, net, edu, info, io, co.uk, ca, fr, au, jp, ru, ch, it, nl, se, es, br, mx, in, kr

### Usage in pipeline

Called in `tasks.rs` after summary generation:
```rust
let youtube_text = convert_markdown_to_youtube_format(&result.summary_text);
db::mark_timestamps_done(db_pool, identifier, &youtube_text).await?;
```

## Stage 2: HTML Rendering

File: `src/utils/markdown_renderer.rs`

Renders markdown to HTML for web display using `pulldown-cmark 0.13.3`:

```rust
use pulldown_cmark::{html, Options, Parser};

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
```

### Usage in route handlers

Called in `render_generation_partial()` before passing to the template:
```rust
let summary_html = render_markdown_to_html(&s.summary);
// Template uses {{ summary|safe }} to render without double-escaping
```

## Data Flow

```
Gemini output (markdown)
    │
    ├──→ render_markdown_to_html() ──→ HTML display in browser (via |safe filter)
    │
    └──→ convert_markdown_to_youtube_format() ──→ stored in DB as timestamped_summary_in_youtube_format
                                                    └──→ replace_timestamps_in_html() ──→ clickable YouTube links
```

## Relevant Files

- `src/utils/markdown_converter.rs` — YouTube format conversion (ported from Python)
- `src/utils/markdown_renderer.rs` — HTML rendering via pulldown-cmark
- `src/routes/mod.rs` — `render_generation_partial()` calls both
- `source04/tsum/s03_convert_markdown_to_youtube_format.py` — Python original
