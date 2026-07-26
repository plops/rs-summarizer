# Dependencies & Crate Documentation: Load Balancing & Download Throttling

This document records the dependencies and core concurrency primitives used to implement process and download concurrency limiting, inter-task delays, and load balancing for `yt-dlp` and external HTTP/HackerNews downloads in `rs-summarizer`.

---

## 1. Core Concurrency Dependencies & Primitives

No external third-party crates are strictly required for this feature, as Tokio's standard concurrency primitives (`tokio::sync::Semaphore`, `tokio::sync::Mutex`, `tokio::time::sleep`) provide lightweight, zero-overhead asynchronous rate and concurrency limiting.

- **Crate Name**: `tokio`
- **Version**: `1.52` (with `full` features enabled in `Cargo.toml`)
- **GitHub Repository**: [tokio-rs/tokio](https://github.com/tokio-rs/tokio)
- **GitHub Organization / Owner**: `tokio-rs`
- **License**: MIT
- **Purpose**: Provides async task scheduling, `Semaphore` for permit-based concurrency limits (capping simultaneous `yt-dlp` subprocesses and HN downloads), `Mutex` for tracking timestamp state across async tasks, and `sleep` for inter-task delay throttling.

### Usage Example: `Semaphore` & Inter-Task Delay Throttling Pattern

```rust
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore, SemaphorePermit};
use tokio::time::{sleep, Duration, Instant};

#[derive(Clone)]
pub struct DownloadLimiter {
    yt_dlp_semaphore: Arc<Semaphore>,
    hn_semaphore: Arc<Semaphore>,
    yt_dlp_last_run: Arc<Mutex<Option<Instant>>>,
    yt_dlp_delay: Duration,
    download_delay: Duration,
}

impl DownloadLimiter {
    pub fn new(
        max_concurrent_yt_dlp: usize,
        max_concurrent_hn: usize,
        yt_dlp_delay_ms: u64,
        download_delay_ms: u64,
    ) -> Self {
        Self {
            yt_dlp_semaphore: Arc::new(Semaphore::new(max_concurrent_yt_dlp)),
            hn_semaphore: Arc::new(Semaphore::new(max_concurrent_hn)),
            yt_dlp_last_run: Arc::new(Mutex::new(None)),
            yt_dlp_delay: Duration::from_millis(yt_dlp_delay_ms),
            download_delay: Duration::from_millis(download_delay_ms),
        }
    }

    /// Acquires a permit for yt-dlp execution and enforces inter-job delay.
    pub async fn acquire_yt_dlp_permit(&self) -> SemaphorePermit<'_> {
        let permit = self
            .yt_dlp_semaphore
            .acquire()
            .await
            .expect("yt-dlp semaphore closed");

        let mut last_run = self.yt_dlp_last_run.lock().await;
        if let Some(last_time) = *last_run {
            let elapsed = last_time.elapsed();
            if elapsed < self.yt_dlp_delay {
                let sleep_dur = self.yt_dlp_delay - elapsed;
                sleep(sleep_dur).await;
            }
        }
        *last_run = Some(Instant::now());

        permit
    }
}
```

### DeepWiki Query Reference Format
To query information regarding Tokio's concurrency primitives in DeepWiki or external docs, use:
- **Repo Target**: `tokio-rs/tokio`
- **Query Pattern**: `"How to use tokio::sync::Semaphore and tokio::time::sleep to rate limit subprocesses?"`

---

## 2. Existing Workspace Dependencies Utilized

- **`tokio`** (`1.52`, Org: `tokio-rs/tokio`): Task orchestration, async semaphores, and timing delays.
- **`tracing`** (`0.1`, Org: `tokio-rs/tracing`): Structured logging of download queueing, semaphore acquisitions, and delay durations.
- **`sqlx`** (`0.9`, Org: `launchbadge/sqlx`): SQLite connection pool that benefits from reduced lock contention when CPU usage is throttled.
