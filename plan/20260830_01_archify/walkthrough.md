# Walkthrough: Archify Diagrams for `rs-summarizer`

This document explains the architecture, workflow, and data flow diagrams generated for the `rs-summarizer` repository using [tt-a1i/archify](https://github.com/tt-a1i/archify).

All diagrams were validated and delivered with the **`showcase`** quality profile (0 composition errors, 0 warnings, passing all 9 artifact and geometry checks).

---

## Generated Diagrams Overview

| Diagram | Type | Specification | Rendered Artifact | Description |
|---|---|---|---|---|
| **System Architecture** | `architecture` | `architecture.json` | `architecture.html` | High-level system structure, web server, async background tasks, rate limiter, persistence, LLM inference providers, and visualization subsystem. |
| **Summarization Workflow** | `workflow` (v2) | `summarization_workflow.json` | `summarization_workflow.html` | Step-by-step request lifecycle: deduplication fast-path, transcript extraction (YouTube/HN), rate-guarding, multi-model fallback chain, formatting, and live HTMX DOM updates. |
| **Embedding & Viz Dataflow** | `dataflow` | `embedding_pipeline.json` | `embedding_pipeline.html` | End-to-end vector pipeline: 3072d Gemini text embeddings, compact SQLite export, accelerated UMAP 2D projection, DBSCAN clustering, LLM cluster titling, and real-time query serving. |

---

## 1. System Architecture (`architecture.json` / `architecture.html`)

### Key Components & Boundaries
- **Core Server (`rs-summarizer`)**:
  - **HTMX Web Client (`ui`)**: Dynamic frontend using Askama templates and HTMX polling partials.
  - **Axum Web Server (`axum`)**: Tokio async HTTP router running on `:5001`, managing shared `AppState`.
  - **Deduplication Service (`dedup`)**: In-memory cache and DB index checker preventing duplicate processing of identical URLs and transcript text.
  - **Task Orchestrator (`worker`)**: Tokio background task runner spawned on intake.
  - **Rate Limiter (`ratelimit`)**: Tracks RPM/RPD limits and enforces concurrency locks per LLM model.
  - **SQLite Storage (`db`)**: Local `summaries.db` running in WAL mode with support for transcripts, summaries, ratings, and raw vector BLOBs.
  - **Viz Tool Subsystem (`viz_tool`)**: Standalone crate combining `fast-umap`, `dbscan_engine`, and `egui` desktop GUI.
- **External Providers Boundary**:
  - **YouTube Subtitles (`ytdlp`)**: Subtitle download via `yt-dlp` CLI and `vtt` crate parser.
  - **Hacker News API (`hn_api`)**: Firebase REST API for fetching story items and nested comment threads.
  - **Google Gemini API (`gemini`)**: LLM summarization and 3072-dimensional text embeddings (`gemini-embedding-001`).
  - **Hetzner vLLM Cluster (`hetzner`)**: OpenAI-compatible endpoints hosting open-source models (Qwen 3.6 35B, DeepSeek v4 Flash, GLM 5.2, Kimi K2.7).

### Curated Guided Views
1. `ingestion-path`: Focuses on UI submission, deduplication, worker task dispatch, and SQLite state.
2. `extraction-and-llm`: Highlights external ingestion from YouTube/HN and multi-model inference.
3. `analytics-viz`: Covers vector storage, UMAP projection, and runtime 2D mapping.

---

## 2. Summarization & Fallback Workflow (`summarization_workflow.json` / `summarization_workflow.html`)

### Pipeline Phases
1. **Intake & Deduplication**:
   - Accepts single or space-separated multiple URLs / pasted transcripts.
   - Splits and normalizes URLs (`YouTube`, `HackerNews`, `Unknown`).
   - **Fast Path (Cache Hit)**: If the URL or transcript hash already exists in SQLite, returns the existing record immediately (`cached_return`).
   - **Async Path**: Inserts a pending database row and spawns `tasks::process_summary`.
2. **Extraction & Rate Limiting**:
   - `ytdlp` fetches subtitle streams and parses cues; fallback to video description if subtitles are absent.
   - Hacker News parser traverses story details and comment trees.
   - `RateLimiter` verifies RPM and daily RPD quotas and acquires an active model lock.
3. **Inference & Delivery**:
   - Invokes the selected primary model (`gemini-3.5-flash`, `gemini-3.7-flash`, `hetzner-qwen-3.6-35b`, etc.).
   - On 429 quota exhaustion or transient 503 errors, automatically steps through the configured model fallback chain.
   - Post-processes output: converts YouTube timestamps to clickable links (`timestamp_linker.rs`), renders markdown to HTML, and computes embeddings.
   - Commits summary and embedding to SQLite; HTMX polling triggers DOM update on client.

---

## 3. Embedding & Visualization Pipeline (`embedding_pipeline.json` / `embedding_pipeline.html`)

### Data Stages
1. **Ingest & Persist**:
   - Gemini embedding model produces 3072-dimensional dense float vectors.
   - Stored in SQLite `summaries` table as binary BLOBs.
   - `export-db` CLI command creates memory-mapped compact DB files.
2. **Extract & Truncate**:
   - `data_loader` deserializes raw BLOB bytes into `f32` vectors with configurable dimension truncation.
   - `nn_descent` builds k-NN sparse connectivity graphs across embeddings using Rayon multi-threading.
3. **Project & Cluster**:
   - `fast-umap` projects high-dimensional graphs to 2D coordinate space using GPU shaders (WGPU) or CPU fallback.
   - `dbscan_engine` identifies dense clusters and filters out noise points.
   - `cluster_titler` sends sampled cluster items to Gemini/Gemma to generate semantic cluster titles.
4. **Explore & Serve**:
   - `viz_gui` (`egui`/`eframe`) provides an interactive scatter plot with zoom/pan and cluster inspection.
   - 2D coordinates, centroids, and titles are exported to `AppState::viz_data` and `NnMapper` in the Axum web server.
   - Powers real-time vector similarity search (`POST /search`) and 2D map browsing.

---

## How to View and Inspect the Diagrams

The generated HTML files are standalone, self-contained interactive viewers:
- **`plan/20260830_01_archify/architecture.html`**
- **`plan/20260830_01_archify/summarization_workflow.html`**
- **`plan/20260830_01_archify/embedding_pipeline.html`**

Open any HTML file in a web browser to use:
- **Theme toggle**: Light and dark mode support.
- **Guided views**: Select pre-configured architectural perspectives using the chapter buttons.
- **Interactive canvas**: Pan, zoom, highlight connected relationships, and inspect node metadata.
