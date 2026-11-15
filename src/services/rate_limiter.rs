use chrono::{DateTime, Datelike, NaiveDate, Utc, Weekday};
use sqlx::SqlitePool;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};

use crate::state::ModelOption;

pub struct RateLimiter;

impl RateLimiter {
    /// Check if a model has exceeded its daily request limit.
    /// Returns true if the request is allowed, false if rate limited.
    pub async fn check_rate_limit(
        model: &ModelOption,
        model_counts: &Arc<RwLock<HashMap<String, u32>>>,
        last_reset_day: &Arc<RwLock<Option<NaiveDate>>>,
    ) -> bool {
        // Check if we need to reset counters (new day in LA timezone)
        Self::maybe_reset_counters(model_counts, last_reset_day).await;

        let counts = model_counts.read().await;
        let current_count = counts.get(&model.name).copied().unwrap_or(0);
        current_count < model.rpd_limit
    }

    /// Increment the request counter for a model.
    pub async fn increment_counter(
        model_name: &str,
        model_counts: &Arc<RwLock<HashMap<String, u32>>>,
    ) {
        let mut counts = model_counts.write().await;
        let counter = counts.entry(model_name.to_string()).or_insert(0);
        *counter += 1;
    }

    /// Reset counters if the current day (America/Los_Angeles) differs from last_reset_day.
    async fn maybe_reset_counters(
        model_counts: &Arc<RwLock<HashMap<String, u32>>>,
        last_reset_day: &Arc<RwLock<Option<NaiveDate>>>,
    ) {
        let today = Self::today_la();

        let mut last_day = last_reset_day.write().await;
        match *last_day {
            Some(day) if day == today => {
                // Same day, no reset needed
            }
            _ => {
                // New day or first run — reset all counters
                let mut counts = model_counts.write().await;
                counts.clear();
                *last_day = Some(today);
            }
        }
    }

    /// Reset the in-memory counters from the persistent SQLite table
    /// (`model_daily_usage`) when the LA day changed, so a restart never
    /// resets — or double-spends — the daily quotas.
    pub async fn maybe_reset_counters_persistent(
        db: &SqlitePool,
        model_counts: &Arc<RwLock<HashMap<String, u32>>>,
        last_reset_day: &Arc<RwLock<Option<NaiveDate>>>,
    ) {
        let today = Self::today_la();
        let needs_reset = {
            let last_day = last_reset_day.read().await;
            last_day.is_none_or(|day| day != today)
        };
        if !needs_reset {
            return;
        }
        let loaded = Self::load_daily_counts(db, today).await;
        {
            let mut counts = model_counts.write().await;
            *counts = loaded;
        }
        {
            let mut last_day = last_reset_day.write().await;
            *last_day = Some(today);
        }
    }

    /// Load today's per-model request counts from `model_daily_usage`.
    pub async fn load_daily_counts(db: &SqlitePool, day: NaiveDate) -> HashMap<String, u32> {
        crate::db::load_model_daily_usage(db, &day.format("%Y-%m-%d").to_string()).await
    }

    /// Persistent variant of [`RateLimiter::check_rate_limit`]: reloads from
    /// SQLite on day rollover, then checks the in-memory counters.
    pub async fn check_rate_limit_persistent(
        model: &ModelOption,
        db: &SqlitePool,
        model_counts: &Arc<RwLock<HashMap<String, u32>>>,
        last_reset_day: &Arc<RwLock<Option<NaiveDate>>>,
    ) -> bool {
        Self::maybe_reset_counters_persistent(db, model_counts, last_reset_day).await;
        let counts = model_counts.read().await;
        let current_count = counts.get(&model.name).copied().unwrap_or(0);
        current_count < model.rpd_limit
    }

    /// Persistent variant of [`RateLimiter::increment_counter`]: bumps the
    /// in-memory counter and writes through to `model_daily_usage`.
    pub async fn increment_counter_persistent(
        db: &SqlitePool,
        model_name: &str,
        model_counts: &Arc<RwLock<HashMap<String, u32>>>,
    ) {
        Self::increment_counter(model_name, model_counts).await;
        let day_str = Self::today_la().format("%Y-%m-%d").to_string();
        crate::db::record_model_daily_usage(db, &day_str, model_name).await;
    }

