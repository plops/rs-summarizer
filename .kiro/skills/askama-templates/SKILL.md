---
name: askama-templates
description: Use when editing or creating Askama HTML templates, adding template variables, fixing compile-time template errors, or working with HTMX attributes in templates.
---

# Askama Template Conventions

## Overview

rs-summarizer uses askama 0.12 for HTML templating with compile-time checks and auto-escaping.

## File Locations

Templates live at the **project root** in `templates/` (not `src/templates/`). This is askama's default location.

```
templates/
├── index.html              # Main page with submission form
├── generation_partial.html # HTMX polling div for progressive display
├── browse.html             # Paginated browse page
└── search_results.html     # Similarity search results partial
```

## Template Structs

Each template has a corresponding Rust struct in `src/templates.rs`:

```rust
#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub models: Vec<ModelOption>,
}
```

The `path` attribute is relative to the `templates/` directory.

## Key Filters

| Filter | Usage | Purpose |
|--------|-------|---------|
| `\|safe` | `{{ summary\|safe }}` | Render pre-escaped HTML without double-escaping |
| `\|truncate(n)` | `{{ item.summary\|truncate(200) }}` | Truncate text to n characters |

## Auto-Escaping

Askama auto-escapes all template variables by default (XSS prevention). Use `|safe` only for content that's already been rendered to HTML (e.g., via `render_markdown_to_html()`).

## HTMX Integration

Templates use HTMX attributes for dynamic behavior:

```html
<div id="generation"
     {% if !summary_done %}hx-post="/generations/{{ identifier }}" hx-trigger="every 1s" hx-swap="outerHTML"{% endif %}>
```

Conditional HTMX attributes control polling — when `summary_done` is true, no HTMX attributes are rendered and polling stops.

## Rendering in Route Handlers

```rust
use askama::Template;

let template = IndexTemplate { models: app.model_options.as_ref().clone() };
Html(template.render().unwrap_or_default())
```

## Relevant Files

- `templates/` — HTML template files
- `src/templates.rs` — Template struct definitions
- `src/routes/mod.rs` — Template rendering in handlers
