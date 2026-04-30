# Requirements Document

## Introduction

rs-summarizer is a Rust port of the Python-based RocketRecap YouTube video transcript summarizer. The system provides a web interface for submitting YouTube URLs, downloading captions via yt-dlp, generating AI-powered summaries using Google Gemini with streaming, computing vector embeddings for similarity search, and storing all data in SQLite with WAL mode. The frontend uses HTMX for real-time progressive updates during summary generation.

## Glossary

- **System**: The rs-summarizer web application as a whole
- **URL_Validator**: The module responsible for validating YouTube URLs and extracting video IDs
- **VTT_Parser**: The module responsible for parsing WebVTT subtitle files into plain text with timestamps
- **Transcript_Service**: The service that downloads and parses YouTube video transcripts using yt-dlp
- **Summary_Service**: The service that orchestrates AI summary generation via Gemini with streaming support
- **Embedding_Service**: The service that computes vector embeddings and performs similarity search
- **Deduplication_Service**: The service that prevents duplicate submissions within a configurable time window
- **Markdown_Converter**: The module that converts markdown-formatted summaries to YouTube-compatible format
- **Timestamp_Linker**: The module that converts HTML timestamps into clickable YouTube links
- **Background_Task**: A tokio-spawned async task that processes a summarization pipeline
- **Metadata_Cache**: An in-memory cache of summary metadata for fast browse and filter operations
- **Matryoshka_Embedding**: An embedding vector that retains semantic meaning when truncated to fewer dimensions

## Requirements

### Requirement 1: YouTube URL Validation

**User Story:** As a user, I want to submit YouTube URLs in various formats, so that the system can extract the video ID regardless of how I copied the link.

#### Acceptance Criteria

1. WHEN a valid YouTube watch URL is provided (e.g., `https://www.youtube.com/watch?v=ID`), THE URL_Validator SHALL return the 11-character video ID
2. WHEN a valid YouTube live URL is provided (e.g., `https://www.youtube.com/live/ID`), THE URL_Validator SHALL return the 11-character video ID
3. WHEN a valid YouTube short URL is provided (e.g., `https://youtu.be/ID`), THE URL_Validator SHALL return the 11-character video ID
4. WHEN a valid YouTube shorts URL is provided (e.g., `https://www.youtube.com/shorts/ID`), THE URL_Validator SHALL return the 11-character video ID
5. WHEN a valid YouTube mobile URL is provided (e.g., `https://m.youtube.com/watch?v=ID`), THE URL_Validator SHALL return the 11-character video ID
6. WHEN a URL with additional query parameters is provided, THE URL_Validator SHALL extract only the 11-character video ID and ignore extra parameters
7. WHEN a non-HTTPS YouTube URL is provided (e.g., `http://`), THE URL_Validator SHALL reject the URL and return None
8. WHEN a non-YouTube URL is provided, THE URL_Validator SHALL reject the URL and return None
9. THE URL_Validator SHALL only return video IDs consisting of exactly 11 characters from the set [A-Za-z0-9_-]

### Requirement 2: VTT Subtitle Parsing

**User Story:** As a user, I want the system to parse downloaded subtitle files into readable text, so that the transcript can be used for summarization.

#### Acceptance Criteria

1. WHEN a valid WebVTT file is provided, THE VTT_Parser SHALL produce output lines in the format "HH:MM:SS caption_text\n"
2. WHEN consecutive duplicate caption lines appear in the VTT input, THE VTT_Parser SHALL collapse them into a single entry
3. THE VTT_Parser SHALL truncate all timestamps to second granularity, removing millisecond components
4. WHEN the VTT file contains multi-line cues, THE VTT_Parser SHALL use only the last line of each cue as the caption text
5. THE VTT_Parser SHALL produce output that matches the Python implementation byte-for-byte for the same input

### Requirement 3: Markdown to YouTube Format Conversion

**User Story:** As a user, I want summaries formatted for YouTube's comment/description syntax, so that I can paste them directly into YouTube.

#### Acceptance Criteria

1. WHEN markdown bold markers (`**word**`) are present, THE Markdown_Converter SHALL convert them to YouTube bold format (`*word*`)
2. WHEN markdown heading markers (`## Heading`) are present, THE Markdown_Converter SHALL convert them to YouTube bold format (`*Heading*`)
3. WHEN URLs containing dots are present in the text, THE Markdown_Converter SHALL replace dots in URLs with `-dot-` to avoid YouTube link censoring
4. WHEN punctuation is adjacent to bold markers (e.g., `**:`), THE Markdown_Converter SHALL reposition the punctuation outside the converted markers
5. THE Markdown_Converter SHALL produce output containing no remaining markdown-style double-asterisk bold markers

