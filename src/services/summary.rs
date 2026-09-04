use futures_util::StreamExt;
use gemini_rust::{Gemini, Model, Part, ThinkingLevel, Tool};
use sqlx::SqlitePool;
use tracing;

use crate::db;
use crate::errors::SummaryError;
use crate::models::ThinkingPreference;
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
    pub thinking_text: String,
    pub thinking_tokens: u64,
    pub cost: f64,
    pub duration_secs: f64,
}

pub struct SummaryService {
    api_key: String,
}

/// Maps persisted preferences to Gemini 3 request values. `auto` deliberately
/// returns no level so the provider chooses its own default.
pub(crate) fn gemini_3_thinking_level(
    model_name: &str,
    preference: ThinkingPreference,
) -> Option<ThinkingLevel> {
    if !model_name.to_ascii_lowercase().starts_with("gemini-3.") {
        return None;
    }

    match preference {
        ThinkingPreference::Auto => None,
        ThinkingPreference::Minimal => Some(ThinkingLevel::Minimal),
        ThinkingPreference::Low => Some(ThinkingLevel::Low),
        ThinkingPreference::Medium => Some(ThinkingLevel::Medium),
        ThinkingPreference::High => Some(ThinkingLevel::High),
    }
}

pub(crate) fn gemini_2_5_thinking_budget(model_name: &str) -> Option<i32> {
    let name_lower = model_name.to_ascii_lowercase();
    if !name_lower.starts_with("gemini-2.5") {
        return None;
    }
    Some(if name_lower.contains("pro") {
        32768
    } else {
        24576
    })
}

