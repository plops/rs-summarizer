# Implementation Plan: Load Balancing & Concurrency Control for Downloads & yt-dlp

**Client**: Wol Pumba (`wolpumba@gmail.com`)  
**Project**: `plops/rs-summarizer`  
**Plan Directory**: `file:///workspace/src/rs-summarizer/plan/20260726_02_loadbalance/`  

---

## 1. Overview & Architecture Design

When 6 or more `yt-dlp` processes are launched concurrently on a 2 vCPU Hetzner server (2 vCPU, 2GB RAM), CPU utilization reaches 100%, causing severe system unresponsiveness and forcing manual reboots. Log traces show SQLite database query execution times spiking above threshold:

```text
WARN sqlx::query: slow statement: execution time exceeded alert threshold summary="SELECT * FROM summaries …" elapsed=2.537523309s
```

`yt-dlp` relies on Python execution to extract YouTube page metadata, parse stream manifests, and download subtitles. Running multiple Python processes simultaneously on 2 vCPUs overwhelms the host CPU and thread context switches, starving Tokio worker threads and SQLite queries.

### Load Balancing & Concurrency Governor Architecture

To prevent CPU exhaustion, we introduce a thread-safe **Download Limiter (`DownloadLimiter`)** service in `src/services/download_limiter.rs`:

1. **`yt-dlp` Semaphore & Concurrency Capping**:
   - Limit concurrent `yt-dlp` process execution to a safe maximum (default: **2** concurrent processes, configurable via environment variable `MAX_CONCURRENT_YTDLP`).
   - Uses `tokio::sync::Semaphore` to asynchronously queue tasks without blocking Tokio runtime worker threads.

2. **Hacker News & Article Download Semaphore**:
   - Limit concurrent external HTTP downloads (e.g. HackerNews submission API / comment fetching and external article HTML scraping) to a safe maximum (default: **2** concurrent downloads, configurable via `MAX_CONCURRENT_HN`).

3. **Inter-Task Delay Throttling (Staggered Execution)**:
   - Introduce a configurable inter-job delay between consecutive `yt-dlp` runs (default: **1000 ms**, configurable via `YTDLP_DELAY_MS`).
   - Introduce a configurable inter-job delay between HTTP article/HN downloads (default: **500 ms**, configurable via `DOWNLOAD_DELAY_MS`).
   - Prevents burst spikes in CPU usage and avoids rate limiting or IP bans from remote services (YouTube, HackerNews, article hosts).

4. **Integration with `AppState` & Task Pipeline**:
   - Add `pub download_limiter: Arc<DownloadLimiter>` to `AppState` in `src/state.rs`.
   - Update `tasks::fetch_youtube_content` / `TranscriptService` to acquire a `yt-dlp` permit from `app.download_limiter`.
   - Update `tasks::fetch_hn_content` / `HackerNewsService` to acquire a download permit from `app.download_limiter`.

---

## 2. Requirements Assessment & Recommendations

### User Requirements Checklist
- [x] Limit `yt-dlp` process concurrency to match server capacity (e.g. 1–2 concurrent processes on 2 vCPU VPS).
- [x] Introduce inter-download delay (`yt-dlp` and HN downloads) to prevent CPU spikes and remote server rate limits.
- [x] Utilize `cargo-workflow` skill tools (`cargo check`, `cargo fmt`, `cargo clippy`, `cargo test`).
- [x] Query DeepWiki MCP (`plops/rs-summarizer`) for architecture context.
- [x] Document dependencies in `deps.md`.
- [x] Support Ubuntu Docker environment and runtime configurable environment variables.
- [x] Provide autonomous AI agent File Context Map with descriptions.
- [x] Follow Conventional Commit format with comprehensive descriptions.
- [x] Define step-by-step serial task breakdown in `task.md`.
- [x] Implement unit tests and integration tests and execute them.
- [x] Create post-implementation `walkthrough.md` summarizing changes, learnings, and docker tool suggestions.

### Recommended Additional Enhancements
1. **Environment Variable Configuration**:
   Allow node operators to tune concurrency limits and delays via `.env` or system environment variables (`MAX_CONCURRENT_YTDLP`, `MAX_CONCURRENT_HN`, `YTDLP_DELAY_MS`, `DOWNLOAD_DELAY_MS`).
