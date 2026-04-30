use axum::{
    extract::{ConnectInfo, Form, Path, Query, State},
    response::{Html, IntoResponse},
};
use askama::Template;
use chrono::Utc;
use std::net::SocketAddr;
use std::time::Duration;

use crate::db;
use crate::models::{BrowseParams, SearchForm, SubmitForm};
use crate::services::deduplication::DeduplicationService;
use crate::services::embedding::EmbeddingService;
use crate::services::rate_limiter::RateLimiter;
use crate::state::AppState;
use crate::tasks;
use crate::templates::{BrowseTemplate, GenerationPartialTemplate, IndexTemplate, SearchResultsTemplate};
use crate::utils::timestamp_linker::replace_timestamps_in_html;

/// GET / — renders the index page with the submission form.
pub async fn index(State(app): State<AppState>) -> impl IntoResponse {
    let template = IndexTemplate {
        models: app.model_options.as_ref().clone(),
    };
    Html(template.render().unwrap_or_default())
}

/// POST /process_transcript — accepts a form submission, checks for duplicates,
/// spawns a background summarization task, and returns an HTMX polling partial.
pub async fn process_transcript(
    State(app): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Form(input): Form<SubmitForm>,
) -> impl IntoResponse {
    // Find the model option
    let model = app.model_options.iter().find(|m| m.name == input.model);
    let model = match model {
        Some(m) => m.clone(),
        None => return Html("<p>Invalid model selected.</p>".to_string()),
    };

    // Check rate limit
    let allowed = RateLimiter::check_rate_limit(
        &model,
        &app.model_counts,
        &app.last_reset_day,
    )
    .await;
    if !allowed {
        return Html(
            "<p>Rate limit exceeded for this model. Please try again later.</p>".to_string(),
        );
    }

    // Check for duplicates
    let dedup_svc = DeduplicationService::new(Duration::from_secs(300));
    if let Ok(Some(existing_id)) = dedup_svc
        .check_duplicate(&app.db, &input.original_source_link, &input.model)
        .await
    {
        // Return existing generation partial
        return render_generation_partial(&app, existing_id).await;
    }

    // Insert new row
    let timestamp_start = Utc::now().to_rfc3339();
    let id = match db::insert_new_summary(&app.db, &input, &addr.to_string(), &timestamp_start)
        .await
    {
        Ok(id) => id,
        Err(e) => return Html(format!("<p>Error: {}</p>", e)),
    };

    // Increment rate limit counter
    RateLimiter::increment_counter(&input.model, &app.model_counts).await;

    // Spawn background task
    let app_clone = app.clone();
    let db_clone = app.db.clone();
    tokio::spawn(async move {
        tasks::process_summary(db_clone, id, app_clone).await;
    });

    // Return HTMX polling partial
    let template = GenerationPartialTemplate {
        identifier: id,
        summary: "Processing...".to_string(),
        summary_done: false,
        timestamps: String::new(),
    };
    Html(template.render().unwrap_or_default())
}

/// POST /generations/{identifier} — polling endpoint that returns the current
/// partial summary or final result for a given generation.
pub async fn get_generation(
    State(app): State<AppState>,
    Path(identifier): Path<i64>,
) -> impl IntoResponse {
    render_generation_partial(&app, identifier).await
}

/// GET /browse — paginated browse page showing summaries from the metadata cache.
pub async fn browse_summaries(
    State(app): State<AppState>,
    Query(params): Query<BrowseParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(0);
    let summaries = db::fetch_browse_page(&app.db, page)
        .await
        .unwrap_or_default();
    let has_next = summaries.len() == 20;

    let template = BrowseTemplate {
        summaries,
        page,
        has_next,
    };
    Html(template.render().unwrap_or_default())
}

/// POST /search — similarity search endpoint using embeddings.
pub async fn search_similar(
    State(app): State<AppState>,
    Form(query): Form<SearchForm>,
) -> impl IntoResponse {
    let embedding_svc = EmbeddingService::new(
        app.gemini_api_key.clone(),
        "text-embedding-004",
        3072,
    );

    let results = match embedding_svc.embed_text(&query.query).await {
        Ok(query_embedding) => {
            match embedding_svc
                .find_similar(&app.db, &query_embedding, 10)
                .await
            {
                Ok(similar) => {
                    // Fetch full summaries for results
                    let mut full_results = Vec::new();
                    for (id, score) in similar {
                        if let Ok(Some(summary)) = db::fetch_summary(&app.db, id).await {
                            full_results.push((score, summary));
                        }
                    }
                    full_results
                }
                Err(_) => Vec::new(),
            }
        }
        Err(_) => Vec::new(),
    };

    let template = SearchResultsTemplate { results };
    Html(template.render().unwrap_or_default())
}

/// Helper to render the generation partial for a given identifier.
async fn render_generation_partial(app: &AppState, identifier: i64) -> Html<String> {
    let summary = db::fetch_summary(&app.db, identifier).await.ok().flatten();

    match summary {
        Some(s) => {
            let timestamps_html = if s.timestamps_done {
                replace_timestamps_in_html(
                    &s.timestamped_summary_in_youtube_format,
                    &s.original_source_link,
                )
            } else {
                String::new()
            };

            let template = GenerationPartialTemplate {
                identifier: s.identifier,
                summary: s.summary.clone(),
                summary_done: s.summary_done,
                timestamps: timestamps_html,
            };
            Html(template.render().unwrap_or_default())
        }
        None => Html("<p>Summary not found.</p>".to_string()),
    }
}
