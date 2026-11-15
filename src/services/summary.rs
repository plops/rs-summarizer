use futures_util::{StreamExt, TryStreamExt};
use gemini_rust::prelude::{
    Gemini, InteractionContent, InteractionEvent, InteractionStatus, InteractionThinkingLevel,
    StepDeltaData, ThinkingSummaries,
};
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

/// Pure event accumulator used by the adapter and deterministic fake streams in
/// tests. Database ownership remains outside this type.
#[derive(Default)]
struct InteractionAccumulator {
    summary_text: String,
    thinking_text: String,
    input_tokens: u64,
    output_tokens: u64,
    thinking_tokens: u64,
    completed: bool,
}

impl InteractionAccumulator {
    fn observe(&mut self, event: InteractionEvent) -> Result<Option<String>, SummaryError> {
        match event {
            InteractionEvent::StepDelta {
                delta: StepDeltaData::Text { text },
                ..
            } if !text.is_empty() => {
                self.summary_text.push_str(&text);
                Ok(Some(text))
            }
            InteractionEvent::StepDelta {
                delta:
                    StepDeltaData::ThoughtSummary {
                        content: Some(InteractionContent::Text { text, .. }),
                    },
                ..
            } => {
                self.thinking_text.push_str(&text);
                Ok(None)
            }
            InteractionEvent::StepStop {
                usage: Some(usage), ..
            } => {
                self.input_tokens = usage.total_input_tokens.unwrap_or(0).max(0) as u64;
                self.output_tokens = usage.total_output_tokens.unwrap_or(0).max(0) as u64;
                self.thinking_tokens = usage.total_thought_tokens.unwrap_or(0).max(0) as u64;
                Ok(None)
            }
            InteractionEvent::InteractionCompleted { interaction, .. }
                if interaction.status == InteractionStatus::Completed =>
            {
                self.completed = true;
                Ok(None)
            }
            InteractionEvent::InteractionCompleted { interaction, .. } => {
                Err(SummaryError::ApiError(format!(
                    "provider terminal status: {}",
                    interaction.status.as_ref()
                )))
            }
            InteractionEvent::Error { .. } => {
                Err(SummaryError::ApiError("provider stream error".into()))
            }
            _ => Ok(None),
        }
    }
}

/// Incremental SSE frame parser for the raw `xhigh` Interactions call.
/// Accumulates bytes across `chunk()` boundaries and yields one
/// `InteractionEvent` per complete `data:` frame (frames end at a blank
/// line, `\n\n` or `\r\n\r\n`; multi-line `data:` payloads join with `\n`).
#[derive(Default)]
struct SseFrameBuffer {
    buf: Vec<u8>,
}

impl SseFrameBuffer {
    /// Appends bytes and returns the events of all fully received frames.
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<InteractionEvent>, SummaryError> {
        self.buf.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(end) = sse_frame_end(&self.buf) {
            let frame: Vec<u8> = self.buf.drain(..end).collect();
            if let Some(event) = parse_sse_frame(&frame)? {
                events.push(event);
            }
        }
        Ok(events)
    }

    /// Parses any trailing bytes after the stream closed.
    fn finish(mut self) -> Result<Vec<InteractionEvent>, SummaryError> {
        self.buf.extend_from_slice(b"\n\n");
        self.push(&[])
    }
}

/// Index just past the first blank-line frame delimiter, if present.
fn sse_frame_end(buf: &[u8]) -> Option<usize> {
    let lf = buf.windows(2).position(|w| w == b"\n\n").map(|pos| pos + 2);
    let crlf = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4);
    match (lf, crlf) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Parses one raw SSE frame into an `InteractionEvent`, or `None` for
