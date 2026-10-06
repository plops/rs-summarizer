//! Provider-neutral, persisted generation lifecycle rules.
use std::{fmt, str::FromStr, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GenerationStatus {
    Queued,
    Running,
    RetryWait,
    Succeeded,
    Failed,
    PartialFailed,
}

impl GenerationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::RetryWait => "retry_wait",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::PartialFailed => "partial_failed",
        }
    }
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::PartialFailed)
    }
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Queued, Self::Running)
                | (
                    Self::Running,
                    Self::RetryWait | Self::Succeeded | Self::Failed | Self::PartialFailed
                )
                | (
                    Self::RetryWait,
                    Self::Running | Self::Failed | Self::PartialFailed
                )
                | (Self::Failed | Self::PartialFailed, Self::Queued)
        )
    }
}
impl fmt::Display for GenerationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl FromStr for GenerationStatus {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "retry_wait" => Ok(Self::RetryWait),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "partial_failed" => Ok(Self::PartialFailed),
            _ => Err("unknown generation status"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PublicErrorCode {
    Unavailable,
    RateLimited,
    NetworkInterrupted,
    ProviderAborted,
    IncompleteOutput,
    SafetyRefusal,
    InvalidRequest,
    Internal,
}
impl PublicErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::RateLimited => "rate_limited",
            Self::NetworkInterrupted => "network_interrupted",
            Self::ProviderAborted => "provider_aborted",
            Self::IncompleteOutput => "incomplete_output",
            Self::SafetyRefusal => "safety_refusal",
            Self::InvalidRequest => "invalid_request",
            Self::Internal => "internal",
        }
    }
    pub const fn message(self) -> &'static str {
        match self {
            Self::Unavailable => "The model is temporarily unavailable. Retrying shortly.",
            Self::RateLimited => "This model is rate limited. Please try again later.",
            Self::NetworkInterrupted => {
                "The connection to the model was interrupted. Retrying shortly."
            }
            Self::ProviderAborted => "The model stopped before completing the summary.",
            Self::IncompleteOutput => "The model returned an incomplete summary.",
            Self::SafetyRefusal => {
                "The model could not generate this summary due to safety restrictions."
            }
            Self::InvalidRequest => "This request could not be processed.",
            Self::Internal => "Summary generation failed unexpectedly.",
        }
    }
    pub const fn retryable(self) -> bool {
        matches!(self, Self::Unavailable | Self::NetworkInterrupted)
    }
}
impl fmt::Display for PublicErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl FromStr for PublicErrorCode {
    type Err = &'static str;
    fn from_str(v: &str) -> Result<Self, Self::Err> {
        match v {
            "unavailable" => Ok(Self::Unavailable),
            "rate_limited" => Ok(Self::RateLimited),
            "network_interrupted" => Ok(Self::NetworkInterrupted),
            "provider_aborted" => Ok(Self::ProviderAborted),
            "incomplete_output" => Ok(Self::IncompleteOutput),
            "safety_refusal" => Ok(Self::SafetyRefusal),
            "invalid_request" => Ok(Self::InvalidRequest),
            "internal" => Ok(Self::Internal),
            _ => Err("unknown public error code"),
        }
    }
}

pub const MAX_RETRY_ATTEMPTS: i64 = 3;
pub fn retry_delay(attempt: i64) -> Duration {
    Duration::from_secs((30_u64.saturating_mul(1_u64 << attempt.min(4) as u32)).min(600))
}

/// Formats a stored `next_retry_at` timestamp (RFC 3339) as a human-readable
/// German retry notice, e.g. `Wiederholung in ca. 1 Minute (geplant um 05:15 Uhr)`.
///
/// The clock time is shown in UTC (the container timezone) so no extra
/// timezone crate is needed. Unparseable or empty input falls back to a short
/// notice without any raw timestamp.
pub fn format_retry_display(next_retry_at: &str) -> String {
    format_retry_display_at(next_retry_at, chrono::Utc::now())
}

fn format_retry_display_at(next_retry_at: &str, now: chrono::DateTime<chrono::Utc>) -> String {
    match next_retry_at.parse::<chrono::DateTime<chrono::Utc>>() {
        Ok(retry_at) => {
            let clock = retry_at.format("%H:%M");
            let remaining_secs = retry_at.signed_duration_since(now).num_seconds();
            if remaining_secs <= 0 {
                format!("Wiederholung geplant (geplant um {clock} Uhr)")
            } else if remaining_secs < 90 {
                format!("Wiederholung in ca. 1 Minute (geplant um {clock} Uhr)")
            } else {
                let minutes = (remaining_secs + 59) / 60;
                format!("Wiederholung in ca. {minutes} Minuten (geplant um {clock} Uhr)")
            }
        }
        Err(_) => "Wiederholung geplant".to_string(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transitions_and_completion_are_strict() {
        assert!(GenerationStatus::Queued.can_transition_to(GenerationStatus::Running));
        assert!(!GenerationStatus::Succeeded.can_transition_to(GenerationStatus::Running));
    }
    #[test]
    fn retry_policy_is_finite() {
        assert!(PublicErrorCode::Unavailable.retryable());
        assert!(!PublicErrorCode::RateLimited.retryable());
        assert!(retry_delay(2) > retry_delay(1));
    }
    #[test]
    fn retry_display_shows_relative_minutes_and_clock_time() {
        let now = "2026-10-06T05:14:30Z".parse().unwrap();
        assert_eq!(
            format_retry_display_at("2026-10-06T05:15:30Z", now),
            "Wiederholung in ca. 1 Minute (geplant um 05:15 Uhr)"
        );
        assert_eq!(
            format_retry_display_at("2026-10-06T05:19:30Z", now),
            "Wiederholung in ca. 5 Minuten (geplant um 05:19 Uhr)"
        );
    }
    #[test]
    fn retry_display_never_leaks_raw_timestamps() {
        let now = "2026-10-06T05:14:30Z".parse().unwrap();
        let overdue = format_retry_display_at("2026-10-06T05:10:00Z", now);
        assert!(overdue.contains("Wiederholung geplant"));
        assert!(!overdue.contains("2026-10-06"));
        assert_eq!(format_retry_display_at("", now), "Wiederholung geplant");
        assert_eq!(
            format_retry_display_at("not-a-time", now),
            "Wiederholung geplant"
        );
    }
}
