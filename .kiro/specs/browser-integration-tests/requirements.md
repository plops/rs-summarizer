# Requirements Document

## Introduction

This document specifies requirements for comprehensive browser integration tests for the rs-summarizer application. The tests extend the existing fantoccini-based test suite (`tests/integration_browser.rs`) with additional coverage for HTMX behavior, browse page functionality, search, concurrency/resilience, and accessibility. All tests use headless Firefox via geckodriver and are `#[ignore]` by default.

## Glossary

- **Test_Harness**: The shared test infrastructure that starts geckodriver, connects a headless Firefox browser, and spins up an in-memory test server instance
- **Browser_Client**: A fantoccini WebDriver client connected to headless Firefox
- **Test_Server**: An axum application instance bound to a random port with an in-memory SQLite database
- **Deduplication_Service**: The service that detects duplicate URL+model submissions within a 5-minute window
- **Rate_Limiter**: The per-model daily request counter that rejects requests when the limit is reached
- **HTMX_Polling**: The client-side mechanism where `hx-trigger="every 1s"` causes repeated POST requests to `/generations/{id}` until `summary_done` is true
- **Browse_Page**: The `/browse` endpoint that displays paginated summaries (20 per page)
- **Search_Endpoint**: The `POST /search` endpoint that uses Gemini embeddings for similarity search
- **Generation_Partial**: The HTML partial rendered by `generation_partial.html` that shows summary progress or completion

## Requirements

### Requirement 1: Duplicate Submission Detection

**User Story:** As a test author, I want to verify that submitting the same URL twice within 5 minutes returns the existing result instead of creating a new generation, so that the deduplication logic is confirmed to work end-to-end in the browser.

#### Acceptance Criteria

1. WHEN the Browser_Client submits a URL and model combination via the form, AND WHEN the Browser_Client submits the same URL and model combination again within 5 minutes, THE Test_Harness SHALL verify that the second submission returns a Generation_Partial with the same identifier as the first submission
2. WHEN the Browser_Client submits a duplicate URL, THE Generation_Partial SHALL render without triggering a new background summarization task

### Requirement 2: Rate Limit Display

**User Story:** As a test author, I want to verify that exhausting a model's daily rate limit causes the UI to display an appropriate error message, so that rate limiting is confirmed to work in the browser.

#### Acceptance Criteria

1. WHEN the Browser_Client submits requests until the model's `rpd_limit` is reached, AND WHEN the Browser_Client submits one additional request, THE Test_Server SHALL return an HTML response containing the text "Rate limit exceeded"
2. WHEN the rate limit error is displayed, THE Generation_Partial SHALL NOT contain an `hx-post` attribute for polling

### Requirement 3: Polling Stops on Error

**User Story:** As a test author, I want to verify that when a submission results in an error, the HTMX polling spinner stops and an error message is displayed, so that error states are handled gracefully in the UI.

#### Acceptance Criteria

1. WHEN the Browser_Client submits a form with an invalid model value injected via JavaScript, THE Test_Server SHALL return an error message within the `#result` div
2. WHEN an error message is displayed in the `#result` div, THE `#generation` element SHALL NOT be present with an `hx-trigger` attribute

### Requirement 4: Form Validation

**User Story:** As a test author, I want to verify that the browser's native `required` attribute prevents empty form submissions, so that client-side validation is confirmed to work.

#### Acceptance Criteria

1. WHEN the Browser_Client clicks the submit button without entering a URL, THE Browser_Client SHALL remain on the index page without triggering an HTMX request
2. THE URL input field SHALL have the `required` attribute present in the DOM

### Requirement 5: Browse Page Pagination

**User Story:** As a test author, I want to verify that the browse page correctly paginates summaries with 20 items per page and functional navigation links, so that pagination logic is confirmed end-to-end.

#### Acceptance Criteria

1. WHEN the Test_Server database contains 25 summary records, AND WHEN the Browser_Client navigates to `/browse`, THE Browse_Page SHALL display exactly 20 summary article elements
2. WHEN the Browse_Page displays page 0 with 25 total records, THE Browse_Page SHALL contain a "Next →" link pointing to `/browse?page=1`
3. WHEN the Browser_Client navigates to `/browse?page=1`, THE Browse_Page SHALL display exactly 5 summary article elements
4. WHEN the Browser_Client is on page 1, THE Browse_Page SHALL contain a "← Previous" link pointing to `/browse?page=0`
5. WHEN the Browser_Client is on the last page, THE Browse_Page SHALL NOT contain a "Next →" link

### Requirement 6: Summary Content Rendering

