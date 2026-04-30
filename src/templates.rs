use askama::Template;

use crate::models::Summary;
use crate::state::ModelOption;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub models: Vec<ModelOption>,
}

#[derive(Template)]
#[template(path = "generation_partial.html")]
pub struct GenerationPartialTemplate {
    pub identifier: i64,
    pub summary: String,
    pub summary_done: bool,
    pub timestamps: String,
}

#[derive(Template)]
#[template(path = "browse.html")]
pub struct BrowseTemplate {
    pub summaries: Vec<Summary>,
    pub page: u32,
    pub has_next: bool,
}

#[derive(Template)]
#[template(path = "search_results.html")]
pub struct SearchResultsTemplate {
    pub results: Vec<(f32, Summary)>,
}
