# Task Breakdown: YouTube PO Token Provider & yt-dlp Subtitle Acquisition

**Project**: `plops/rs-summarizer`  
**Reference**: [plan.md](file:///workspace/src/rs-summarizer/plan/20260818_01_po_provider/plan.md) | [deps.md](file:///workspace/src/rs-summarizer/plan/20260818_01_po_provider/deps.md)  

Each task is self-contained: **implement → test → validate → commit**. Execute serially.

---

## Task 1: Environment & System Runtime Setup (Deno Installation)

**Goal**: Ensure `deno` is installed in the container environment so `yt-dlp` can solve YouTube JavaScript challenges (`[jsc:deno]`).

### Steps
1. Verify `deno` is installed and accessible in PATH:
   ```bash
   which deno && deno --version
   ```
2. If missing, install Deno and place binary in `/usr/local/bin`:
   ```bash
   curl -fsSL https://deno.land/install.sh | sh
   cp /root/.deno/bin/deno /usr/local/bin/deno
   ```
3. Test that `uvx --with bgutil-ytdlp-pot-provider yt-dlp --version` runs cleanly.

### Validate
```bash
deno --version
uvx --with bgutil-ytdlp-pot-provider yt-dlp --version
```

---

## Task 2: Implement Unified `yt-dlp` Argument Builders & Unit Tests in `src/services/transcript.rs`

**Goal**: Add reusable, configurable functions in `src/services/transcript.rs` to construct `uvx` arguments and `--extractor-args`.

### Files to Modify
- [src/services/transcript.rs](file:///workspace/src/rs-summarizer/src/services/transcript.rs)

### Steps
1. Add `base_uvx_args()` in `src/services/transcript.rs`:
   ```rust
   /// Returns the uvx and plugin arguments to run yt-dlp.
   /// Uses bgutil-ytdlp-pot-provider by default unless DISABLE_POT_PROVIDER is set to true/1.
   fn base_uvx_args() -> Vec<String> {
       let disable_pot = std::env::var("DISABLE_POT_PROVIDER")
           .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
           .unwrap_or(false);

       if disable_pot {
           vec!["yt-dlp".to_string()]
       } else {
           vec![
               "--with".to_string(),
               "bgutil-ytdlp-pot-provider".to_string(),
               "yt-dlp".to_string(),
           ]
       }
   }
   ```
2. Add `extractor_args()` in `src/services/transcript.rs`:
   ```rust
   /// Returns the extractor arguments for YouTube player client and PO token provider base URL.
   fn extractor_args() -> Vec<String> {
       let mut args = Vec::new();

       // Player client: default to mweb
       let player_client = std::env::var("YTDLP_PLAYER_CLIENT")
           .unwrap_or_else(|_| "mweb".to_string());
       let player_client = player_client.trim();
       if !player_client.is_empty() {
           args.extend([
               "--extractor-args".to_string(),
               format!("youtube:player_client={}", player_client),
           ]);
       }

       // POT provider base URL: check POT_PROVIDER_URL, BGUTIL_POT_PROVIDER_URL, or YTDLP_POT_PROVIDER_URL
       let pot_url = std::env::var("POT_PROVIDER_URL")
           .or_else(|_| std::env::var("BGUTIL_POT_PROVIDER_URL"))
           .or_else(|_| std::env::var("YTDLP_POT_PROVIDER_URL"))
           .ok();
       if let Some(url) = pot_url {
           let trimmed = url.trim();
           if !trimmed.is_empty() {
               args.extend([
                   "--extractor-args".to_string(),
                   format!("youtubepot-bgutilhttp:base_url={}", trimmed),
               ]);
           }
       }

       // Custom extractor args passthrough
       if let Ok(extra) = std::env::var("YTDLP_EXTRACTOR_ARGS") {
           let trimmed = extra.trim();
           if !trimmed.is_empty() {
               args.extend(["--extractor-args".to_string(), trimmed.to_string()]);
           }
       }

       args
   }
   ```
3. Add unit tests for `base_uvx_args()` and `extractor_args()` in `src/services/transcript.rs`:
   - `test_base_uvx_args_default`
   - `test_extractor_args_default`
   - `test_extractor_args_custom_url`

### Validate
```bash
cargo test -- service::transcript::tests
```

### Commit
```text
feat(transcript): add argument builders for yt-dlp pot-provider and extractor-args

Introduce base_uvx_args() and extractor_args() helpers in TranscriptService
to support bgutil-ytdlp-pot-provider plugin and configurable YouTube player
clients and remote POT provider base URLs.

Includes comprehensive unit tests for argument construction and defaults.
```

---

## Task 3: Update `list_subtitles()` & `download_subtitles()` and Diagnostics

**Goal**: Connect the argument builders into `list_subtitles()` and `download_subtitles()`, and enhance error diagnostics.

### Files to Modify
- [src/services/transcript.rs](file:///workspace/src/rs-summarizer/src/services/transcript.rs)

### Steps
1. Refactor `list_subtitles()`:
   ```rust
   let mut args = base_uvx_args();
   args.extend(cookie_args());
   args.extend(extractor_args());
   args.push("--list-subs".to_string());
   args.push(url.to_string());
   ```
2. Refactor `download_subtitles()`:
   ```rust
   let mut args = base_uvx_args();
   args.extend(cookie_args());
   args.extend(extractor_args());
   args.extend([
       "--write-sub".to_string(),
       "--write-auto-sub".to_string(),
       "--sub-lang".to_string(),
       lang.to_string(),
       "--sub-format".to_string(),
       "vtt".to_string(),
       "--skip-download".to_string(),
       "--format".to_string(),
       "mhtml".to_string(),
       "-o".to_string(),
       output_template.to_string(),
       url.to_string(),
   ]);
   ```
3. Enhance error matching in both functions for POT provider / bot connection issues:
   ```rust
   if combined.contains("Connection refused") && combined.contains("4416") {
       return Err(TranscriptError::YtDlpFailed(format!(
           "Failed to connect to PO Token Provider on port 4416. Ensure docker container 'ytdlp-pot-provider' is running and POT_PROVIDER_URL is configured correctly if on another host. (cmd: `{}`)",
           cmd_str
       )));
   }
   ```

### Validate
```bash
cargo test
```

### Commit
```text
feat(transcript): apply pot-provider and mweb extractor args to yt-dlp invocations

Wire up base_uvx_args() and extractor_args() in list_subtitles() and
download_subtitles(). Add enhanced diagnostic error messages when
PO Token Provider connection or YouTube authentication fails.
```

---

## Task 4: Update Integration Tests and Skill Documentation

**Goal**: Keep integration tests in sync with the new command pattern and update skill documentation.

### Files to Modify
- [tests/integration_transcript.rs](file:///workspace/src/rs-summarizer/tests/integration_transcript.rs)
- [.kiro/skills/yt-dlp-invocation/SKILL.md](file:///workspace/src/rs-summarizer/.kiro/skills/yt-dlp-invocation/SKILL.md)

### Steps
1. Update `tests/integration_transcript.rs` test commands to include `--with bgutil-ytdlp-pot-provider` and `--extractor-args "youtube:player_client=mweb"`.
2. Update `.kiro/skills/yt-dlp-invocation/SKILL.md` with:
   - Plugin usage: `uvx --with bgutil-ytdlp-pot-provider yt-dlp`
   - `youtube:player_client=mweb`
   - Environment variables: `POT_PROVIDER_URL`, `YTDLP_PLAYER_CLIENT`, `DISABLE_POT_PROVIDER`, `YTDLP_EXTRACTOR_ARGS`.
   - Deno runtime requirement for JS challenges.

### Validate
```bash
cargo test
```

### Commit
```text
docs(transcript): update yt-dlp integration tests and invocation skill documentation

Update integration test commands with bgutil-ytdlp-pot-provider and mweb
extractor args. Document POT provider configuration, Deno JS engine dependency,
and environment variables in yt-dlp-invocation SKILL.md.
```

---

## Task 5: Complete Verification & Code Cleanup

**Goal**: Run formatting, linting, and full test suite to guarantee high code quality.

### Steps
1. Run `cargo fmt --check` (or `cargo fmt` to format).
2. Run `cargo clippy --all-targets -- -D warnings`.
3. Run `cargo test`.

### Validate
Ensure 0 warnings and all tests passing.

---

## Task 6: Final Walkthrough Document

**Goal**: Record what was implemented, tests run, learnings, and docker requirements in `plan/20260818_01_po_provider/walkthrough.md`.