2. **Graceful Fallback & Queue Timeout**:
   If a download permit acquisition takes longer than a configurable timeout (e.g. 5 minutes), log a descriptive warning so tasks don't hang indefinitely if a permit is leaked.
3. **Structured Tracing**:
   Add `tracing::info!` and `tracing::debug!` logs indicating when a download task is queued, when a permit is acquired, and the delay applied before execution.

---

## 3. Autonomous AI Agent File Context Map

An autonomous AI agent working on this task should inspect the following files:

| File Path | Description & Relevance |
| :--- | :--- |
| [src/state.rs](file:///workspace/src/rs-summarizer/src/state.rs#L37-L69) | `AppState` definition. Needs `download_limiter: Arc<DownloadLimiter>` added and initialized. |
| [src/services/mod.rs](file:///workspace/src/rs-summarizer/src/services/mod.rs) | Module registry for service layer. Needs `pub mod download_limiter;` export. |
| [src/services/transcript.rs](file:///workspace/src/rs-summarizer/src/services/transcript.rs#L30-L103) | `TranscriptService::download_transcript` execution via `uvx yt-dlp`. Needs concurrency permit acquisition. |
| [src/services/hacker_news.rs](file:///workspace/src/rs-summarizer/src/services/hacker_news.rs#L51-L120) | `HackerNewsService::fetch_hn_submission` and external article downloading. Needs download permit acquisition. |
| [src/tasks.rs](file:///workspace/src/rs-summarizer/src/tasks.rs#L203-L254) | Background task handlers `fetch_youtube_content` and `fetch_hn_content`. Integrates `download_limiter` permits into pipeline execution. |
| [src/routes/mod.rs](file:///workspace/src/rs-summarizer/src/routes/mod.rs#L190-L200) | Spawns background tasks via `tokio::spawn(tasks::process_summary(...))`. |
| [Cargo.toml](file:///workspace/src/rs-summarizer/Cargo.toml#L12-L37) | Project dependencies (`tokio`, `tracing`, `axum`). |

---

## 4. Git Commit Message Guidelines

All git commits **must** strictly adhere to the **Conventional Commit** specification:

### Conventional Commit Format
```gitcommit
<type>(<scope>): <short description>

<blank line>

<detailed description of changes, motivation, design decisions, and testing verification>
```

### Commit Types
- `feat`: Implementing `DownloadLimiter` and concurrency throttling for `yt-dlp` and downloads.
- `test`: Adding unit tests for `DownloadLimiter` and integration tests for download queuing.
- `docs`: Updating `plan.md`, `task.md`, `deps.md`, and `walkthrough.md`.
- `refactor`: Integrating `download_limiter` into `AppState`, `tasks.rs`, and service modules.

### Example Commit
```gitcommit
feat(loadbalance): implement DownloadLimiter for yt-dlp and external downloads

Add a thread-safe download limiter to prevent CPU saturation on 2 vCPU Hetzner servers when processing 6+ concurrent requests.

- Create `DownloadLimiter` in `src/services/download_limiter.rs` using `tokio::sync::Semaphore` and inter-task delay timers.
- Add `download_limiter: Arc<DownloadLimiter>` to `AppState` in `src/state.rs`.
- Configure `MAX_CONCURRENT_YTDLP` (default: 2), `MAX_CONCURRENT_HN` (default: 2), `YTDLP_DELAY_MS` (default: 1000ms), and `DOWNLOAD_DELAY_MS` (default: 500ms) with environment variable overrides.
- Update `tasks::fetch_youtube_content` and `tasks::fetch_hn_content` to acquire permits before downloading.
- Add unit tests in `src/services/download_limiter.rs` verifying permit limits and delay behavior.

Tested via `cargo test` and simulated multi-request load tests.
```

---

## 5. Testing & Verification Strategy

1. **Unit Tests**:
   - `test_download_limiter_concurrency_capacity`: Verify that `DownloadLimiter` permits do not exceed max capacity.
   - `test_download_limiter_inter_task_delay`: Verify that delay logic properly pauses execution when requests occur rapidly.
   - `test_download_limiter_env_config`: Test environment variable configuration parsing with fallbacks.
2. **Integration & Code Quality Checks**:
   - `cargo check`: Compile check all targets.
   - `cargo test`: Run unit test suite.
   - `cargo fmt -- --check`: Code formatting verification.
   - `cargo clippy -- -W clippy::all`: Lint verification.
