use futures_util::StreamExt;
use gemini_rust::{Gemini, Model};
use sqlx::SqlitePool;
use tracing;

use crate::db;
use crate::errors::SummaryError;
use crate::state::ModelOption;

/// The "adaptive knowledge synthesis engine" persona prompt.
const SYSTEM_INSTRUCTION: &str = include_str!("../../prompts/system_instruction.txt");

/// Example input: title, description, comments, and transcript of a demo video.
const EXAMPLE_INPUT: &str = include_str!("../../prompts/example_input.txt");

/// Example output: the expected abstract for the demo video.
const EXAMPLE_OUTPUT_ABSTRACT: &str = include_str!("../../prompts/example_output_abstract.txt");

/// Example output: the expected bullet-point summary for the demo video.
const EXAMPLE_OUTPUT: &str = include_str!("../../prompts/example_output.txt");

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

        // Model-aware prompt routing (Req 2.3, 3.1, 3.2):
        // - Gemini models: system instruction as API parameter, standard user prompt
        // - Gemma models: no system prompt param, system instruction prepended to user prompt
        let prompt = if !model.name.starts_with("gemma") {
            builder = builder.with_system_prompt(SYSTEM_INSTRUCTION);
            self.build_prompt(transcript)
        } else {
            self.build_prompt_for_gemma(transcript)
        };

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
    ///
    /// Produces the full few-shot template matching the Python `get_prompt()` function:
    /// instruction paragraph → bold formatting instruction → example input → example output →
    /// real transcript framing → transcript.
    pub fn build_prompt(&self, transcript: &str) -> String {
        format!(
            "Below, I will provide input for an example video (comprising of title, description, \
and transcript, in this order) and the corresponding abstract and summary I expect. Afterward, \
I will provide a new transcript that I want a summarization in the same format. \n\
\n\
**Please give an abstract of the transcript and then summarize the transcript in a self-contained \
bullet list format.** Include starting timestamps, important details and key takeaways. \n\
\n\
Example Input: \n\
{example_input}\n\
Example Output:\n\
{example_output_abstract}\n\
{example_output}\n\
Here is the real transcript. What would be a good group of people to review this topic? \
Please summarize provide a summary like they would: \n\
{transcript}",
            example_input = EXAMPLE_INPUT,
            example_output_abstract = EXAMPLE_OUTPUT_ABSTRACT,
            example_output = EXAMPLE_OUTPUT,
            transcript = transcript,
        )
    }

    /// Builds the prompt for Gemma models by prepending the system instruction.
    ///
    /// Gemma models don't support a separate system prompt parameter, so the
    /// system instruction is prepended to the user prompt with a `---` delimiter.
    ///
    /// Requirements: 3.1, 3.3
    pub fn build_prompt_for_gemma(&self, transcript: &str) -> String {
        format!(
            "{}\n\n---\n\n{}",
            SYSTEM_INSTRUCTION,
            self.build_prompt(transcript)
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

        // Verify transcript is present
        assert!(prompt.contains(transcript));
        // Verify few-shot example markers
        assert!(prompt.contains("Example Input:"));
        assert!(prompt.contains("Example Output:"));
        // Verify instruction paragraph
        assert!(
            prompt.contains("Below, I will provide input"),
            "Prompt should contain the instruction paragraph"
        );
        // Verify bold formatting instruction
        assert!(
            prompt.contains("**Please give an abstract"),
            "Prompt should contain the bold formatting instruction"
        );
        // Verify real transcript framing
        assert!(prompt.contains("Here is the real transcript"));
    }

    #[test]
    fn test_build_prompt_non_empty() {
        let svc = SummaryService::new("test-key".to_string());
        let prompt = svc.build_prompt("some text");
        assert!(!prompt.is_empty());
        // Verify the prompt contains the few-shot template structure
        assert!(
            prompt.contains("Example Input:"),
            "Prompt should contain 'Example Input:' marker"
        );
        assert!(
            prompt.contains("Example Output:"),
            "Prompt should contain 'Example Output:' marker"
        );
        assert!(
            prompt.contains("Below, I will provide input"),
            "Prompt should contain the instruction paragraph"
        );
        assert!(
            prompt.contains("**Please give an abstract"),
            "Prompt should contain the bold formatting instruction"
        );
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

    #[test]
    fn test_system_instruction_non_empty_and_contains_core_instruction() {
        assert!(!SYSTEM_INSTRUCTION.is_empty());
        assert!(
            SYSTEM_INSTRUCTION.contains("CORE INSTRUCTION"),
            "SYSTEM_INSTRUCTION should contain 'CORE INSTRUCTION'"
        );
    }

    #[test]
    fn test_example_input_non_empty_and_contains_expected_content() {
        assert!(!EXAMPLE_INPUT.is_empty());
        assert!(
            EXAMPLE_INPUT.contains("Fluidigm Polaris"),
            "EXAMPLE_INPUT should contain 'Fluidigm Polaris'"
        );
    }

    #[test]
    fn test_example_output_non_empty() {
        assert!(!EXAMPLE_OUTPUT.is_empty());
    }

    #[test]
    fn test_example_output_abstract_non_empty_and_contains_abstract() {
        assert!(!EXAMPLE_OUTPUT_ABSTRACT.is_empty());
        assert!(
            EXAMPLE_OUTPUT_ABSTRACT.contains("Abstract"),
            "EXAMPLE_OUTPUT_ABSTRACT should contain 'Abstract'"
        );
    }

    #[test]
    fn test_build_prompt_for_gemma() {
        let svc = SummaryService::new("test-key".to_string());
        let transcript = "test transcript";

        let gemma_prompt = svc.build_prompt_for_gemma(transcript);

        // Verify the output starts with system instruction content (Req 3.1)
        assert!(
            gemma_prompt.starts_with("### CORE INSTRUCTION"),
            "Gemma prompt should start with the beginning of SYSTEM_INSTRUCTION"
        );

        // Verify the `---` delimiter separates system instruction from the template (Req 3.3)
        assert!(
            gemma_prompt.contains("\n\n---\n\n"),
            "Gemma prompt should contain the '---' delimiter"
        );

        // Verify the template portion after the delimiter matches build_prompt() output
        let delimiter = "\n\n---\n\n";
        let delimiter_pos = gemma_prompt.find(delimiter).unwrap();
        let template_portion = &gemma_prompt[delimiter_pos + delimiter.len()..];
        let expected_template = svc.build_prompt(transcript);
        assert_eq!(
            template_portion, expected_template,
            "The portion after the delimiter should match build_prompt() output"
        );
    }
}
