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
pub fn validate_complete_output(text: &str, is_hn: bool) -> Result<(), PublicErrorCode> {
    let required: &[&str] = if is_hn {
        &["abstract", "key points", "discussion highlights"]
    } else {
        &["abstract", "key highlights"]
    };
    let lower = text.to_ascii_lowercase();
    if text.trim().len() < 80 || required.iter().any(|heading| !lower.contains(heading)) {
        Err(PublicErrorCode::IncompleteOutput)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transitions_and_completion_are_strict() {
        assert!(GenerationStatus::Queued.can_transition_to(GenerationStatus::Running));
        assert!(!GenerationStatus::Succeeded.can_transition_to(GenerationStatus::Running));
        assert!(validate_complete_output("# Abstract\nEnough text to make this a meaningful response that continues beyond eighty characters.\n# Key Points\n- x\n# Discussion Highlights\n- y", true).is_ok());
        assert_eq!(
            validate_complete_output("# Abstract\ncut off", false),
            Err(PublicErrorCode::IncompleteOutput)
        );
    }
    #[test]
    fn retry_policy_is_finite() {
        assert!(PublicErrorCode::Unavailable.retryable());
        assert!(!PublicErrorCode::RateLimited.retryable());
        assert!(retry_delay(2) > retry_delay(1));
    }
}