### Requirement 4: HTML Timestamp to YouTube Link Conversion

**User Story:** As a user, I want timestamps in the rendered summary to be clickable links that jump to the correct position in the YouTube video.

#### Acceptance Criteria

1. WHEN a valid YouTube URL and HTML containing MM:SS timestamps are provided, THE Timestamp_Linker SHALL wrap each timestamp in an anchor tag linking to the video at the corresponding time offset
2. WHEN a valid YouTube URL and HTML containing HH:MM:SS timestamps are provided, THE Timestamp_Linker SHALL wrap each timestamp in an anchor tag with the correct computed seconds offset
3. WHEN the YouTube URL contains existing query parameters (e.g., `?t=100`), THE Timestamp_Linker SHALL use only the canonical `watch?v=ID` form in generated links
4. WHEN an invalid YouTube URL is provided, THE Timestamp_Linker SHALL return the HTML unchanged with no modifications
5. WHEN multiple timestamps appear in the HTML, THE Timestamp_Linker SHALL convert all of them to clickable links

### Requirement 5: Transcript Download

**User Story:** As a user, I want the system to automatically download video transcripts, so that I don't have to manually copy and paste them.

#### Acceptance Criteria

1. WHEN a valid YouTube URL is submitted, THE Transcript_Service SHALL invoke yt-dlp to list available subtitle languages
2. WHEN multiple subtitle languages are available, THE Transcript_Service SHALL select the best language using priority ordering: (1) `-orig` languages matching preferred base order, (2) any `-orig` language sorted, (3) non-orig matching preferred base, (4) any `en*` prefix, (5) first sorted language
3. WHEN the selected language is determined, THE Transcript_Service SHALL download the VTT subtitle file and parse it into plain text
4. IF no subtitles are available for the video, THEN THE Transcript_Service SHALL return a NoSubtitles error
5. IF yt-dlp execution fails or times out, THEN THE Transcript_Service SHALL return an appropriate error with a descriptive message
6. THE Transcript_Service SHALL store temporary VTT files in `/dev/shm` (tmpfs) and clean them up after parsing, including on error paths

### Requirement 6: AI Summary Generation

**User Story:** As a user, I want AI-generated summaries of video transcripts, so that I can quickly understand video content without watching the full video.

#### Acceptance Criteria

1. WHEN a transcript is ready for summarization, THE Summary_Service SHALL stream the response from Gemini and persist each chunk to the database progressively
2. WHILE streaming is in progress, THE Summary_Service SHALL ensure the database summary field contains the concatenation of all chunks received so far (monotonically growing)
3. WHEN streaming completes successfully, THE Summary_Service SHALL set `summary_done` to true and record token counts, cost, and timestamps
4. WHEN the summary is complete, THE Summary_Service SHALL convert it to YouTube format and set `timestamps_done` to true
5. IF the transcript contains fewer than 30 words, THEN THE Summary_Service SHALL reject it with a TranscriptTooShort error
6. IF the transcript exceeds 280,000 words, THEN THE Summary_Service SHALL reject it with a TranscriptTooLong error
7. IF the Gemini API returns a rate-limiting error (ResourceExhausted), THEN THE Summary_Service SHALL append the error to the partial summary without setting `summary_done` to true

### Requirement 7: Vector Embedding and Similarity Search

**User Story:** As a user, I want to find summaries similar to a search query, so that I can discover related video content.

#### Acceptance Criteria

1. WHEN a summary is successfully generated, THE Embedding_Service SHALL compute a vector embedding via the Gemini embedding model
2. THE Embedding_Service SHALL store embeddings as raw f32 byte blobs in SQLite with byte length equal to dimensions × 4
3. WHEN computing cosine similarity between vectors of different dimensions (Matryoshka embeddings), THE Embedding_Service SHALL truncate both vectors to the shorter length before computation
4. THE Embedding_Service SHALL produce cosine similarity values in the range [-1.0, 1.0] for all non-empty vector pairs
5. WHEN a zero-magnitude vector is encountered, THE Embedding_Service SHALL return a similarity of 0.0
6. WHEN a similarity search is performed, THE Embedding_Service SHALL return the top-k most similar summaries ranked by cosine similarity score
7. IF embedding computation fails after a successful summary, THEN THE Embedding_Service SHALL log a warning and leave the embedding as NULL without failing the overall job

### Requirement 8: Deduplication