**User Story:** As a test author, I want to verify that markdown content in summaries is rendered as proper HTML on the browse and generation pages, so that the markdown rendering pipeline is confirmed to work end-to-end.

#### Acceptance Criteria

1. WHEN a summary record contains markdown bold syntax (`**text**`), THE Generation_Partial SHALL render it as an HTML `<strong>` element
2. WHEN a summary record contains markdown list syntax, THE Generation_Partial SHALL render it as HTML `<ul>` or `<ol>` elements
3. WHEN a summary record contains markdown heading syntax (`## Heading`), THE Generation_Partial SHALL render it as an HTML heading element (e.g., `<h2>`)

### Requirement 7: Timestamp Links

**User Story:** As a test author, I want to verify that timestamps in completed summaries are rendered as clickable YouTube links with the `&t=` parameter, so that the timestamp linking utility is confirmed to work in the browser.

#### Acceptance Criteria

1. WHEN a summary record has `timestamps_done` set to true and contains timestamped content, THE Generation_Partial SHALL render timestamps as `<a>` elements
2. WHEN a timestamp link is rendered, THE link `href` attribute SHALL contain the original YouTube URL with a `&t=` parameter representing the timestamp in seconds

### Requirement 8: Search Returns Ranked Results

**User Story:** As a test author, I want to verify that the search endpoint returns results ordered by embedding similarity, so that the vector search pipeline is confirmed to work end-to-end in the browser.

#### Acceptance Criteria

1. WHEN the Test_Server database contains summaries with pre-computed embeddings, AND WHEN the Browser_Client submits a search query related to the seeded content, THE `#search-results` div SHALL contain one or more result entries
2. WHEN search results are displayed, THE results SHALL appear in descending order of relevance score

### Requirement 9: Empty Search Results

**User Story:** As a test author, I want to verify that searching for nonsensical terms renders a clean empty state, so that the UI handles zero-result searches gracefully.

#### Acceptance Criteria

1. WHEN the Browser_Client submits a search query that matches no embeddings, THE `#search-results` div SHALL render without error
2. WHEN no search results are found, THE `#search-results` div SHALL NOT contain any result entry elements

### Requirement 10: Multiple Simultaneous Submissions

**User Story:** As a test author, I want to verify that two concurrent form submissions from separate browser sessions each receive their own independent polling partials, so that concurrency isolation is confirmed.

#### Acceptance Criteria

1. WHEN two Browser_Client instances submit different URLs at the same time, THE Test_Server SHALL create two separate summary records with distinct identifiers
2. WHEN two concurrent submissions are in progress, EACH Browser_Client SHALL receive a Generation_Partial with its own unique identifier for polling

### Requirement 11: Server Restart Mid-Poll

**User Story:** As a test author, I want to verify that if the server restarts while a client is polling for a summary, the page recovers gracefully without a permanent error state, so that resilience is confirmed.

#### Acceptance Criteria

1. WHEN the Browser_Client is polling for a generation, AND WHEN the Test_Server is stopped and restarted, THE Browser_Client SHALL eventually receive a valid HTTP response when polling resumes
2. IF the Test_Server restarts and the summary record is lost (in-memory DB), THEN THE Browser_Client SHALL display a "Summary not found" message rather than an unhandled error

### Requirement 12: Aria-Busy Attribute During Loading

**User Story:** As a test author, I want to verify that the `aria-busy` attribute is present during summary generation and removed upon completion, so that assistive technologies can communicate loading state to users.

#### Acceptance Criteria

1. WHILE a summary is being generated, THE Generation_Partial SHALL contain an element with `aria-busy="true"`
2. WHEN the summary generation is complete, THE Generation_Partial SHALL NOT contain any element with `aria-busy="true"`

### Requirement 13: Form Input Labels

**User Story:** As a test author, I want to verify that all form inputs have associated `<label>` elements, so that the application meets basic accessibility requirements.

#### Acceptance Criteria

1. THE index page SHALL have a `<label>` element with a `for` attribute matching the `id` of the URL input field
2. THE index page SHALL have a `<label>` element with a `for` attribute matching the `id` of the model select field
3. THE search input SHALL have an accessible name (via `placeholder`, `aria-label`, or associated `<label>`)

### Requirement 14: Keyboard Navigation

**User Story:** As a test author, I want to verify that the form can be navigated and submitted using only the keyboard, so that keyboard-only users can operate the application.

#### Acceptance Criteria

1. WHEN the Browser_Client uses Tab key navigation, THE focus SHALL move sequentially through the URL input, model select, and submit button
2. WHEN the URL input is focused and contains a value, AND WHEN the Browser_Client presses Enter, THE form SHALL submit via HTMX
