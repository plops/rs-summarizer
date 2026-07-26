use std::sync::Arc;
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::time::{sleep, Duration, Instant};
use tracing;

/// Service managing global concurrency limits and inter-task delays for external downloads
/// and CPU-heavy subprocesses (e.g. yt-dlp via Python).
#[derive(Clone, Debug)]
pub struct DownloadLimiter {
    yt_dlp_semaphore: Arc<Semaphore>,
    hn_semaphore: Arc<Semaphore>,
    yt_dlp_last_run: Arc<Mutex<Option<Instant>>>,
    download_last_run: Arc<Mutex<Option<Instant>>>,
    yt_dlp_delay: Duration,
    download_delay: Duration,
}

impl Default for DownloadLimiter {
    fn default() -> Self {
        Self::from_env()
    }
}

impl DownloadLimiter {
    /// Creates a new `DownloadLimiter` with explicit parameters.
    pub fn new(
        max_concurrent_yt_dlp: usize,
        max_concurrent_hn: usize,
        yt_dlp_delay_ms: u64,
        download_delay_ms: u64,
    ) -> Self {
        tracing::info!(
            max_concurrent_yt_dlp = max_concurrent_yt_dlp,
            max_concurrent_hn = max_concurrent_hn,
            yt_dlp_delay_ms = yt_dlp_delay_ms,
            download_delay_ms = download_delay_ms,
            "Initializing DownloadLimiter concurrency governor"
        );

        Self {
            yt_dlp_semaphore: Arc::new(Semaphore::new(max_concurrent_yt_dlp)),
            hn_semaphore: Arc::new(Semaphore::new(max_concurrent_hn)),
            yt_dlp_last_run: Arc::new(Mutex::new(None)),
            download_last_run: Arc::new(Mutex::new(None)),
            yt_dlp_delay: Duration::from_millis(yt_dlp_delay_ms),
            download_delay: Duration::from_millis(download_delay_ms),
        }
    }

    /// Construct a `DownloadLimiter` reading from environment variables with sensible defaults for 2 vCPU servers.
    pub fn from_env() -> Self {
        let max_yt_dlp = std::env::var("MAX_CONCURRENT_YTDLP")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2);

        let max_hn = std::env::var("MAX_CONCURRENT_HN")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2);

        let yt_dlp_delay = std::env::var("YTDLP_DELAY_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000);

        let download_delay = std::env::var("DOWNLOAD_DELAY_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(500);

        Self::new(max_yt_dlp, max_hn, yt_dlp_delay, download_delay)
    }

    /// Acquires an owned permit for `yt-dlp` execution and enforces inter-job delay throttling.
    pub async fn acquire_yt_dlp_permit(&self) -> OwnedSemaphorePermit {
        let available = self.yt_dlp_semaphore.available_permits();
        tracing::debug!(
            available_permits = available,
            "Acquiring permit for yt-dlp execution"
        );

        let permit = Arc::clone(&self.yt_dlp_semaphore)
            .acquire_owned()
            .await
            .expect("yt-dlp semaphore closed unexpectedly");

        let mut last_run = self.yt_dlp_last_run.lock().await;
        if let Some(prev) = *last_run {
            let elapsed = prev.elapsed();
            if elapsed < self.yt_dlp_delay {
                let sleep_dur = self.yt_dlp_delay - elapsed;
                tracing::info!(
                    sleep_ms = sleep_dur.as_millis(),
                    "Throttling yt-dlp execution with inter-job delay"
                );
                sleep(sleep_dur).await;
            }
        }
        *last_run = Some(Instant::now());

        permit
    }

    /// Acquires an owned permit for HackerNews / external HTTP article download and enforces inter-job delay throttling.
    pub async fn acquire_hn_permit(&self) -> OwnedSemaphorePermit {
        let available = self.hn_semaphore.available_permits();
        tracing::debug!(
            available_permits = available,
            "Acquiring permit for HackerNews / HTTP download"
        );

        let permit = Arc::clone(&self.hn_semaphore)
            .acquire_owned()
            .await
            .expect("HN download semaphore closed unexpectedly");

        let mut last_run = self.download_last_run.lock().await;
        if let Some(prev) = *last_run {
            let elapsed = prev.elapsed();
            if elapsed < self.download_delay {
                let sleep_dur = self.download_delay - elapsed;
                tracing::info!(
                    sleep_ms = sleep_dur.as_millis(),
                    "Throttling download execution with inter-job delay"
                );
                sleep(sleep_dur).await;
            }
        }
        *last_run = Some(Instant::now());

        permit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_download_limiter_initialization() {
        let limiter = DownloadLimiter::new(2, 4, 100, 50);
        assert_eq!(limiter.yt_dlp_semaphore.available_permits(), 2);
        assert_eq!(limiter.hn_semaphore.available_permits(), 4);
    }

    #[tokio::test]
    async fn test_download_limiter_concurrency_capacity() {
        let limiter = DownloadLimiter::new(2, 2, 0, 0);

        let permit1 = limiter.acquire_yt_dlp_permit().await;
        assert_eq!(limiter.yt_dlp_semaphore.available_permits(), 1);

        let permit2 = limiter.acquire_yt_dlp_permit().await;
        assert_eq!(limiter.yt_dlp_semaphore.available_permits(), 0);

        drop(permit1);
        assert_eq!(limiter.yt_dlp_semaphore.available_permits(), 1);

        drop(permit2);
        assert_eq!(limiter.yt_dlp_semaphore.available_permits(), 2);
    }

    #[tokio::test]
    async fn test_download_limiter_inter_task_delay() {
        let limiter = DownloadLimiter::new(2, 2, 50, 0);

        let start = Instant::now();
        let permit1 = limiter.acquire_yt_dlp_permit().await;
        drop(permit1);

        let permit2 = limiter.acquire_yt_dlp_permit().await;
        drop(permit2);

        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(40),
            "Inter-task delay should pause rapid consecutive calls"
        );
    }
}