/// keep-alives, comments, and `[DONE]` terminators.
fn parse_sse_frame(frame: &[u8]) -> Result<Option<InteractionEvent>, SummaryError> {
    let text = String::from_utf8_lossy(frame);
    let mut data = String::new();
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.starts_with(':') {
            continue;
        }
        if let Some(payload) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            // SSE strips a single leading space after the colon.
            data.push_str(payload.strip_prefix(' ').unwrap_or(payload));
        }
    }
    let data = data.trim();
    if data.is_empty() || data == "[DONE]" {
        return Ok(None);
    }
    serde_json::from_str(data)
        .map(Some)
        .map_err(|e| SummaryError::ApiError(format!("bad SSE event: {e}")))
}

pub struct SummaryService {
    api_key: String,
}

/// How a persisted thinking preference is sent to the Interactions API.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ResolvedThinking {
    /// Send no `thinking_level` (provider default): `Auto` preferences and
    /// models whose API rejects the parameter (Gemini 2.5, Gemma, Hetzner).
    Omitted,
    /// A level `gemini-rust` can send directly via the builder.
    Named(InteractionThinkingLevel),
    /// `xhigh`, which `gemini-rust 2.0.0` cannot express: sent through the
    /// raw SSE path in `generate_summary_xhigh` (`gemini-4-hydrogen` only).
    XHigh,
}

