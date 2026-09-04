# Dependency research: abort-safe Gemini generation

This change introduces no new crates.  `cargo search gemini-rust --limit 5` and
`cargo info gemini-rust` on 2026-09-04 confirm that the already pinned
`gemini-rust = 2.0.0` is the newest published release.  The implementation
must keep that exact version unless a later implementation-time check finds a
newer stable release.  If it does, use `cargo upgrade --incompatible allow`,
record the exact version here, and run the complete verification matrix in
`task.md`.

## Direct dependency

### `gemini-rust` 2.0.0

- GitHub: [`flachesis/gemini-rust`](https://github.com/flachesis/gemini-rust)
- Owner / organisation: `flachesis`
- Purpose: Gemini requests, Interactions API, typed streaming lifecycle events,
  Google Search, URL Context, thinking, and typed interaction status/usage.
- DeepWiki queries already made:
  - `What is the current recommended streaming API and how are completion,
    stream interruption, finish reasons, usage, and 503 UNAVAILABLE exposed?`
  - `How should generateContent streaming be migrated to Interactions while
    retaining Google Search, URL Context, system instructions, and thinking?`

The crate README and checked-in source are the authority when DeepWiki differs:
2.0.0 labels `generateContent` as legacy/deprecated and recommends the
Interactions API.  The existing `SummaryService` uses that legacy path and
locally suppresses its deprecation warning.  The new code should use the typed
events and treat `InteractionCompleted` with an acceptable terminal status as
the proof of successful provider completion; EOF alone is insufficient.

### Target usage sketch (must compile against the selected version)

```rust
use futures_util::TryStreamExt;
use gemini_rust::prelude::*;

let mut request = client
    .create_interaction()
    .with_model(&model.name)
    .with_system_instruction(system_instruction)
    .with_text(prompt);

if google_search_grounding { request = request.with_google_search(); }
if url_context { request = request.with_url_context(); }
// Map the persisted preference to InteractionThinkingLevel and
// ThinkingSummaries::Auto only for Gemini 3.x.

let mut stream = request.execute_stream().await?;
let mut terminal = None;
while let Some(event) = stream.try_next().await? {
    match event {
        InteractionEvent::StepDelta { delta: StepDeltaData::Text { text }, .. } => {
            // append idempotently through the generation repository
        }
        InteractionEvent::StepStop { usage: Some(usage), .. } => {
            // retain final provider usage
        }
        InteractionEvent::InteractionCompleted { interaction, .. } => {
            terminal = Some(interaction);
        }
        _ => {}
    }
}
// Reject EOF without a terminal event, a failed/cancelled terminal status, or
// an output that fails the product completeness validator.
```

`async-openai`, `sqlx`, `tokio`, `axum`, Askama, and `futures-util` are already
present and need no new dependency record.  Do not add a retry crate: bounded
backoff, cancellation, and durable state are application policy and should
remain explicit/testable in the repository.