    /// Returns true when at least one model in the fallback chain still has
    /// daily budget left. Callers reject the submission only when the whole
    /// chain — not just the primary model — is exhausted.
    pub async fn check_chain_available(
        chain: &[&str],
        model_options: &[ModelOption],
        db: &SqlitePool,
        model_counts: &Arc<RwLock<HashMap<String, u32>>>,
        last_reset_day: &Arc<RwLock<Option<NaiveDate>>>,
    ) -> bool {
        Self::maybe_reset_counters_persistent(db, model_counts, last_reset_day).await;
        let counts = model_counts.read().await;
        chain.iter().any(|name| {
            model_options
                .iter()
                .find(|m| m.name == *name)
                .is_some_and(|m| counts.get(&m.name).copied().unwrap_or(0) < m.rpd_limit)
        })
    }

    /// Get today's date in America/Los_Angeles.
    ///
    /// DST transitions use the exact US Pacific switchover instants: PDT
    /// begins at 02:00 local (10:00 UTC) on the second Sunday of March and
    /// ends at 02:00 local (09:00 UTC) on the first Sunday of November.
    pub fn today_la() -> NaiveDate {
        Self::la_date_at(Utc::now())
    }

    /// LA calendar date for an arbitrary UTC instant (testable core of
    /// [`RateLimiter::today_la`]).
    fn la_date_at(utc_now: DateTime<Utc>) -> NaiveDate {
        let offset_hours = if Self::is_pacific_daylight_at(utc_now) {
            7
        } else {
            8
        };
        (utc_now - Duration::from_secs(offset_hours * 3600)).date_naive()
    }

    /// Whether Pacific Daylight Time applies at `utc_now`.
    fn is_pacific_daylight_at(utc_now: DateTime<Utc>) -> bool {
        let year = utc_now.date_naive().year();
        // Second Sunday of March, 10:00 UTC (= 02:00 PST).
        let mut march_sunday = NaiveDate::from_ymd_opt(year, 3, 8).unwrap();
        while march_sunday.weekday() != Weekday::Sun {
            march_sunday = march_sunday.succ_opt().unwrap();
        }
        let pdt_start = march_sunday.and_hms_opt(10, 0, 0).unwrap().and_utc();
        // First Sunday of November, 09:00 UTC (= 02:00 PDT).
        let mut nov_sunday = NaiveDate::from_ymd_opt(year, 11, 1).unwrap();
        while nov_sunday.weekday() != Weekday::Sun {
            nov_sunday = nov_sunday.succ_opt().unwrap();
        }
        let pdt_end = nov_sunday.and_hms_opt(9, 0, 0).unwrap().and_utc();
        utc_now >= pdt_start && utc_now < pdt_end
    }

    /// Conservative prompt-size estimate in tokens: ~4 chars per token plus
    /// system/few-shot/directive overhead (measured templates total ~36 KB).
    pub fn estimate_prompt_tokens(transcript_chars: usize) -> u64 {
        transcript_chars as u64 / 4 + PROMPT_OVERHEAD_TOKENS
    }

    /// Sliding 60-second TPM gate. Reserves `estimate` tokens against
    /// `limit` (`0` = no limit): `Allowed` records the reservation,
    /// `OverLimit` means the prompt can never fit, and `Wait(d)` asks the
    /// caller to sleep `d` before retrying once.
    pub async fn check_tpm_capacity(
        tracker: &Arc<Mutex<TpmTracker>>,
        model: &str,
        limit: u32,
        estimate: u64,
    ) -> TpmDecision {
        tracker
            .lock()
            .await
            .check_and_reserve(model, limit, estimate)
    }
}

/// Fixed per-request token overhead (system instruction, few-shot example
/// input/output, language and glossary directives).
pub const PROMPT_OVERHEAD_TOKENS: u64 = 12_000;

/// Outcome of a [`TpmTracker`] reservation attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpmDecision {
    Allowed,
    OverLimit,
    Wait(Duration),
}

/// Per-model sliding 60-second windows of estimated token usage.
#[derive(Debug, Default)]
pub struct TpmTracker {
    windows: HashMap<String, VecDeque<(Instant, u64)>>,
}