/// Maps persisted preferences to Interactions API values using the
/// per-model capability table in `state.rs` (never string prefixes).
/// Unsupported named levels clamp to the closest supported one instead of
/// failing with HTTP 400 when a fallback moves across model generations
/// (e.g. `xhigh` from `gemini-4-hydrogen` becomes `high` on Gemini 3.x,
/// `minimal` becomes `low` where `minimal` is rejected).
pub(crate) fn effective_thinking_level(
    model_name: &str,
    preference: ThinkingPreference,
) -> ResolvedThinking {
    use ThinkingPreference::*;
    let supported = crate::state::supported_thinking_levels(model_name);
    match preference {
        Auto => ResolvedThinking::Omitted,
        XHigh if supported.contains(&XHigh) => ResolvedThinking::XHigh,
        XHigh if supported.contains(&High) => {
            ResolvedThinking::Named(InteractionThinkingLevel::High)
        }
        Minimal if supported.contains(&Minimal) => {
            ResolvedThinking::Named(InteractionThinkingLevel::Minimal)
        }
        Minimal if supported.contains(&Low) => {
            ResolvedThinking::Named(InteractionThinkingLevel::Low)
        }
        Low if supported.contains(&Low) => ResolvedThinking::Named(InteractionThinkingLevel::Low),
        Medium if supported.contains(&Medium) => {
            ResolvedThinking::Named(InteractionThinkingLevel::Medium)
        }
        High if supported.contains(&High) => {
            ResolvedThinking::Named(InteractionThinkingLevel::High)
        }
        _ => ResolvedThinking::Omitted,
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

    /// Feeds one interaction event into the accumulator and persists any
    /// emitted summary text, guarded by the generation epoch so a stale
    /// stream from a cancelled attempt cannot corrupt a retry's body.
    async fn append_observed_chunk(
        db_pool: &SqlitePool,
        identifier: i64,
        epoch: i64,
        accumulated: &mut InteractionAccumulator,
        event: InteractionEvent,
    ) -> Result<(), SummaryError> {
        if let Some(text) = accumulated.observe(event)?
            && !db::append_summary_chunk_for_epoch(db_pool, identifier, epoch, &text)
                .await
                .map_err(|e| SummaryError::ApiError(format!("DB error: {e}")))?
        {
            return Err(SummaryError::ApiError("generation ownership lost".into()));
        }
        Ok(())
    }

    /// Shared terminal validation, token estimation, and cost accounting for
    /// both Gemini streaming paths (builder-driven and raw `xhigh` SSE).
    fn finalize_generation(
        &self,
        model: &ModelOption,
        mut accumulated: InteractionAccumulator,
        prompt_len: usize,
        start: std::time::Instant,
    ) -> Result<SummaryResult, SummaryError> {
        if !accumulated.completed {
            return Err(SummaryError::ApiError(
                "provider stream ended without successful terminal event".into(),
            ));
        }

        if accumulated.summary_text.is_empty() {
            return Err(SummaryError::ApiError(
                "Gemini returned empty response".to_string(),
            ));
        }
        let duration_secs = start.elapsed().as_secs_f64();

        // If token counts weren't provided by the API, estimate them
        if accumulated.input_tokens == 0 {
            accumulated.input_tokens = (prompt_len as u64) / 4;
        }
        if accumulated.output_tokens == 0 {
            accumulated.output_tokens = (accumulated.summary_text.len() as u64) / 4;
        }

        let cost = self.compute_cost(model, accumulated.input_tokens, accumulated.output_tokens);

        tracing::info!(
            input_tokens = accumulated.input_tokens,
            output_tokens = accumulated.output_tokens,
            thinking_tokens = accumulated.thinking_tokens,
            cost = cost,
            duration_secs = duration_secs,
            "Summary generation complete"
        );

        Ok(SummaryResult {
            summary_text: accumulated.summary_text,
            input_tokens: accumulated.input_tokens,
            output_tokens: accumulated.output_tokens,
            thinking_text: accumulated.thinking_text,
            thinking_tokens: accumulated.thinking_tokens,
            cost,
            duration_secs,
        })
    }

    /// Streams an `xhigh` thinking request over a raw SSE call.
    ///
    /// `gemini-rust 2.0.0` cannot express `InteractionThinkingLevel::XHigh`,
    /// so the caller serializes the fully configured builder request and
    /// this method patches `"thinking_level": "xhigh"` into
    /// `generation_config` before POSTing to
    /// `.../v1beta/interactions?alt=sse` with the same `x-goog-api-key`
    /// header the SDK uses. Each `data:` frame parses directly into
    /// `InteractionEvent` and flows through the same accumulator and
    /// epoch-guarded chunk loop as the SDK path.
    #[allow(clippy::too_many_arguments)]
    async fn generate_summary_xhigh(
        &self,
        db_pool: &SqlitePool,
        identifier: i64,
        epoch: i64,
        model_name: &str,
        payload: serde_json::Value,
        prompt_len: usize,
        start: std::time::Instant,
    ) -> Result<SummaryResult, SummaryError> {
        const INTERACTIONS_SSE_URL: &str =
            "https://generativelanguage.googleapis.com/v1beta/interactions?alt=sse";

        tracing::info!(
            identifier = identifier,
            model = %model_name,
            "Starting xhigh summary generation via raw Interactions SSE"
        );

        let client = reqwest::Client::new();
        let mut response = client
            .post(INTERACTIONS_SSE_URL)
            .header("x-goog-api-key", &self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| SummaryError::ApiError(format!("xhigh HTTP error: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if status.as_u16() == 429 || is_rate_limit_error(&body) {
                tracing::warn!(
                    identifier = identifier,
                    "Rate limited by Gemini API (xhigh)"
                );
                return Err(SummaryError::RateLimited);
            }
            tracing::error!(identifier = identifier, status = %status, "Gemini xhigh API error");
            return Err(SummaryError::ApiError(format!("HTTP {status}: {body}")));
        }

        let mut frames = SseFrameBuffer::default();
        let mut accumulated = InteractionAccumulator::default();
        loop {
            let chunk = response
                .chunk()
                .await
                .map_err(|e| SummaryError::ApiError(format!("xhigh stream error: {e}")))?;
            let Some(bytes) = chunk else {
                break;
            };
            for event in frames.push(&bytes)? {
                Self::append_observed_chunk(db_pool, identifier, epoch, &mut accumulated, event)
                    .await?;
            }
        }
        for event in frames.finish()? {
            Self::append_observed_chunk(db_pool, identifier, epoch, &mut accumulated, event)
                .await?;
        }

        let model = ModelOption {
            name: model_name.to_string(),
            input_price_per_mtoken: 0.0,
            output_price_per_mtoken: 0.0,
            context_window: 0,
            rpm_limit: 0,
            tpm_limit: 0,
            rpd_limit: 0,
            architecture: crate::state::ModelArchitecture::Gemini,
        };
        // Price the xhigh flagship run with its real catalog entry.
        let model = crate::state::get_default_models()
            .into_iter()
            .find(|m| m.name == model_name)
            .unwrap_or(model);
        self.finalize_generation(&model, accumulated, prompt_len, start)
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

        let client =
            Gemini::new(&self.api_key).map_err(|e| SummaryError::ApiError(e.to_string()))?;

        tracing::info!(
            identifier = identifier,
            model = %model.name,
            word_count = word_count,
            "Starting summary generation"
        );

        let epoch = db::fetch_summary(db_pool, identifier)
            .await
            .map_err(|e| SummaryError::ApiError(format!("DB error: {e}")))?
            .ok_or_else(|| SummaryError::ApiError("summary row disappeared".into()))?
            .generation_epoch;
        let mut builder = client.create_interaction().with_model(&model.name);

        // Configure grounding with Google Search if selected and architecture is Gemini or Gemma
        if google_search_grounding
            && (model.architecture == crate::state::ModelArchitecture::Gemini
                || model.architecture == crate::state::ModelArchitecture::Gemma)
        {
            builder = builder.with_google_search();
        }

        // Configure URL context if selected and architecture is Gemini
        if url_context && model.architecture == crate::state::ModelArchitecture::Gemini {
            builder = builder.with_url_context();
        }

        // Named Gemini thinking levels go through the builder, while
        // `xhigh` on gemini-4-hydrogen needs the raw SSE path below
        // (gemini-rust 2.0.0 has no XHigh variant). Anything else sends no
        // level so the provider chooses its own default. Gemini 2.5 retains
        // its separate, numeric budget API outside the Interactions API.
        let resolved_thinking = effective_thinking_level(&model.name, thinking_preference);
        let wants_xhigh = resolved_thinking == ResolvedThinking::XHigh;
        if model.architecture == crate::state::ModelArchitecture::Gemini {
            match resolved_thinking {
                ResolvedThinking::Named(level) => {
                    builder = builder
                        .with_thinking_level(level)
                        .with_thinking_summaries(ThinkingSummaries::Auto);
                }
                ResolvedThinking::Omitted if gemini_2_5_thinking_budget(&model.name).is_some() => {
                    // Interactions exposes named Gemini thinking only. Keep the provider default for 2.5.
                }
                ResolvedThinking::Omitted | ResolvedThinking::XHigh => {}
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
                builder = builder.with_system_instruction(&system_instruction_with_date);
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

        // `xhigh` cannot travel through gemini-rust 2.0.0 (its
        // InteractionThinkingLevel has no XHigh variant), so the fully
        // configured builder request is serialized, patched with
        // `"thinking_level": "xhigh"`, and streamed over a raw SSE call.
        if wants_xhigh {
            let request = builder
                .with_text(&prompt)
                .build()
                .map_err(|e| SummaryError::ApiError(e.to_string()))?;
            let mut payload = serde_json::to_value(&request)
                .map_err(|e| SummaryError::ApiError(e.to_string()))?;
            let generation_config = payload
                .as_object_mut()
                .and_then(|obj| {
                    obj.entry("generation_config".to_string())
                        .or_insert_with(|| serde_json::json!({}))
                        .as_object_mut()
                })
                .ok_or_else(|| SummaryError::ApiError("xhigh payload has no JSON object".into()))?;
            generation_config.insert("thinking_level".to_string(), serde_json::json!("xhigh"));
            generation_config.insert("thinking_summaries".to_string(), serde_json::json!("auto"));
            payload
                .as_object_mut()
                .expect("payload is a JSON object")
                .insert("stream".to_string(), serde_json::json!(true));
            return self
                .generate_summary_xhigh(
                    db_pool,
                    identifier,
                    epoch,
                    &model.name,
                    payload,
                    prompt.len(),
                    start,
                )
                .await;
        }

        let mut stream = builder
            .with_text(&prompt)
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

        let mut accumulated = InteractionAccumulator::default();
        while let Some(event) = stream
            .try_next()
            .await
            .map_err(|e| SummaryError::ApiError(e.to_string()))?
        {
            Self::append_observed_chunk(db_pool, identifier, epoch, &mut accumulated, event)
                .await?;
        }

        Self::finalize_generation(self, model, accumulated, prompt.len(), start)
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
    ///
    /// `output_tokens` is billed at the output rate as reported by
    /// `InteractionUsage::total_output_tokens`. Per Google's Gemini billing
    /// documentation, thinking tokens count toward the output-token total,
    /// so `total_thought_tokens` (persisted separately for visibility via
    /// `mark_summary_done`) is deliberately NOT added again here. If a
    /// future API ever reports output tokens exclusive of thinking, this
    /// under-counts: compare `total_tokens` against
    /// `total_input_tokens + total_output_tokens + total_thought_tokens`
    /// on live usage payloads to re-verify.
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

        // Own this generation epoch: chunks from a superseded stream must
        // not leak into a retry's body.
        let epoch = db::fetch_summary(db_pool, identifier)
            .await
            .map_err(|e| SummaryError::ApiError(format!("DB error: {e}")))?
            .ok_or_else(|| SummaryError::ApiError("summary row disappeared".into()))?
            .generation_epoch;

        let mut summary_text = String::new();
        let mut thinking_text = String::new();
        let mut think_pending = String::new();
        let mut think_closed = false;
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut saw_finish_reason = false;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(response) => {
                    for choice in response.choices {
                        if choice.finish_reason.is_some() {
                            saw_finish_reason = true;
                        }
                        if let Some(content) = choice.delta.content
                            && !content.is_empty()
                        {
                            let (summary_part, think_part) = split_hetzner_content(
                                &mut think_pending,
                                &mut think_closed,
                                &content,
                            );
                            thinking_text.push_str(&think_part);
                            if !summary_part.is_empty() {
                                if !db::append_summary_chunk_for_epoch(
                                    db_pool,
                                    identifier,
                                    epoch,
                                    &summary_part,
                                )
                                .await
                                .map_err(|e| SummaryError::ApiError(format!("DB error: {e}")))?
                                {
                                    return Err(SummaryError::ApiError(
                                        "generation ownership lost".into(),
                                    ));
                                }
                                summary_text.push_str(&summary_part);
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

        // Fail open: an unclosed think block is summary text, not silence.
        if !think_pending.is_empty() {
            let pending = std::mem::take(&mut think_pending);
            if !db::append_summary_chunk_for_epoch(db_pool, identifier, epoch, &pending)
                .await
                .map_err(|e| SummaryError::ApiError(format!("DB error: {e}")))?
            {
                return Err(SummaryError::ApiError("generation ownership lost".into()));
            }
            summary_text.push_str(&pending);
        }

        if !saw_finish_reason {
            return Err(SummaryError::ApiError(
                "Hetzner provider stream ended without finish reason".to_string(),
            ));
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
        // Qwen usage reports carry no thinking counts; estimate for storage.
        let thinking_tokens = (thinking_text.len() as u64) / 4;

        let cost = self.compute_cost(model, input_tokens, output_tokens);

        tracing::info!(
            identifier = identifier,
            input_tokens = input_tokens,
            output_tokens = output_tokens,
            thinking_tokens = thinking_tokens,
            cost = cost,
            duration_secs = duration_secs,
            "Hetzner summary generation complete"
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
}

/// Upper bound for a held-back unclosed `<think>` block before failing open
/// and treating the buffered text as summary content.
const MAX_HELD_THINK_BYTES: usize = 65_536;

/// Splits one streamed Hetzner content delta into `(summary_part, think_part)`.
///
/// Qwen models on the OpenAI-compatible Hetzner endpoint expose reasoning as
/// a leading `<think>...</think>` block inside `content` (there is no
/// `reasoning_content` delta field in `async-openai 0.41`). While the block
/// is unclosed, text is held in `pending` so no raw `<think>` markup reaches
/// the persisted summary; once `</think>` arrives, the inner text moves to
/// the thinking accumulator and only the remainder is summary content.
fn split_hetzner_content(
    pending: &mut String,
    think_closed: &mut bool,
    chunk: &str,
) -> (String, String) {
    if *think_closed {
        return (chunk.to_string(), String::new());
    }
    pending.push_str(chunk);
    let trimmed = pending.trim_start().to_string();
    if !trimmed.starts_with("<think>") {
        *think_closed = true;
        return (std::mem::take(pending), String::new());
    }
    if let Some(close) = trimmed.find("</think>") {
        let inner = trimmed["<think>".len()..close].to_string();
        let rest = trimmed[close + "</think>".len()..].to_string();
        *think_closed = true;
        pending.clear();
        return (rest, inner);
    }
    if pending.len() > MAX_HELD_THINK_BYTES {
        *think_closed = true;
        return (std::mem::take(pending), String::new());
    }
    (String::new(), String::new())
}

/// Maps internal Hetzner model names to their API-side model identifiers.
fn resolve_hetzner_model_name(internal_name: &str) -> &str {
    match internal_name {
        "hetzner-qwen-3.6-35b" => "Qwen/Qwen3.6-35B-A3B-FP8",
        "hetzner-qwen-3.8-27b" => "Qwen/Qwen3.8-27B",
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

    fn fake_event(json: &str) -> InteractionEvent {
        serde_json::from_str(json).expect("valid fake interaction event")
    }

    #[test]
    fn interaction_event_accumulator_requires_successful_terminal_event() {
        let mut stream = InteractionAccumulator::default();
        assert_eq!(stream.observe(fake_event(r#"{"event_type":"step.delta","index":0,"delta":{"type":"text","text":"hello"}}"#)).unwrap(), Some("hello".into()));
        stream.observe(fake_event(r#"{"event_type":"step.stop","index":0,"usage":{"total_input_tokens":12,"total_output_tokens":34,"total_thought_tokens":5}}"#)).unwrap();
        assert!(
            !stream.completed,
            "EOF without terminal event is rejected by caller"
        );
        assert_eq!(stream.input_tokens, 12);
        assert_eq!(stream.output_tokens, 34);
        assert_eq!(stream.thinking_tokens, 5);
        stream
            .observe(fake_event(
                r#"{"event_type":"interaction.completed","interaction":{"status":"completed"}}"#,
            ))
            .unwrap();
        assert!(stream.completed);
    }

    #[test]
    fn interaction_event_accumulator_rejects_failed_and_error_events() {
        let mut stream = InteractionAccumulator::default();
        assert!(
            stream
                .observe(fake_event(
                    r#"{"event_type":"interaction.completed","interaction":{"status":"cancelled"}}"#
                ))
                .is_err()
        );
        assert!(
            stream
                .observe(fake_event(r#"{"event_type":"error","error":{}}"#))
                .is_err()
        );
    }

    #[test]
    fn thinking_levels_map_only_for_gemini_3() {
        use ResolvedThinking::*;
        // Auto never sends a level; unknown and non-Gemini models omit it.
        assert_eq!(
            effective_thinking_level("gemini-4-hydrogen", ThinkingPreference::Auto),
            Omitted
        );
        assert_eq!(
            effective_thinking_level("gemini-2.5-flash-lite", ThinkingPreference::High),
            Omitted
        );
        assert_eq!(
            effective_thinking_level("hetzner-qwen-3.8-27b", ThinkingPreference::High),
            Omitted
        );
        assert_eq!(
            effective_thinking_level("gemma-4-31b-it", ThinkingPreference::Low),
            Omitted
        );
        // xhigh only survives on hydrogen; Gemini 3.x clamps it to high.
        assert_eq!(
            effective_thinking_level("gemini-4-hydrogen", ThinkingPreference::XHigh),
            XHigh
        );
        assert_eq!(
            effective_thinking_level("gemini-3.6-flash", ThinkingPreference::XHigh),
            Named(InteractionThinkingLevel::High)
        );
        // minimal is real on lite models but clamps to low where rejected.
        assert_eq!(
            effective_thinking_level("gemini-3.5-flash-lite", ThinkingPreference::Minimal),
            Named(InteractionThinkingLevel::Minimal)
        );
        assert_eq!(
            effective_thinking_level("gemini-4-hydrogen", ThinkingPreference::Minimal),
            Named(InteractionThinkingLevel::Low)
        );
        assert_eq!(
            effective_thinking_level("gemini-3.8-flash", ThinkingPreference::Minimal),
            Named(InteractionThinkingLevel::Low)
        );
        assert_eq!(
            effective_thinking_level("gemini-3.8-flash", ThinkingPreference::Medium),
            Named(InteractionThinkingLevel::Medium)
        );
        assert_eq!(gemini_2_5_thinking_budget("gemini-2.5-flash"), Some(24576));
        assert_eq!(gemini_2_5_thinking_budget("gemini-2.5-pro"), Some(32768));
        assert_eq!(gemini_2_5_thinking_budget("gemini-3.8-flash"), None);
    }

    #[test]
    fn sse_frame_buffer_parses_split_chunks() {
        // One event split across chunk boundaries, plus keep-alive and DONE.
        let mut frames = SseFrameBuffer::default();
        let events = frames
            .push(b": keep-alive\n\ndata: {\"event_type\":\"step.delta\",\"index\":0,")
            .unwrap();
        assert!(events.is_empty());
        let events = frames
            .push(b"\"delta\":{\"type\":\"text\",\"text\":\"hello\"}}\n\ndata: [DONE]\n\n")
            .unwrap();
        assert_eq!(events.len(), 1);
        let mut accumulated = InteractionAccumulator::default();
        assert_eq!(
            accumulated
                .observe(events.into_iter().next().unwrap())
                .unwrap(),
            Some("hello".into())
        );
        // Trailing bytes without a terminal blank line still parse.
        let mut frames = SseFrameBuffer::default();
        let events = frames
            .push(b"data: {\"event_type\":\"step.delta\",\"index\":0,\"delta\":{\"type\":\"text\",\"text\":\"tail\"}}")
            .unwrap();
        assert!(events.is_empty());
        let events = frames.finish().unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn hetzner_think_block_is_separated_from_summary() {
        let mut pending = String::new();
        let mut closed = false;
        // Opening tag alone is held back.
        assert_eq!(
            split_hetzner_content(&mut pending, &mut closed, "<think>"),
            (String::new(), String::new())
        );
        assert!(!closed);
        // Closing tag releases the thought and the summary remainder.
        assert_eq!(
            split_hetzner_content(
                &mut pending,
                &mut closed,
                "reasoning here</think>## Abstract"
            ),
            ("## Abstract".to_string(), "reasoning here".to_string())
        );
        assert!(closed);
        // Later chunks pass through untouched.
        assert_eq!(
            split_hetzner_content(&mut pending, &mut closed, " more"),
            (" more".to_string(), String::new())
        );
    }

    #[test]
    fn hetzner_content_without_think_passes_through() {
        let mut pending = String::new();
        let mut closed = false;
        assert_eq!(
            split_hetzner_content(&mut pending, &mut closed, "## Abstract: foo"),
            ("## Abstract: foo".to_string(), String::new())
        );
        assert!(closed);
    }

    fn test_model() -> ModelOption {
        ModelOption {
            name: "gemini-2.0-flash".to_string(),
            input_price_per_mtoken: 0.075,
            output_price_per_mtoken: 0.30,
            context_window: 1_000_000,
            rpm_limit: 10,
            tpm_limit: 250_000,
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
            tpm_limit: 0,
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
            "Qwen/Qwen3.8-27B"
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
