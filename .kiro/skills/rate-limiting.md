---
inclusion: manual
---

# Rate Limiting Design

## Overview

rs-summarizer implements per-model daily request counters to prevent exceeding Gemini API quotas. Counters reset at the start of each calendar day in America/Los_Angeles timezone.

## Architecture

Rate limiting state lives in `AppState` (shared across all requests):

```rust
pub struct AppState {
    pub model_counts: Arc<RwLock<HashMap<String, u32>>>,
    pub last_reset_day: Arc<RwLock<Option<NaiveDate>>>,
    // ...
}
```

## RateLimiter API

File: `src/services/rate_limiter.rs`

```rust
pub struct RateLimiter;

impl RateLimiter {
    /// Returns true if request is allowed, false if rate limited.
    pub async fn check_rate_limit(
        model: &ModelOption,
        model_counts: &Arc<RwLock<HashMap<String, u32>>>,
        last_reset_day: &Arc<RwLock<Option<NaiveDate>>>,
    ) -> bool;

    /// Increment counter after spawning a task.
    pub async fn increment_counter(
        model_name: &str,
        model_counts: &Arc<RwLock<HashMap<String, u32>>>,
    );
}
```

## Daily Reset Logic

`maybe_reset_counters()` is called on every `check_rate_limit()`:

1. Compute today's date in America/Los_Angeles (approximated as UTC-8)
2. Compare with `last_reset_day`
3. If different day (or first run): clear all counters, update `last_reset_day`
4. If same day: no-op

```rust
fn today_la() -> NaiveDate {
    let utc_now = chrono::Utc::now();
    let la_time = utc_now - chrono::Duration::hours(8);
    la_time.date_naive()
}
```

Note: Uses fixed UTC-8 offset (PST approximation). For DST-aware handling, `chrono-tz` could be added.

## Usage in Route Handler

```rust
// In process_transcript():
let allowed = RateLimiter::check_rate_limit(&model, &app.model_counts, &app.last_reset_day).await;
if !allowed {
    return Html("<p>Rate limit exceeded...</p>".to_string());
}

// After spawning task:
RateLimiter::increment_counter(&input.model, &app.model_counts).await;
```

## Model Limits

Each `ModelOption` has `rpd_limit` (requests per day). The counter is compared against this:

```rust
let current_count = counts.get(&model.name).copied().unwrap_or(0);
current_count < model.rpd_limit  // true = allowed
```

## Relevant Files

- `src/services/rate_limiter.rs` — Rate limiter implementation
- `src/state.rs` — `ModelOption.rpd_limit` field
- `src/routes/mod.rs` — Check + increment in `process_transcript()`