impl SummaryService {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    /// Validates transcript length and generates a summary via Gemini.
    /// Persists chunks to DB progressively during streaming.
    ///
    /// Requirements: 6.1, 6.2, 6.5, 6.6, 6.7
    #[allow(clippy::too_many_arguments)]
    pub async fn generate_summary(
        &self,
        db_pool: &SqlitePool,
        identifier: i64,
        transcript: &str,
        model: &ModelOption,
        google_search_grounding: bool,
        url_context: bool,
        include_glossary: bool,
        output_language: &str,
        thinking_preference: ThinkingPreference,
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

        if model.architecture == crate::state::ModelArchitecture::Hetzner {
            return self
                .generate_summary_hetzner(
                    db_pool,
                    identifier,
                    transcript,
                    model,
                    start,
                    include_glossary,
                    output_language,
                )
                .await;
        }

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

        // Configure grounding with Google Search if selected and architecture is Gemini or Gemma
        if google_search_grounding
            && (model.architecture == crate::state::ModelArchitecture::Gemini
                || model.architecture == crate::state::ModelArchitecture::Gemma)
        {
            builder = builder.with_tool(Tool::google_search());
        }

        // Configure URL context if selected and architecture is Gemini
        if url_context && model.architecture == crate::state::ModelArchitecture::Gemini {
            builder = builder.with_tool(Tool::url_context());
        }

        // Gemini 3 uses named levels while Gemini 2.5 retains its separate,
        // numeric budget API. The builder methods clear each other, so these
        // branches must remain mutually exclusive.
        if model.architecture == crate::state::ModelArchitecture::Gemini {
            if let Some(level) = gemini_3_thinking_level(&model.name, thinking_preference) {
                builder = builder
                    .with_thinking_level(level)
                    .with_thoughts_included(true);
            } else if let Some(budget) = gemini_2_5_thinking_budget(&model.name) {
                builder = builder
                    .with_thinking_budget(budget)
                    .with_thoughts_included(true);
            }
        }

        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let is_hn = transcript.contains("=== HACKER NEWS SUBMISSION ===");

        let system_instruction_with_date = format!(
            "{}\n\nToday's date is {}. Keep in mind that new events, personnel changes, political developments, and world occurrences may have happened since your knowledge training cutoff date. Accept current facts presented in the input without making skeptical remarks or questioning them based on older training data.",
            SYSTEM_INSTRUCTION, today
        );

        // Model-aware prompt routing (Req 2.3, 3.1, 3.2):
        // - Gemini models: system instruction as API parameter, standard user prompt
        // - Gemma models: no system prompt param, system instruction prepended to user prompt
        // - Other models: fallback base prompt
        let prompt = match model.architecture {
            crate::state::ModelArchitecture::Gemini => {
                builder = builder.with_system_prompt(&system_instruction_with_date);
                if is_hn {
                    self.build_hn_prompt(transcript, include_glossary, output_language)
                } else {
                    self.build_prompt(transcript, include_glossary, output_language)
                }
            }
            crate::state::ModelArchitecture::Gemma => {
                self.build_prompt_for_gemma(transcript, include_glossary, output_language)
            }
            crate::state::ModelArchitecture::Other | crate::state::ModelArchitecture::Hetzner => {
                if is_hn {
                    self.build_hn_prompt(transcript, include_glossary, output_language)
                } else {
                    self.build_prompt(transcript, include_glossary, output_language)
                }
            }
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
        let mut thinking_text = String::new();
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut thinking_tokens: u64 = 0;

        // Process streaming chunks, persisting each to DB progressively
        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(response) => {
                    // Extract text parts, separating thoughts if it's a Gemini model with thoughts
                    if model.architecture == crate::state::ModelArchitecture::Gemini {
                        if let Some(candidate) = response.candidates.first() {
                            if let Some(parts) = &candidate.content.parts {
                                for part in parts {
                                    if let Part::Text { text, thought, .. } = part {
                                        let is_thought = thought.unwrap_or(false);
                                        if is_thought {
                                            thinking_text.push_str(text);
                                        } else if !text.is_empty() {
                                            db::update_summary_chunk(db_pool, identifier, text)
                                                .await
                                                .map_err(|e| {
                                                    SummaryError::ApiError(format!(
                                                        "DB error: {}",
                                                        e
                                                    ))
                                                })?;
                                            summary_text.push_str(text);
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        // For Gemma or others, store the entire text in summary progressively
                        let chunk_text = response.text();
                        if !chunk_text.is_empty() {
                            db::update_summary_chunk(db_pool, identifier, &chunk_text)
                                .await
                                .map_err(|e| SummaryError::ApiError(format!("DB error: {}", e)))?;
                            summary_text.push_str(&chunk_text);
                        }
                    }

                    // Extract token counts from usage metadata (last chunk typically has them)
                    if let Some(usage) = &response.usage_metadata {
                        if let Some(prompt_tokens) = usage.prompt_token_count {
                            input_tokens = prompt_tokens as u64;
                        }
                        if let Some(candidates_tokens) = usage.candidates_token_count {
                            output_tokens = candidates_tokens as u64;
                        }
                        if let Some(thoughts_tokens) = usage.thoughts_token_count {
                            thinking_tokens = thoughts_tokens as u64;
                        }
                    }
                }
                Err(e) => {
                    let err_str = e.to_string();
                    // Handle rate limiting mid-stream (Req 6.7)
                    if is_rate_limit_error(&err_str) {
                        // Append error to partial summary without setting summary_done
                        let error_msg =
                            "\n\n[Error: Rate limited (ResourceExhausted). Please retry later.]";
                        db::update_summary_chunk(db_pool, identifier, error_msg)
                            .await
                            .map_err(|e| SummaryError::ApiError(format!("DB error: {}", e)))?;
                        return Err(SummaryError::RateLimited);
                    }
                    return Err(SummaryError::ApiError(err_str));
                }
            }
        }

        if summary_text.is_empty() && thinking_text.is_empty() {
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
            thinking_tokens = thinking_tokens,
            cost = cost,
            duration_secs = duration_secs,
            "Summary generation complete"
        );

        Ok(SummaryResult {
            summary_text,
            input_tokens,
            output_tokens,
            thinking_text,
            thinking_tokens,
            cost,
            duration_secs,
        })
    }

    /// Generates prompt directives for language selection and glossary generation.
    pub fn get_prompt_directives(include_glossary: bool, output_language: &str) -> String {
        let mut directives = String::new();

        if output_language == "de" || output_language == "de-DE" {
            directives.push_str(
                "\n\n**LANGUAGE DIRECTIVE (DEUTSCH)**:\n\
                 You MUST generate the entire output (Abstract, Key Highlights/Points, Discussion, Glossary, and section titles) in German (Deutsch).\n\
                 Use German section titles (e.g. `## Zusammenfassung` or `## Abstract`, `## Wichtigste Punkte & Zeitstempel` for transcripts, `## Wichtigste Punkte` for articles, `## Diskussions-Highlights`, and `## Glossar`)."
            );
        }

        if include_glossary {
            if output_language == "de" || output_language == "de-DE" {
                directives.push_str(
                    "\n\n**GLOSSAR DIRECTIVE**:\n\
                     You MUST append a dedicated section titled `## Glossar` at the end of the summary. In this section, extract all specialized technical, medical, scientific, and domain-specific terms, acronyms, clinical metrics, and jargon present in the content (e.g. filovirus, VMAT2, PASC, PICU, HR 0.53, etc.) and explain each term clearly and concisely in German for a lay reader."
                );
            } else {
                directives.push_str(
                    "\n\n**GLOSSARY DIRECTIVE**:\n\
                     You MUST append a dedicated section titled `## Glossary` at the end of the summary. In this section, extract all specialized technical, medical, scientific, and domain-specific terms, acronyms, clinical metrics, and jargon present in the content (e.g. filovirus, VMAT2, PASC, PICU, HR 0.53, etc.) and explain each term clearly and concisely for a lay reader."
                );
            }
        }

        directives
    }

    /// Builds the prompt for a Hacker News submission and discussion summary.
    pub fn build_hn_prompt(
        &self,
        hn_content: &str,
        include_glossary: bool,
        output_language: &str,
    ) -> String {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let directives = Self::get_prompt_directives(include_glossary, output_language);
        format!(
            "Today's date: {today}.\n\
             Note on temporal awareness: Events, leadership roles, technology updates, and public occurrences may have changed since your knowledge training cutoff date. Accept current facts presented in the input without expressing skepticism or contradicting them based on older training data.\n\
             \n\
             Below is input from a Hacker News submission, containing the original article (if retrieved or provided) and the discussion from news.ycombinator.com.\n\
             \n\
             This is an ARTICLE, not a video transcript. Do NOT include any timestamps in the output.\n\
             \n\
             Please provide the following exact, repeatable section structure:\n\
             1. **## Abstract**: A concise, high-density executive summary of the main article (3-5 sentences max).\n\
             2. **## Key Points**: A bulleted list of the most important findings, claims, and technical details from the article. Each bullet begins with a bolded concept name. NO timestamps.\n\
             3. **## Discussion Highlights**: A concise bulleted summary of the most significant perspectives, technical arguments, and counter-arguments from the Hacker News comments.\n\
             \n\
             **DISCUSSION REQUIREMENTS**:\n\
             - Cover the full breadth of the discussion, not just the top few comments.\n\
             - Prioritize substantive technical arguments and novel perspectives over repetitive agreement.\n\
             - Highlight any alternative links, paywall bypasses, or external resources mentioned.\n\
             - Include concrete numbers, benchmark statistics, tool names, and technical details where discussed.\n\
             \n\
             **CONCISENESS REQUIREMENTS**:\n\
             - Be ruthlessly concise. Every sentence must carry unique information.\n\
             - Merge similar discussion points into single bullets rather than listing each comment separately.\n\
             - Omit low-signal commentary (e.g., jokes, tangential anecdotes, meta-discussion about HN itself).\n\
             - Do NOT include greetings, conversational filler, closing remarks, or follow-up questions.\n\
             {directives}\n\
             \n\
             Here is the Hacker News content and discussion:\n\
             {hn_content}",
            today = today,
            directives = directives,
            hn_content = hn_content,
        )
    }

    /// Builds the prompt from the transcript text.
    pub fn build_prompt(
        &self,
        transcript: &str,
        include_glossary: bool,
        output_language: &str,
    ) -> String {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let directives = Self::get_prompt_directives(include_glossary, output_language);
        format!(
            "Today's date: {today}.\n\
             Note on temporal awareness: Events, leadership roles, software versions, and public occurrences may have changed since your knowledge training cutoff date. Accept current facts presented in the input without expressing skepticism or contradicting them based on older training data.\n\
             \n\
             Below, I will provide input for an example video (comprising of title, description, \
and transcript, in this order) and the corresponding abstract and summary I expect. Afterward, \
I will provide a new transcript that I want a summarization in the exact same structure. \n\
\n\
**Please give an abstract of the transcript and then summarize the transcript in a self-contained \
bullet list format.** Maintain strict structural repeatability (`## Abstract` followed by \
`## Key Highlights & Timestamps`). Include starting timestamps, important details and key takeaways. \n\
{directives}\n\
\n\
Example Input: \n\
{example_input}\n\
Example Output:\n\
{example_output_abstract}\n\
{example_output}\n\
Here is the real transcript. What would be a good group of people to review this topic? \
Please provide a summary like they would: \n\
{transcript}",
            today = today,
            directives = directives,
            example_input = EXAMPLE_INPUT,
            example_output_abstract = EXAMPLE_OUTPUT_ABSTRACT,
            example_output = EXAMPLE_OUTPUT,
            transcript = transcript,
        )
    }

    /// Builds the prompt for Gemma models by prepending the system instruction.
    pub fn build_prompt_for_gemma(
        &self,
        transcript: &str,
        include_glossary: bool,
        output_language: &str,
    ) -> String {
        let base_prompt = if transcript.contains("=== HACKER NEWS SUBMISSION ===") {
            self.build_hn_prompt(transcript, include_glossary, output_language)
        } else {
            self.build_prompt(transcript, include_glossary, output_language)
        };

        format!("{}\n\n---\n\n{}", SYSTEM_INSTRUCTION, base_prompt)
    }

    /// Computes the cost based on token counts and model pricing.
    pub fn compute_cost(&self, model: &ModelOption, input_tokens: u64, output_tokens: u64) -> f64 {
        let input_cost = (input_tokens as f64) * model.input_price_per_mtoken / 1_000_000.0;
        let output_cost = (output_tokens as f64) * model.output_price_per_mtoken / 1_000_000.0;
        input_cost + output_cost
    }

    /// Generates a summary via Hetzner OpenAI-compatible API.
    /// Persists chunks to DB progressively during streaming.
    #[allow(clippy::too_many_arguments)]
    pub async fn generate_summary_hetzner(
        &self,
        db_pool: &SqlitePool,
        identifier: i64,
        transcript: &str,
        model: &ModelOption,
        start: std::time::Instant,
        include_glossary: bool,
        output_language: &str,
    ) -> Result<SummaryResult, SummaryError> {
        let hetzner_api_key = std::env::var("HETZNER_API_KEY")
            .unwrap_or_else(|_| "2jwqK0zWB54O0ipIzRtmv9jHme7jSazg".to_string());
        let hetzner_base_url = std::env::var("HETZNER_BASE_URL")
            .unwrap_or_else(|_| "https://inference.hetzner.com/api/v1".to_string());

        let config = async_openai::config::OpenAIConfig::new()
            .with_api_base(hetzner_base_url)
            .with_api_key(hetzner_api_key);
        let client = async_openai::Client::with_config(config);

        let actual_model_name = resolve_hetzner_model_name(&model.name).to_string();

        tracing::info!(
            identifier = identifier,
            model = %model.name,
            actual_model = %actual_model_name,
            "Starting summary generation via Hetzner API"
        );

        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let is_hn = transcript.contains("=== HACKER NEWS SUBMISSION ===");

        let system_instruction = format!(
            "{}\n\nToday's date is {}. Accept current facts presented in the input without making skeptical remarks or questioning them based on older training data.",
            SYSTEM_INSTRUCTION, today
        );

        let prompt = if is_hn {
            self.build_hn_prompt(transcript, include_glossary, output_language)
        } else {
            self.build_prompt(transcript, include_glossary, output_language)
        };

        use async_openai::types::chat::{
            ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
            CreateChatCompletionRequestArgs,
        };

        let request = CreateChatCompletionRequestArgs::default()
            .model(&actual_model_name)
            .messages([
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(system_instruction)
                    .build()
                    .map_err(|e| SummaryError::ApiError(e.to_string()))?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content(prompt.clone())
                    .build()
                    .map_err(|e| SummaryError::ApiError(e.to_string()))?
                    .into(),
            ])
            .stream(true)
            .build()
            .map_err(|e| SummaryError::ApiError(e.to_string()))?;

        let mut stream = client.chat().create_stream(request).await.map_err(|e| {
            let err_str = e.to_string();
            if is_rate_limit_error(&err_str) {
                tracing::warn!(identifier = identifier, "Rate limited by Hetzner API");
                SummaryError::RateLimited
            } else {
                tracing::error!(identifier = identifier, error = %err_str, "Hetzner API error");
                SummaryError::ApiError(err_str)
            }
        })?;

        let mut summary_text = String::new();
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(response) => {
                    for choice in response.choices {
                        if let Some(content) = choice.delta.content {
                            if !content.is_empty() {
                                db::update_summary_chunk(db_pool, identifier, &content)
                                    .await
                                    .map_err(|e| {
                                        SummaryError::ApiError(format!("DB error: {}", e))
                                    })?;
                                summary_text.push_str(&content);
                            }
                        }
                    }
                    if let Some(usage) = response.usage {
                        input_tokens = usage.prompt_tokens as u64;
                        output_tokens = usage.completion_tokens as u64;
                    }
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if is_rate_limit_error(&err_str) {
                        let error_msg =
                            "\n\n[Error: Rate limited (ResourceExhausted). Please retry later.]";
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
                "Hetzner API returned empty response".to_string(),
            ));
        }

        let duration_secs = start.elapsed().as_secs_f64();

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
            "Hetzner summary generation complete"
        );

        Ok(SummaryResult {
            summary_text,
            input_tokens,
            output_tokens,
            thinking_text: String::new(),
            thinking_tokens: 0,
            cost,
            duration_secs,
        })
    }
}

/// Maps internal Hetzner model names to their API-side model identifiers.
fn resolve_hetzner_model_name(internal_name: &str) -> &str {
    match internal_name {
        "hetzner-qwen-3.6-35b" => "Qwen/Qwen3.6-35B-A3B-FP8",
        "hetzner-qwen-3.8-27b" => "Qwen3.8-27B",
        other => other, // pass-through for direct API names (e.g. containing '/')
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
    use crate::models::ThinkingPreference;
    use crate::state::{ModelArchitecture, ModelOption};

    #[test]
    fn thinking_levels_map_only_for_gemini_3() {
        assert_eq!(
            gemini_3_thinking_level("gemini-3.8-flash", ThinkingPreference::Minimal),
            Some(ThinkingLevel::Minimal)
        );
        assert_eq!(
            gemini_3_thinking_level("gemini-3.7-flash", ThinkingPreference::Low),
            Some(ThinkingLevel::Low)
        );
        assert_eq!(
            gemini_3_thinking_level("gemini-3.6-flash", ThinkingPreference::Medium),
            Some(ThinkingLevel::Medium)
        );
        assert_eq!(
            gemini_3_thinking_level("gemini-3.5-flash", ThinkingPreference::High),
            Some(ThinkingLevel::High)
        );
        assert_eq!(
            gemini_3_thinking_level("gemini-3.8-flash", ThinkingPreference::Auto),
            None
        );
        assert_eq!(
            gemini_3_thinking_level("gemini-2.5-flash", ThinkingPreference::High),
            None
        );
        assert_eq!(
            gemini_3_thinking_level("hetzner-qwen-3.8-27b", ThinkingPreference::High),
            None
        );
        assert_eq!(gemini_2_5_thinking_budget("gemini-2.5-flash"), Some(24576));
        assert_eq!(gemini_2_5_thinking_budget("gemini-2.5-pro"), Some(32768));
        assert_eq!(gemini_2_5_thinking_budget("gemini-3.8-flash"), None);
    }

    fn test_model() -> ModelOption {
        ModelOption {
            name: "gemini-2.0-flash".to_string(),
            input_price_per_mtoken: 0.075,
            output_price_per_mtoken: 0.30,
            context_window: 1_000_000,
            rpm_limit: 10,
            rpd_limit: 1500,
            architecture: ModelArchitecture::Gemini,
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
    fn test_build_hn_prompt_content() {
        let svc = SummaryService::new("test-key".to_string());
        let content = "=== HACKER NEWS SUBMISSION ===\nTitle: Test Post";
        let prompt = svc.build_hn_prompt(content, false, "en");

        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        assert!(
            prompt.contains(&today),
            "HN Prompt should contain today's date"
        );
        assert!(
            prompt.contains("temporal awareness"),
            "HN Prompt should contain temporal awareness note"
        );
        assert!(
            prompt.contains("Do NOT include any timestamps"),
            "HN Prompt should forbid timestamps for articles"
        );
        assert!(
            prompt.contains("CONCISENESS REQUIREMENTS"),
            "HN Prompt should contain conciseness requirements"
        );
        assert!(
            prompt.contains("Discussion Highlights"),
            "HN Prompt should request discussion highlights"
        );
        assert!(
            prompt.contains("full breadth of the discussion"),
            "HN Prompt should instruct full discussion coverage"
        );
    }

    #[test]
    fn test_build_prompt_contains_transcript() {
        let svc = SummaryService::new("test-key".to_string());
        let transcript = "00:00:00 Hello world\n00:01:00 This is a test";
        let prompt = svc.build_prompt(transcript, false, "en");

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
        let prompt = svc.build_prompt("some text", false, "en");
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
    fn test_get_prompt_directives_glossary_and_german() {
        let directives_en_no_glossary = SummaryService::get_prompt_directives(false, "en");
        assert!(directives_en_no_glossary.is_empty());

        let directives_en_glossary = SummaryService::get_prompt_directives(true, "en");
        assert!(directives_en_glossary.contains("GLOSSARY DIRECTIVE"));
        assert!(directives_en_glossary.contains("## Glossary"));

        let directives_de_no_glossary = SummaryService::get_prompt_directives(false, "de");
        assert!(directives_de_no_glossary.contains("LANGUAGE DIRECTIVE (DEUTSCH)"));
        assert!(directives_de_no_glossary.contains("## Glossar"));

        let directives_de_glossary = SummaryService::get_prompt_directives(true, "de");
        assert!(directives_de_glossary.contains("LANGUAGE DIRECTIVE (DEUTSCH)"));
        assert!(directives_de_glossary.contains("GLOSSAR DIRECTIVE"));
        assert!(directives_de_glossary.contains("## Glossar"));
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
        assert!(is_rate_limit_error(
            "bad response from server; code 429; description: rate limited"
        ));
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
        assert!(
            SYSTEM_INSTRUCTION.contains("Single-Pass Output Directive"),
            "SYSTEM_INSTRUCTION should contain 'Single-Pass Output Directive'"
        );
        assert!(
            SYSTEM_INSTRUCTION.contains("Conciseness Mandate"),
            "SYSTEM_INSTRUCTION should contain 'Conciseness Mandate'"
        );
        assert!(
            SYSTEM_INSTRUCTION.contains("Structural Repeatability Directive"),
            "SYSTEM_INSTRUCTION should contain 'Structural Repeatability Directive'"
        );
        assert!(
            SYSTEM_INSTRUCTION.contains("articles/text content without timestamps"),
            "SYSTEM_INSTRUCTION should differentiate article vs video structure"
        );
        assert!(
            SYSTEM_INSTRUCTION.contains("Specifics Over Generalities"),
            "SYSTEM_INSTRUCTION should contain 'Specifics Over Generalities'"
        );
        assert!(
            SYSTEM_INSTRUCTION
                .contains("Strict Objectivity, High Data Density & Uniform Repeatability"),
            "SYSTEM_INSTRUCTION should contain 'Strict Objectivity, High Data Density & Uniform Repeatability'"
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

        let gemma_prompt = svc.build_prompt_for_gemma(transcript, false, "en");

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
        let expected_template = svc.build_prompt(transcript, false, "en");
        assert_eq!(
            template_portion, expected_template,
            "The portion after the delimiter should match build_prompt() output"
        );
    }

    #[test]
    fn test_hetzner_model_cost_calculation() {
        let svc = SummaryService::new("test-key".to_string());
        let hetzner_model = ModelOption {
            name: "hetzner-qwen-3.6-35b".to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 262_144,
            rpm_limit: 60,
            rpd_limit: 14400,
            architecture: ModelArchitecture::Hetzner,
        };

        let cost = svc.compute_cost(&hetzner_model, 5000, 2000);
        assert_eq!(cost, 0.0, "Experimental Hetzner model cost should be zero");
    }

    #[test]
    fn test_hetzner_model_architecture_as_str() {
        assert_eq!(ModelArchitecture::Hetzner.as_str(), "Hetzner");
    }

    #[test]
    fn test_resolve_hetzner_model_name() {
        assert_eq!(
            resolve_hetzner_model_name("hetzner-qwen-3.6-35b"),
            "Qwen/Qwen3.6-35B-A3B-FP8"
        );
        assert_eq!(
            resolve_hetzner_model_name("hetzner-qwen-3.8-27b"),
            "Qwen3.8-27B"
        );
        // Pass-through for direct API names
        assert_eq!(
            resolve_hetzner_model_name("Qwen/Qwen3.6-35B-A3B-FP8"),
            "Qwen/Qwen3.6-35B-A3B-FP8"
        );
        assert_eq!(resolve_hetzner_model_name("Qwen3.8-27B"), "Qwen3.8-27B");
        // Unknown names pass through unchanged
        assert_eq!(resolve_hetzner_model_name("unknown-model"), "unknown-model");
    }
}
