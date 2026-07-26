# Implementation Tasks: Load Balancing & Concurrency Control for Downloads & yt-dlp

This document outlines the step-by-step implementation tasks for an AI agent to execute load balancing and concurrency control for `yt-dlp` processes and external downloads.

---

## Tasks Overview

- [x] **Task 1: Implement `DownloadLimiter` Service & Unit Tests**
  - Create `src/services/download_limiter.rs` with `DownloadLimiter` struct, `tokio::sync::Semaphore` fields, inter-task delay timers, and environment variable configuration (`MAX_CONCURRENT_YTDLP`, `MAX_CONCURRENT_HN`, `YTDLP_DELAY_MS`, `DOWNLOAD_DELAY_MS`).
  - Add unit tests verifying concurrency capacity and delay timing.
  - Export module in `src/services/mod.rs`.

- [x] **Task 2: Integrate `DownloadLimiter` into `AppState`**
  - Add `pub download_limiter: Arc<DownloadLimiter>` to `AppState` in `src/state.rs`.
  - Initialize `download_limiter` in `AppState` default/new constructors.
  - Update `src/state.rs` tests if necessary.

- [x] **Task 3: Update `tasks.rs` to Acquire Permits for Downloads**
  - Update `fetch_youtube_content` in `src/tasks.rs` to acquire a `yt-dlp` permit from `app.download_limiter` before downloading.
  - Update `fetch_hn_content` in `src/tasks.rs` to acquire a download permit from `app.download_limiter` before fetching metadata and articles.
  - Log tracing events for permit queuing, acquisition, and release.

- [x] **Task 4: Run Verification & Rust Workflow Tools**
  - Run `cargo check` to verify compilation.
  - Run `cargo test` to execute unit tests.
  - Run `cargo fmt` to format code.
  - Run `cargo clippy -- -W clippy::all` to enforce lint compliance.

- [x] **Task 5: Git Commit Following Conventional Commit Guidelines**
  - Stage updated and new files.
  - Commit using Conventional Commit format with comprehensive context.

- [x] **Task 6: Write Post-Implementation `walkthrough.md`**
  - Create `plan/20260726_02_loadbalance/walkthrough.md`.
  - Document implementation details, test results, learnings, future extensions, and recommended Docker container software packages.
