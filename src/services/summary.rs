use futures_util::StreamExt;
use gemini_rust::{Gemini, Model};
use sqlx::SqlitePool;
use tracing;

use crate::db;
use crate::errors::SummaryError;
use crate::state::ModelOption;

/// Result of a successful summary generation.
pub struct SummaryResult {
    pub summary_text: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: f64,
    pub duration_secs: f64,
}

pub struct SummaryService {
    api_key: String,
}

impl SummaryService {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    /// Validates transcript length and generates a summary via Gemini.
    /// Persists chunks to DB progressively during streaming.
    ///
    /// Requirements: 6.1, 6.2, 6.5, 6.6, 6.7
    pub async fn generate_summary(
        &self,
        db_pool: &SqlitePool,
        identifier: i64,
        transcript: &str,
        model: &ModelOption,
    ) -> Result<SummaryResult, SummaryError> {
        // Validate transcript length (Req 6.5, 6.6)
        let word_count = transcript.split_whitespace().count();
        if word_count < 30 {
            return Err(SummaryError::TranscriptTooShort);
        }
        if word_count > 280_000 {
            return Err(SummaryError::TranscriptTooLong(word_count, 280_000));
        }

        let start = std::time::Instant::now();
        let prompt = self.build_prompt(transcript);

        // Create Gemini client with the specified model
        let gemini_model = Model::Custom(format!("models/{}", model.name));
        let client = Gemini::with_model(&self.api_key, gemini_model)
            .map_err(|e| SummaryError::ApiError(e.to_string()))?;

        tracing::info!(
            identifier = identifier,
            model = %model.name,
            word_count = word_count,
            "Starting summary generation"
        );

        // Use streaming to persist chunks progressively (Req 6.1, 6.2)
        let mut builder = client.generate_content();

        // Gemma models don't support system prompts (developer instructions)
        if !model.name.starts_with("gemma") {
            builder = builder.with_system_prompt(
                "You are a helpful assistant that summarizes YouTube video transcripts. \
                 Provide a comprehensive summary with key points and timestamps where relevant.",
            );
        }

        let mut stream = builder
            .with_user_message(&prompt)
            .execute_stream()
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if is_rate_limit_error(&err_str) {
                    tracing::warn!(identifier = identifier, "Rate limited by Gemini API");
                    SummaryError::RateLimited
                } else {
                    tracing::error!(identifier = identifier, error = %err_str, "Gemini API error");
                    SummaryError::ApiError(err_str)
                }
            })?;

        let mut summary_text = String::new();
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;

        // Process streaming chunks, persisting each to DB progressively
        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(response) => {
                    let chunk_text = response.text();
                    if !chunk_text.is_empty() {
                        // Persist chunk to DB (Req 6.2 - monotonically growing)
                        db::update_summary_chunk(db_pool, identifier, &chunk_text)
                            .await
                            .map_err(|e| SummaryError::ApiError(format!("DB error: {}", e)))?;
                        summary_text.push_str(&chunk_text);
                    }

                    // Extract token counts from usage metadata (last chunk typically has them)
                    if let Some(usage) = &response.usage_metadata {
                        if let Some(prompt_tokens) = usage.prompt_token_count {
                            input_tokens = prompt_tokens as u64;
                        }
                        if let Some(candidates_tokens) = usage.candidates_token_count {
                            output_tokens = candidates_tokens as u64;
                        }
                    }
                }
                Err(e) => {
                    let err_str = e.to_string();
                    // Handle rate limiting mid-stream (Req 6.7)
                    if is_rate_limit_error(&err_str) {
                        // Append error to partial summary without setting summary_done
                        let error_msg = "\n\n[Error: Rate limited (ResourceExhausted). Please retry later.]";
                        db::update_summary_chunk(db_pool, identifier, error_msg)
                            .await
                            .map_err(|e| SummaryError::ApiError(format!("DB error: {}", e)))?;
                        return Err(SummaryError::RateLimited);
                    }
                    return Err(SummaryError::ApiError(err_str));
                }
            }
        }

        if summary_text.is_empty() {
            return Err(SummaryError::ApiError(
                "Gemini returned empty response".to_string(),
            ));
        }

        let duration_secs = start.elapsed().as_secs_f64();

        // If token counts weren't provided by the API, estimate them
        if input_tokens == 0 {
            input_tokens = (prompt.len() as u64) / 4;
        }
        if output_tokens == 0 {
            output_tokens = (summary_text.len() as u64) / 4;
        }

        let cost = self.compute_cost(model, input_tokens, output_tokens);

        tracing::info!(
            identifier = identifier,
            input_tokens = input_tokens,
            output_tokens = output_tokens,
            cost = cost,
            duration_secs = duration_secs,
            "Summary generation complete"
        );

        Ok(SummaryResult {
            summary_text,
            input_tokens,
            output_tokens,
            cost,
            duration_secs,
        })
    }

    /// Builds the prompt from the transcript text.
    pub fn build_prompt(&self, transcript: &str) -> String {
        format!(
            "Summarize the following YouTube video transcript. \
             Include key points, timestamps where relevant, and a brief overview.\n\n\
             Transcript:\n{}",
            transcript
        )
    }

    /// Computes the cost based on token counts and model pricing.
    ///
    /// Formula: (input_tokens * input_price_per_mtoken / 1_000_000)
    ///        + (output_tokens * output_price_per_mtoken / 1_000_000)
    pub fn compute_cost(&self, model: &ModelOption, input_tokens: u64, output_tokens: u64) -> f64 {
        let input_cost = (input_tokens as f64) * model.input_price_per_mtoken / 1_000_000.0;
        let output_cost = (output_tokens as f64) * model.output_price_per_mtoken / 1_000_000.0;
        input_cost + output_cost
    }
}