**User Story:** As a user, I want the system to detect duplicate submissions, so that redundant processing is avoided and I see existing results immediately.

#### Acceptance Criteria

1. WHEN a URL and model combination matches an existing entry within the 5-minute time window, THE Deduplication_Service SHALL return the existing entry's identifier
2. WHEN a transcript and model combination matches an existing entry within the time window, THE Deduplication_Service SHALL return the existing entry's identifier
3. WHEN no matching entry exists within the time window, THE Deduplication_Service SHALL return None, allowing a new submission to proceed
4. THE Deduplication_Service SHALL use the `summary_timestamp_start` field for time window comparison

### Requirement 9: Background Task Processing

**User Story:** As a user, I want summarization to happen in the background, so that the web interface remains responsive during long-running operations.

#### Acceptance Criteria

1. WHEN a new submission is accepted, THE System SHALL insert a new database row with status pending and spawn a background task via `tokio::spawn`
2. WHEN the background task starts, THE System SHALL wait for the row to be readable (retry with backoff, max 400 attempts at 100ms intervals)
3. WHEN the background task completes successfully, THE System SHALL ensure `summary_done == true`, `timestamps_done == true`, and `cost > 0`
4. IF the background task encounters an error, THEN THE System SHALL store the error message in the summary field and set `summary_done` to true
5. IF embedding computation fails, THEN THE System SHALL still mark the overall task as successful with `summary_done` and `timestamps_done` both true

### Requirement 10: Web Interface and HTMX Polling

**User Story:** As a user, I want real-time updates as my summary is being generated, so that I can see progress without manually refreshing.

#### Acceptance Criteria

1. WHEN a submission is accepted, THE System SHALL return an HTML partial containing an HTMX polling div that polls every 1 second
2. WHEN the polling endpoint is called for an in-progress summary, THE System SHALL return the current partial summary as an HTML fragment
3. WHEN the summary is complete (`summary_done == true`), THE System SHALL return the final summary and stop HTMX polling
4. THE System SHALL serve a main page with a submission form accepting YouTube URLs and model selection
5. THE System SHALL serve a browse page with paginated summaries (20 per page) ordered by most recent first
6. THE System SHALL serve a search endpoint that accepts a text query and returns similar summaries ranked by embedding similarity

### Requirement 11: Database and Storage

**User Story:** As a developer, I want reliable concurrent database access, so that polling reads don't block background task writes.

#### Acceptance Criteria

1. THE System SHALL use SQLite with WAL (Write-Ahead Logging) journal mode to enable concurrent reads during writes
2. THE System SHALL use sqlx with a configurable connection pool (default: 5 connections)
3. THE System SHALL use parameterized query bindings for all SQL operations to prevent injection
4. THE System SHALL maintain a composite index on `(original_source_link, model, summary_timestamp_start)` for O(log n) deduplication lookups
5. THE System SHALL run database migrations at startup via sqlx

### Requirement 12: In-Memory Metadata Cache

**User Story:** As a user, I want the browse and filter pages to load quickly, so that I can navigate summaries without delay.

#### Acceptance Criteria

1. WHEN the application starts, THE Metadata_Cache SHALL load all summary metadata (identifier, model, cost, link, timestamps, has_embedding flag) into memory
2. WHEN a new summary completes, THE Metadata_Cache SHALL be refreshed to include the new entry
3. WHEN browse or filter requests are received, THE System SHALL serve them from the Metadata_Cache without querying SQLite
4. THE Metadata_Cache SHALL group consecutive entries with identical summaries in the browse view for improved UX

### Requirement 13: Rate Limiting and Cost Control

**User Story:** As an operator, I want per-model daily request counters, so that API costs are controlled and don't exceed budget.

#### Acceptance Criteria

1. THE System SHALL maintain per-model request counters that track daily usage
2. WHEN a new calendar day begins (America/Los_Angeles timezone), THE System SHALL reset all model counters exactly once
3. WHEN a model's daily request count reaches its configured RPD limit, THE System SHALL reject new requests for that model until the next reset

### Requirement 14: Security and Input Safety

**User Story:** As an operator, I want the system to handle user input safely, so that it is protected against injection and data leakage.

#### Acceptance Criteria

1. THE System SHALL load the Gemini API key from an environment variable and never log or expose it in responses
2. THE System SHALL invoke yt-dlp with explicit argument arrays, avoiding shell interpolation
3. THE System SHALL render all HTML output through askama templates with auto-escaping enabled
4. THE System SHALL validate YouTube URLs via regex before passing them to any subprocess
