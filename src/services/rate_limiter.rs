use chrono::NaiveDate;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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
                return;
            }
            _ => {
                // New day or first run — reset all counters
                let mut counts = model_counts.write().await;
                counts.clear();
                *last_day = Some(today);
            }
        }
    }

    /// Get today's date in America/Los_Angeles timezone.
    ///
    /// Uses a fixed UTC-8 offset (PST) as an approximation.
    /// For DST-aware handling, the `chrono-tz` crate could be added.
    fn today_la() -> NaiveDate {
        let utc_now = chrono::Utc::now();
        // Pacific time is UTC-8 (PST) or UTC-7 (PDT).
        // Using fixed offset of -8 hours as an approximation for MVP.
        let la_time = utc_now - chrono::Duration::hours(8);
        la_time.date_naive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_rate_limit_allows_under_limit() {
        let model = ModelOption {
            name: "test-model".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 1000,
            rpm_limit: 10,
            rpd_limit: 5,
        };
        let model_counts = Arc::new(RwLock::new(HashMap::new()));
        let last_reset_day = Arc::new(RwLock::new(None));

        let allowed = RateLimiter::check_rate_limit(&model, &model_counts, &last_reset_day).await;
        assert!(allowed);
    }

    #[tokio::test]
    async fn test_check_rate_limit_rejects_at_limit() {
        let model = ModelOption {
            name: "test-model".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 1000,
            rpm_limit: 10,
            rpd_limit: 3,
        };
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

        // Set last_reset_day to yesterday to trigger a reset
        let yesterday = chrono::Utc::now().date_naive() - chrono::Duration::days(2);
        {
            let mut last_day = last_reset_day.write().await;
            *last_day = Some(yesterday);
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
}
