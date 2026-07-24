use axum::{
    extract::{ConnectInfo, Form, Path, Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse},
};
use askama::Template;
use chrono::Utc;
use std::net::SocketAddr;

use crate::db;
use crate::models::{BrowseParams, SearchForm, SubmitForm, SubmitRatingForm};
use crate::services::embedding::EmbeddingService;
use crate::services::rate_limiter::RateLimiter;
use crate::state::AppState;
use crate::tasks;
use crate::templates::{BrowseTemplate, BrowseSummaryItem, GenerationPartialTemplate, IndexTemplate, SearchResultsTemplate, SearchResultItem, RatingPartialTemplate};
use crate::utils::markdown_renderer::render_markdown_to_html;
use crate::utils::timestamp_linker::replace_timestamps_in_html;

/// Helper function to extract client IP address from request headers or socket address.
pub fn extract_client_ip(headers: &HeaderMap, addr: &SocketAddr) -> String {
    if let Some(forwarded) = headers.get("x-forwarded-for").and_then(|h| h.to_str().ok()) {
        if let Some(first_ip) = forwarded.split(',').next() {
            let trimmed = first_ip.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    if let Some(real_ip) = headers.get("x-real-ip").and_then(|h| h.to_str().ok()) {
        let trimmed = real_ip.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    addr.ip().to_string()
}

fn render_template<T: Template>(template: &T) -> Html<String> {
    match template.render() {
        Ok(html) => Html(html),
        Err(e) => {
            tracing::error!("Template render failed: {e}");
            Html("<p>Internal rendering error</p>".into())
        }
    }
}

/// GET / — renders the index page with the submission form.
pub async fn index(State(app): State<AppState>) -> impl IntoResponse {
    let template = IndexTemplate {
        models: app.model_options.as_ref().clone(),
    };
    render_template(&template)
}

/// POST /process_transcript — accepts a form submission, checks for duplicates,
/// spawns a background summarization task, and returns an HTMX polling partial.
pub async fn process_transcript(
    State(app): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Form(mut input): Form<SubmitForm>,
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

    // Check at least one of original_source_link or transcript is provided
    let url_empty = input.original_source_link.trim().is_empty();
    let transcript_empty = input.transcript.as_ref().is_none_or(|t| t.trim().is_empty());
    if url_empty && transcript_empty {
        return Html("<p>Error: Please provide either a YouTube URL, Hacker News URL, or paste content.</p>".to_string());
    }

    // Split, validate, and normalize input URLs/IDs
    let mut normalized_links = Vec::new();
    if !url_empty {
        let items = crate::utils::url_validator::split_urls(&input.original_source_link);
        for item in &items {
            match crate::utils::url_validator::parse_source_url(item) {
                crate::utils::url_validator::ParsedSource::YouTube(norm) => {
                    normalized_links.push(norm);
                }
                crate::utils::url_validator::ParsedSource::HackerNews(_, norm) => {
                    normalized_links.push(norm);
                }
                crate::utils::url_validator::ParsedSource::Unknown(u) => {
                    return Html(format!(
                        "<p>Error: The provided value '{}' is neither a valid YouTube URL, video ID, nor a Hacker News URL.</p>",
                        u
                    ));
                }
            }
        }
    }
    input.original_source_link = normalized_links.join(" ");

    // Check for duplicates using state's DeduplicationService
    let mut duplicate_id = None;

    if let Some(ref transcript) = input.transcript {
        let trimmed_transcript = transcript.trim();
        if !trimmed_transcript.is_empty() {
            if let Ok(Some(existing_id)) = app
                .dedup_service
                .check_duplicate_by_transcript(&app.db, trimmed_transcript, &input.model)
                .await
            {
                duplicate_id = Some(existing_id);
            }
        }
    }

    if duplicate_id.is_none() && !input.original_source_link.trim().is_empty() {
        if let Ok(Some(existing_id)) = app
            .dedup_service
            .check_duplicate(&app.db, input.original_source_link.trim(), &input.model)
            .await
        {
            duplicate_id = Some(existing_id);
        }
    }

    if let Some(existing_id) = duplicate_id {
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
    render_template(&template)
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
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(params): Query<BrowseParams>,
) -> impl IntoResponse {
    let client_ip = extract_client_ip(&headers, &addr);

    let page = params.page.unwrap_or(0);
    let page_size = 20;
    let summaries = db::fetch_browse_page(&app.db, page, page_size)
        .await
        .unwrap_or_default();
    let has_next = summaries.len() == page_size as usize;

    let mut items = Vec::new();
    for s in summaries {
        let summary_html = render_markdown_to_html(&s.summary);
        let timestamps_html = if s.timestamps_done {
            let html = render_markdown_to_html(&s.timestamped_summary_in_youtube_format);
            replace_timestamps_in_html(&html, &s.original_source_link)
        } else {
            String::new()
        };
        let rating_stats = db::fetch_rating_stats(&app.db, s.identifier, Some(&client_ip))
            .await
            .unwrap_or_default();

        items.push(BrowseSummaryItem {
            identifier: s.identifier,
            model: s.model,
            cost: s.cost,
            original_source_link: s.original_source_link,
            summary_html,
            timestamps_html,
            rating_stats,
        });
    }

    let template = BrowseTemplate {
        summaries: items,
        page,
        has_next,
    };
    render_template(&template)
}

/// POST /summaries/{identifier}/rate — rate a summary or article content (1-5 stars).
pub async fn submit_rating(
    State(app): State<AppState>,
    Path(identifier): Path<i64>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Form(input): Form<SubmitRatingForm>,
) -> impl IntoResponse {
    let client_ip = extract_client_ip(&headers, &addr);

    // Verify summary exists
    let summary = db::fetch_summary(&app.db, identifier).await.ok().flatten();
    if summary.is_none() {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Html("<p>Summary not found.</p>".to_string()),
        )
            .into_response();
    }

    if let Err(e) = db::upsert_rating(
        &app.db,
        identifier,
        &client_ip,
        input.summary_rating,
        input.content_rating,
    )
    .await
    {
        tracing::error!("Failed to submit rating: {e}");
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Html(format!("<p>Error submitting rating: {e}</p>")),
        )
            .into_response();
    }

    let rating_stats = db::fetch_rating_stats(&app.db, identifier, Some(&client_ip))
        .await
        .unwrap_or_default();

    let template = RatingPartialTemplate {
        identifier,
        rating_stats,
    };
    render_template(&template).into_response()
}

/// POST /search — similarity search endpoint using embeddings.
pub async fn search_similar(
    State(app): State<AppState>,
    Form(query): Form<SearchForm>,
) -> impl IntoResponse {
    let embedding_svc = EmbeddingService::new(
        app.gemini_api_key.clone(),
        "gemini-embedding-001",
        3072,
    );

    let results = match embedding_svc.embed_text(&query.query).await {
        Ok(query_embedding) => {
            match embedding_svc
                .find_similar(&app.db, &query_embedding, 10)
                .await
            {
                Ok(similar) => {
                    let mut full_results = Vec::new();
                    for (id, score) in similar {
                        if let Ok(Some(summary)) = db::fetch_summary(&app.db, id).await {
                            let summary_html = render_markdown_to_html(&summary.summary);
                            full_results.push(SearchResultItem {
                                identifier: summary.identifier,
                                model: summary.model,
                                score,
                                summary_html,
                                original_source_link: summary.original_source_link,
                            });
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
    render_template(&template)
}

/// Helper to render the generation partial for a given identifier.
async fn render_generation_partial(app: &AppState, identifier: i64) -> Html<String> {
    let summary = db::fetch_summary(&app.db, identifier).await.ok().flatten();

    match summary {
        Some(s) => {
            let timestamps_html = if s.timestamps_done {
                let html = render_markdown_to_html(
                    &s.timestamped_summary_in_youtube_format,
                );
                replace_timestamps_in_html(
                    &html,
                    &s.original_source_link,
                )
            } else {
                String::new()
            };

            // Render markdown summary as HTML
            let summary_html = render_markdown_to_html(&s.summary);

            let template = GenerationPartialTemplate {
                identifier: s.identifier,
                summary: summary_html,
                summary_done: s.summary_done,
                timestamps: timestamps_html,
            };
            render_template(&template)
        }
        None => Html("<p>Summary not found.</p>".to_string()),
    }
}