impl TpmTracker {
    pub fn new() -> Self {
        Self::default()
    }

    fn check_and_reserve(&mut self, model: &str, limit: u32, estimate: u64) -> TpmDecision {
        if limit == 0 {
            return TpmDecision::Allowed;
        }
        if estimate > limit as u64 {
            return TpmDecision::OverLimit;
        }
        let now = Instant::now();
        let window = self.windows.entry(model.to_string()).or_default();
        while window
            .front()
            .is_some_and(|(at, _)| now.duration_since(*at) >= Duration::from_secs(60))
        {
            window.pop_front();
        }
        let used: u64 = window.iter().map(|(_, n)| n).sum();
        if used + estimate > limit as u64 {
            let wait = window
                .front()
                .map(|(at, _)| Duration::from_secs(60).saturating_sub(now.duration_since(*at)))
                .unwrap_or_default();
            return TpmDecision::Wait(wait);
        }
        window.push_back((now, estimate));
        TpmDecision::Allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ModelArchitecture;

    fn test_model(name: &str, rpd_limit: u32) -> ModelOption {
        ModelOption {
            name: name.to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 1000,
            rpm_limit: 10,
            tpm_limit: 0,
            rpd_limit,
            architecture: ModelArchitecture::Gemini,
        }
    }

    #[tokio::test]
    async fn test_check_rate_limit_allows_under_limit() {
        let model = test_model("test-model", 5);
        let model_counts = Arc::new(RwLock::new(HashMap::new()));
        let last_reset_day = Arc::new(RwLock::new(None));

        let allowed = RateLimiter::check_rate_limit(&model, &model_counts, &last_reset_day).await;
        assert!(allowed);
    }

    #[tokio::test]
    async fn test_check_rate_limit_rejects_at_limit() {
        let model = test_model("test-model", 3);
        let model_counts = Arc::new(RwLock::new(HashMap::new()));
        // Set last_reset_day to today so maybe_reset_counters won't clear our counts
        let today = RateLimiter::today_la();
        let last_reset_day = Arc::new(RwLock::new(Some(today)));

        // Increment counter to the limit
        for _ in 0..3 {
            RateLimiter::increment_counter("test-model", &model_counts).await;
        }

        let allowed = RateLimiter::check_rate_limit(&model, &model_counts, &last_reset_day).await;
        assert!(!allowed);
    }

    #[tokio::test]
    async fn test_increment_counter() {
        let model_counts = Arc::new(RwLock::new(HashMap::new()));

        RateLimiter::increment_counter("model-a", &model_counts).await;
        RateLimiter::increment_counter("model-a", &model_counts).await;
        RateLimiter::increment_counter("model-b", &model_counts).await;

        let counts = model_counts.read().await;
        assert_eq!(counts.get("model-a"), Some(&2));
        assert_eq!(counts.get("model-b"), Some(&1));
    }

    #[tokio::test]
    async fn test_daily_reset_clears_counters() {
        let model_counts = Arc::new(RwLock::new(HashMap::new()));
        let last_reset_day = Arc::new(RwLock::new(None));

        // Add some counts
        RateLimiter::increment_counter("model-a", &model_counts).await;
        RateLimiter::increment_counter("model-b", &model_counts).await;

        // Set last_reset_day to two days ago to trigger a reset
        let two_days_ago = chrono::Utc::now().date_naive() - chrono::Duration::days(2);
        {
            let mut last_day = last_reset_day.write().await;
            *last_day = Some(two_days_ago);
        }

        // Calling maybe_reset_counters should clear everything
        RateLimiter::maybe_reset_counters(&model_counts, &last_reset_day).await;

        let counts = model_counts.read().await;
        assert!(counts.is_empty());

        let last_day = last_reset_day.read().await;
        assert!(last_day.is_some());
    }

    #[test]
    fn test_today_la_returns_valid_date() {
        let today = RateLimiter::today_la();
        // Just verify it returns a reasonable date (not panicking)
        let utc_today = chrono::Utc::now().date_naive();
        // LA date should be within 1 day of UTC date
        let diff = (utc_today - today).num_days().abs();
        assert!(diff <= 1);
    }

    #[test]
    fn test_dst_spring_forward_exact_transition() {
        // 2026: second Sunday of March is the 8th; PDT starts 10:00 UTC.
        let just_before = DateTime::parse_from_rfc3339("2026-03-08T09:59:59Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(!RateLimiter::is_pacific_daylight_at(just_before));
        // 01:59:59 PST on March 8.
        assert_eq!(
            RateLimiter::la_date_at(just_before),
            NaiveDate::from_ymd_opt(2026, 3, 8).unwrap()
        );
        let just_after = DateTime::parse_from_rfc3339("2026-03-08T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(RateLimiter::is_pacific_daylight_at(just_after));
        // 03:00 PDT on March 8.
        assert_eq!(
            RateLimiter::la_date_at(just_after),
            NaiveDate::from_ymd_opt(2026, 3, 8).unwrap()
        );
        // Late March 7 UTC is still March 6 in LA (PST).
        let march_7 = DateTime::parse_from_rfc3339("2026-03-07T06:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(!RateLimiter::is_pacific_daylight_at(march_7));
        assert_eq!(
            RateLimiter::la_date_at(march_7),
            NaiveDate::from_ymd_opt(2026, 3, 6).unwrap()
        );
    }

    #[test]
    fn test_dst_fall_back_exact_transition() {
        // 2026: first Sunday of November is the 1st; PST resumes 09:00 UTC.
        let just_before = DateTime::parse_from_rfc3339("2026-11-01T08:59:59Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(RateLimiter::is_pacific_daylight_at(just_before));
        // 01:59:59 PDT on November 1.
        assert_eq!(
            RateLimiter::la_date_at(just_before),
            NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()
        );
        let just_after = DateTime::parse_from_rfc3339("2026-11-01T09:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(!RateLimiter::is_pacific_daylight_at(just_after));
        // 01:00 PST on November 1.
        assert_eq!(
            RateLimiter::la_date_at(just_after),
            NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()
        );
        // Mid-summer is always PDT.
        let july = DateTime::parse_from_rfc3339("2026-07-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(RateLimiter::is_pacific_daylight_at(july));
        // Mid-winter is always PST.
        let january = DateTime::parse_from_rfc3339("2026-01-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(!RateLimiter::is_pacific_daylight_at(january));
    }

    #[test]
    fn test_tpm_tracker_allows_under_limit() {
        let mut tracker = TpmTracker::new();
        assert_eq!(
            tracker.check_and_reserve("gemini-3.5-flash-lite", 250_000, 20_000),
            TpmDecision::Allowed
        );
        // Zero limit means no TPM cap.
        assert_eq!(
            tracker.check_and_reserve("hetzner-qwen-3.8-27b", 0, 1_000_000),
            TpmDecision::Allowed
        );
    }

    #[test]
    fn test_tpm_tracker_rejects_oversized_prompt() {
        let mut tracker = TpmTracker::new();
        // A ~25k-token transcript can never fit Gemma's 16k TPM window.
        assert_eq!(
            tracker.check_and_reserve("gemma-4-31b-it", 16_000, 25_000),
            TpmDecision::OverLimit
        );
    }

    #[test]
    fn test_tpm_tracker_window_exhaustion_asks_to_wait() {
        let mut tracker = TpmTracker::new();
        assert_eq!(
            tracker.check_and_reserve("gemma-4-31b-it", 16_000, 10_000),
            TpmDecision::Allowed
        );
        match tracker.check_and_reserve("gemma-4-31b-it", 16_000, 10_000) {
            TpmDecision::Wait(wait) => assert!(wait <= Duration::from_secs(60)),
            other => panic!("expected Wait, got {other:?}"),
        }
        // Other models have independent windows.
        assert_eq!(
            tracker.check_and_reserve("gemini-3.5-flash-lite", 250_000, 200_000),
            TpmDecision::Allowed
        );
    }

    #[test]
    fn test_estimate_prompt_tokens_includes_overhead() {
        // Empty transcript still carries the template overhead.
        assert_eq!(
            RateLimiter::estimate_prompt_tokens(0),
            PROMPT_OVERHEAD_TOKENS
        );
        assert_eq!(
            RateLimiter::estimate_prompt_tokens(40_000),
            10_000 + PROMPT_OVERHEAD_TOKENS
        );
    }
}
