use askama::Template;
use axum::{
    extract::{ConnectInfo, Form, Path, Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse},
};
use chrono::Utc;
use std::net::SocketAddr;

use crate::db;
use crate::models::{BrowseParams, SearchForm, SubmitForm, SubmitRatingForm};
use crate::services::embedding::EmbeddingService;
use crate::services::rate_limiter::RateLimiter;
use crate::state::AppState;
use crate::tasks;
use crate::templates::{
    BrowseSummaryItem, BrowseTemplate, GenerationPartialTemplate, IndexTemplate,
    RatingPartialTemplate, SearchResultItem, SearchResultsTemplate,
};
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
    Form(input): Form<SubmitForm>,
) -> impl IntoResponse {
    // Find the model option
    let model = app.model_options.iter().find(|m| m.name == input.model);
    let model = match model {
        Some(m) => m.clone(),
        None => return Html("<p>Invalid model selected.</p>".to_string()),
    };

    // Check rate limit
    let allowed =
        RateLimiter::check_rate_limit(&model, &app.model_counts, &app.last_reset_day).await;
    if !allowed {
        return Html(
            "<p>Rate limit exceeded for this model. Please try again later.</p>".to_string(),
        );
    }

    // Check at least one of original_source_link or transcript is provided
    let url_empty = input.original_source_link.trim().is_empty();
    let transcript_empty = input
        .transcript
        .as_ref()
        .is_none_or(|t| t.trim().is_empty());
    if url_empty && transcript_empty {
        return Html(
            "<p>Error: Please provide either a YouTube URL, Hacker News URL, or paste content.</p>"
                .to_string(),
        );
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
    // Create items to process individually
    let items_to_process: Vec<(String, Option<String>)> = if normalized_links.is_empty() {
        vec![(String::new(), input.transcript.clone())]
    } else {
        normalized_links
            .into_iter()
            .enumerate()
            .map(|(idx, link)| {
                let transcript_for_link = if idx == 0 {
                    input.transcript.clone()
                } else {
                    None
                };
                (link, transcript_for_link)
            })
            .collect()
    };

    let mut html_results = Vec::new();

    for (link, transcript) in items_to_process {
        let single_input = SubmitForm {
            original_source_link: link,
            transcript,
            model: input.model.clone(),
            google_search_grounding: input.google_search_grounding,
            url_context: input.url_context,
        };

        // Check for duplicates using state's DeduplicationService
        let mut duplicate_id = None;

        if let Some(ref transcript_str) = single_input.transcript {
            let trimmed_transcript = transcript_str.trim();
            if !trimmed_transcript.is_empty() {
                if let Ok(Some(existing_id)) = app
                    .dedup_service
                    .check_duplicate_by_transcript(&app.db, trimmed_transcript, &single_input.model)
                    .await
                {
                    duplicate_id = Some(existing_id);
                }
            }
        }

        if duplicate_id.is_none() && !single_input.original_source_link.trim().is_empty() {
            if let Ok(Some(existing_id)) = app
                .dedup_service
                .check_duplicate(
                    &app.db,
                    single_input.original_source_link.trim(),
                    &single_input.model,
                )
                .await
            {
                duplicate_id = Some(existing_id);
            }
        }

        let item_id = if let Some(existing_id) = duplicate_id {
            existing_id
        } else {
            // Insert new row
            let timestamp_start = Utc::now().to_rfc3339();
            let new_id = match db::insert_new_summary(
                &app.db,
                &single_input,
                &addr.to_string(),
                &timestamp_start,
            )
            .await
            {
                Ok(id) => id,
                Err(e) => return Html(format!("<p>Error: {}</p>", e)),
            };

            // Spawn background task
            let app_clone = app.clone();
            let db_clone = app.db.clone();
            tokio::spawn(async move {
                tasks::process_summary(db_clone, new_id, app_clone).await;
            });

            new_id
        };

        let partial_html = render_generation_partial(&app, item_id).await;
        html_results.push(partial_html.0);
    }

    Html(html_results.join("\n"))
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
    let embedding_svc =
        EmbeddingService::new(app.gemini_api_key.clone(), "gemini-embedding-001", 3072);

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
                let html = render_markdown_to_html(&s.timestamped_summary_in_youtube_format);
                replace_timestamps_in_html(&html, &s.original_source_link)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    async fn build_test_state() -> AppState {
        let db_pool = db::init_db("sqlite::memory:")
            .await
            .expect("Failed to init in-memory DB for route tests");

        let model = crate::state::ModelOption {
            name: "gemini-3.5-flash".to_string(),
            input_price_per_mtoken: 0.10,
            output_price_per_mtoken: 0.40,
            context_window: 1_000_000,
            rpm_limit: 5,
            rpd_limit: 20,
            architecture: crate::state::ModelArchitecture::Gemini,
        };

        AppState {
            db: db_pool,
            model_options: Arc::new(vec![model]),
            model_counts: Arc::new(RwLock::new(HashMap::new())),
            last_reset_day: Arc::new(RwLock::new(None)),
            gemini_api_key: "dummy_key".to_string(),
            nn_mapper: None,
            viz_data: None,
            model_locks: Arc::new(RwLock::new(HashMap::new())),
            dedup_service: crate::services::deduplication::DeduplicationService::new(
                std::time::Duration::from_secs(300),
            ),
            download_limiter: Arc::new(
                crate::services::download_limiter::DownloadLimiter::from_env(),
            ),
        }
    }

    #[tokio::test]
    async fn test_process_transcript_multi_url_splitting() {
        let state = build_test_state().await;
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let input = SubmitForm {
            original_source_link: "https://www.youtube.com/watch?v=dQw4w9WgXcQ https://news.ycombinator.com/item?id=40000000".to_string(),
            transcript: None,
            model: "gemini-3.5-flash".to_string(),
            google_search_grounding: false,
            url_context: false,
        };

        let response = process_transcript(State(state.clone()), ConnectInfo(addr), Form(input))
            .await
            .into_response();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let html = String::from_utf8(body_bytes.to_vec()).unwrap();

        // Verify HTML output contains polling partials for both generated IDs
        assert!(
            html.contains("id=\"generation-1\""),
            "Should contain partial for ID 1: {}",
            html
        );
        assert!(
            html.contains("id=\"generation-2\""),
            "Should contain partial for ID 2: {}",
            html
        );

        // Verify database contains 2 distinct rows with individual links
        let row1 = db::fetch_summary(&state.db, 1).await.unwrap().unwrap();
        let row2 = db::fetch_summary(&state.db, 2).await.unwrap().unwrap();

        assert_eq!(
            row1.original_source_link,
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        );
        assert_eq!(
            row2.original_source_link,
            "https://news.ycombinator.com/item?id=40000000"
        );
    }

    #[tokio::test]
    async fn test_process_transcript_multi_url_deduplication() {
        let state = build_test_state().await;
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();

        // First submission: single URL
        let input1 = SubmitForm {
            original_source_link: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
            transcript: None,
            model: "gemini-3.5-flash".to_string(),
            google_search_grounding: false,
            url_context: false,
        };
        let _ = process_transcript(State(state.clone()), ConnectInfo(addr), Form(input1)).await;

        // Second submission: multi-URL with 1 existing and 1 new link
        let input2 = SubmitForm {
            original_source_link: "https://www.youtube.com/watch?v=dQw4w9WgXcQ https://news.ycombinator.com/item?id=40000000".to_string(),
            transcript: None,
            model: "gemini-3.5-flash".to_string(),
            google_search_grounding: false,
            url_context: false,
        };
        let response = process_transcript(State(state.clone()), ConnectInfo(addr), Form(input2))
            .await
            .into_response();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let html = String::from_utf8(body_bytes.to_vec()).unwrap();

        // Should return existing ID 1 for duplicate link and new ID 2 for new link
        assert!(html.contains("id=\"generation-1\""));
        assert!(html.contains("id=\"generation-2\""));

        // Only 2 total rows should exist in database
        let row3 = db::fetch_summary(&state.db, 3).await.unwrap();
        assert!(
            row3.is_none(),
            "Row 3 should not exist due to deduplication of item 1"
        );
    }

    #[tokio::test]
    async fn test_process_transcript_invalid_url_rejection() {
        let state = build_test_state().await;
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let input = SubmitForm {
            original_source_link: "not_a_valid_link".to_string(),
            transcript: None,
            model: "gemini-3.5-flash".to_string(),
            google_search_grounding: false,
            url_context: false,
        };

        let response = process_transcript(State(state.clone()), ConnectInfo(addr), Form(input))
            .await
            .into_response();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let html = String::from_utf8(body_bytes.to_vec()).unwrap();

        assert!(html.contains(
            "Error: The provided value 'not_a_valid_link' is neither a valid YouTube URL"
        ));
        assert!(db::fetch_summary(&state.db, 1).await.unwrap().is_none());
    }
}
