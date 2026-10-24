use askama::Template;

use crate::state::ModelOption;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub models: Vec<ModelOption>,
    pub app_version: &'static str,
    /// Concrete model behind the `auto` pseudo-model selection.
    pub auto_model: &'static str,
}

#[derive(Template)]
#[template(path = "generation_partial.html")]
pub struct GenerationPartialTemplate {
    pub identifier: i64,
    pub summary: String,
    pub summary_done: bool,
    pub generation_status: String,
    pub error_message: String,
    pub retry_display: String,
    pub model: String,
    pub cost_display: String,
    pub original_source_link: String,
    pub clipboard_text: String,
    pub rating_stats: RatingStats,
}

use crate::models::RatingStats;

/// A summary with pre-rendered HTML fields for display.
pub struct BrowseSummaryItem {
    pub identifier: i64,
    pub model: String,
    pub rs_summarizer_version: String,
    pub cost_display: String,
    pub original_source_link: String,
    pub title: String,
    pub summary_html: String,
    pub clipboard_text: String,
    pub rating_stats: RatingStats,
}

#[derive(Template)]
#[template(path = "rating_partial.html")]
pub struct RatingPartialTemplate {
    pub identifier: i64,
    pub rating_stats: RatingStats,
}

#[derive(Template)]
#[template(path = "browse.html")]
pub struct BrowseTemplate {
    pub summaries: Vec<BrowseSummaryItem>,
    pub app_version: &'static str,
    pub page: u32,
    pub has_next: bool,
}

/// One metadata row of the all-entries browser. No summary, no transcript.
pub struct EntryItem {
    pub identifier: i64,
    pub model: String,
    pub original_source_link: String,
    pub short_link: String,
    pub title: String,
    pub timestamp_start: String,
    pub cost_display: String,
}

#[derive(Template)]
#[template(path = "entries.html")]
pub struct EntriesTemplate {
    pub entries: Vec<EntryItem>,
    pub app_version: &'static str,
    pub page: u32,
    pub has_next: bool,
}

/// Lazy-loaded detail fragment for one entry. No transcript by construction.
#[derive(Template)]
#[template(path = "entry_detail.html")]
pub struct EntryDetailTemplate {
    pub identifier: i64,
    pub model: String,
    pub rs_summarizer_version: String,
    pub cost_display: String,
    pub timestamp_start: String,
    pub original_source_link: String,
    pub title: String,
    pub summary_html: String,
    pub clipboard_text: String,
    pub rating_stats: RatingStats,
}

/// A search result with pre-rendered HTML summary.
pub struct SearchResultItem {
    pub identifier: i64,
    pub model: String,
    pub score: f32,
    pub summary_html: String,
    pub original_source_link: String,
}

#[derive(Template)]
#[template(path = "search_results.html")]
pub struct SearchResultsTemplate {
    pub results: Vec<SearchResultItem>,
}