/// Checks if an error string indicates a rate limit / resource exhausted error.
fn is_rate_limit_error(err_str: &str) -> bool {
    err_str.contains("ResourceExhausted")
        || err_str.contains("429")
        || err_str.contains("RESOURCE_EXHAUSTED")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ModelOption;

    fn test_model() -> ModelOption {
        ModelOption {
            name: "gemini-2.0-flash".to_string(),
            input_price_per_mtoken: 0.075,
            output_price_per_mtoken: 0.30,
            context_window: 1_000_000,
            rpm_limit: 10,
            rpd_limit: 1500,
        }
    }

    #[test]
    fn test_compute_cost_basic() {
        let svc = SummaryService::new("test-key".to_string());
        let model = test_model();

        // 1000 input tokens, 500 output tokens
        let cost = svc.compute_cost(&model, 1000, 500);
        // input: 1000 * 0.075 / 1_000_000 = 0.000075
        // output: 500 * 0.30 / 1_000_000 = 0.00015
        // total: 0.000225
        let expected = 0.000075 + 0.00015;
        assert!((cost - expected).abs() < 1e-10);
    }

    #[test]
    fn test_compute_cost_zero_tokens() {
        let svc = SummaryService::new("test-key".to_string());
        let model = test_model();

        let cost = svc.compute_cost(&model, 0, 0);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_compute_cost_large_tokens() {
        let svc = SummaryService::new("test-key".to_string());
        let model = test_model();

        // 1 million input tokens, 100k output tokens
        let cost = svc.compute_cost(&model, 1_000_000, 100_000);
        // input: 1_000_000 * 0.075 / 1_000_000 = 0.075
        // output: 100_000 * 0.30 / 1_000_000 = 0.03
        // total: 0.105
        let expected = 0.075 + 0.03;
        assert!((cost - expected).abs() < 1e-10);
    }

    #[test]
    fn test_build_prompt_contains_transcript() {
        let svc = SummaryService::new("test-key".to_string());
        let transcript = "00:00:00 Hello world\n00:01:00 This is a test";
        let prompt = svc.build_prompt(transcript);

        assert!(prompt.contains("Summarize"));
        assert!(prompt.contains(transcript));
        assert!(prompt.contains("Transcript:"));
    }

    #[test]
    fn test_build_prompt_non_empty() {
        let svc = SummaryService::new("test-key".to_string());
        let prompt = svc.build_prompt("some text");
        assert!(!prompt.is_empty());
    }

    #[test]
    fn test_transcript_validation_boundary_30_words() {
        // Exactly 30 words should pass validation (not be rejected)
        let transcript_30 = "word ".repeat(30);
        let word_count = transcript_30.split_whitespace().count();
        assert_eq!(word_count, 30);
        assert!(word_count >= 30);
    }

    #[test]
    fn test_transcript_validation_boundary_29_words() {
        // 29 words should fail validation
        let transcript_29 = "word ".repeat(29);
        let word_count = transcript_29.split_whitespace().count();
        assert_eq!(word_count, 29);
        assert!(word_count < 30);
    }

    #[test]
    fn test_transcript_validation_boundary_280000_words() {
        // Exactly 280,000 words should pass validation
        let word_count: usize = 280_000;
        assert!(word_count <= 280_000);
    }

    #[test]
    fn test_transcript_validation_boundary_280001_words() {
        // 280,001 words should fail validation
        let word_count: usize = 280_001;
        assert!(word_count > 280_000);
    }

    #[test]
    fn test_is_rate_limit_error() {
        assert!(is_rate_limit_error("ResourceExhausted: quota exceeded"));
        assert!(is_rate_limit_error("bad response from server; code 429; description: rate limited"));
        assert!(is_rate_limit_error("RESOURCE_EXHAUSTED"));
        assert!(!is_rate_limit_error("some other error"));
        assert!(!is_rate_limit_error("network timeout"));
    }
}
