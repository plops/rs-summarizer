# Implementation Plan: YouTube PO Token Provider & yt-dlp Subtitle Acquisition

**Client**: Wol Pumba (`wolpumba@gmail.com`)  
**Project**: `plops/rs-summarizer`  
**Plan Directory**: [plan/20260818_01_po_provider/](file:///workspace/src/rs-summarizer/plan/20260818_01_po_provider/)  

---

## 1. Overview & Motivation

YouTube has drastically increased bot detection and anti-scraping measures (such as requiring signed Proof-of-Origin / PO tokens and JavaScript challenges). As a result, standard `yt-dlp` invocations frequently fail or return empty caption streams.

To resolve this, a dedicated PO token provider container (`brainicism/bgutil-ytdlp-pot-provider`) runs on the host machine (started via [scripts/install_po_provider.sh](file:///workspace/src/rs-summarizer/scripts/install_po_provider.sh)). With the `bgutil-ytdlp-pot-provider` plugin loaded into `yt-dlp` and the player client set to `mweb`, `yt-dlp` can dynamically acquire PO tokens and download subtitles reliably:

```bash
uvx --with bgutil-ytdlp-pot-provider yt-dlp \
  --cookies-from-browser firefox \
  --extractor-args "youtube:player_client=mweb" \
  --list-subs "https://youtube.com/watch?v=..."
```

In addition, `yt-dlp` delegates JavaScript challenge solving to an external JS runtime (`[jsc:deno]`).

This implementation plan outlines how to adapt `rs-summarizer`'s Rust codebase to cleanly, reliably, and maintainably integrate the PO token provider, support cross-host/container network setups, provide robust error diagnostics, and maintain clean test coverage.

---

## 2. Remote Host & Network Topology ("Host is a different computer")

Because the host running the docker container is a **different computer / host environment** from where `rs-summarizer` runs (or is inside a container with separate networking), the PO token server cannot be assumed to be always on `http://127.0.0.1:4416`.

### Environment Variable Configuration:
1. **`POT_PROVIDER_URL`** (with fallback aliases `BGUTIL_POT_PROVIDER_URL`, `YTDLP_POT_PROVIDER_URL`):
   - When set (e.g. `http://192.168.1.100:4416` or `http://host.docker.internal:4416`), `rs-summarizer` passes:
     `--extractor-args "youtubepot-bgutilhttp:base_url=<URL>"`
   - When unset, `bgutil-ytdlp-pot-provider` defaults to `http://127.0.0.1:4416`.
2. **`YTDLP_PLAYER_CLIENT`**:
   - Defaults to `"mweb"`. Can be overridden to `"default"`, `"web"`, `"android"`, etc.
   - Passed as `--extractor-args "youtube:player_client=<client>"`.
3. **`YTDLP_EXTRACTOR_ARGS`**:
   - Allows passing extra custom extractor arguments without code changes.
4. **`DISABLE_POT_PROVIDER`**:
   - If set to `"1"` or `"true"`, omits `--with bgutil-ytdlp-pot-provider` as an emergency fallback/escape hatch.

---

## 3. Suggestions & Additional Requirements Analysis

Answering the user's prompt: *"Habe ich alle Requirements aufgelistet, die wichtig sind, oder kannst du dir vorstellen, dass noch andere Dinge werden sollen?"*

### Recommended Features & Enhancements:

1. **Configurable Provider Base URL (`POT_PROVIDER_URL`)** *(Crucial)*:
   - Necessary because `rs-summarizer` and the POT provider container run on separate network nodes or containers.
2. **Deno Runtime in Container / Dockerfile** *(Crucial)*:
   - Required for `yt-dlp`'s JS challenge solver (`[jsc:deno]`). Deno is installed at `/usr/local/bin/deno`.
3. **Modular Command Builder in Rust** *(Maintainability)*:
   - Refactor `list_subtitles()` and `download_subtitles()` to share a single, unified, testable command-construction helper (`base_uvx_args()`, `extractor_args()`, `cookie_args()`).
4. **Enhanced Diagnostic Error Messages**:
   - When `yt-dlp` returns errors related to bot detection, missing tokens, or unreachable POT provider, provide clear diagnostic messages in `TranscriptError::YtDlpFailed` indicating whether cookies, POT provider URL, or JS engine should be checked.
5. **Escape Hatch (`DISABLE_POT_PROVIDER`)**:
   - Ability to disable the plugin via environment variable for local testing, fallback, or non-YouTube scenarios.
6. **Documentation & Skill Updates**:
   - Update [.kiro/skills/yt-dlp-invocation/SKILL.md](file:///workspace/src/rs-summarizer/.kiro/skills/yt-dlp-invocation/SKILL.md) and [RELEASE_README.md](file:///workspace/src/rs-summarizer/RELEASE_README.md) to document the new environment variables and Deno dependency.

---

## 4. Architecture & Implementation Design

### 4.1 Refactoring `src/services/transcript.rs`

All `yt-dlp` subprocess invocations in `src/services/transcript.rs` will be constructed using unified helper functions:

```rust
/// Builds the base uvx command arguments, including optional pot-provider plugin.
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

/// Builds extractor arguments for player client, POT provider base URL, and custom args.
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

### 4.2 Updating Subtitle Invocations

In `list_subtitles()`:
```rust
let mut args = base_uvx_args();
args.extend(cookie_args());
args.extend(extractor_args());
args.push("--list-subs".to_string());
args.push(url.to_string());
```

In `download_subtitles()`:
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

### 4.3 Error Handling Enhancements

Enhance stderr analysis in `list_subtitles()` and `download_subtitles()` to detect:
- `pot:bgutil` or connection refused errors to port 4416 → suggest checking `POT_PROVIDER_URL` and verifying the container `ytdlp-pot-provider` is running.
- JS challenge errors → suggest verifying Deno is installed.
- YouTube bot detection → suggest refreshing Firefox cookies.

---

## 5. Autonomous AI Agent File Context Map

An AI agent implementing or maintaining this feature should examine these key files:

| File Path | Description |
|:---|:---|
| [src/services/transcript.rs](file:///workspace/src/rs-summarizer/src/services/transcript.rs) | Primary file containing `TranscriptService`, `list_subtitles()`, `download_subtitles()`, `cookie_args()`, language selection, and temp file cleanup. |
| [src/errors.rs](file:///workspace/src/rs-summarizer/src/errors.rs) | Contains `TranscriptError` and `ProcessError` enum definitions. |
| [src/tasks.rs](file:///workspace/src/rs-summarizer/src/tasks.rs) | Background summarization pipeline calling `TranscriptService::download_transcript`. |
| [tests/integration_transcript.rs](file:///workspace/src/rs-summarizer/tests/integration_transcript.rs) | Integration tests for `yt-dlp` subtitle listing and downloading. |
| [scripts/install_po_provider.sh](file:///workspace/src/rs-summarizer/scripts/install_po_provider.sh) | Docker run script for `brainicism/bgutil-ytdlp-pot-provider:latest`. |
| [.kiro/skills/yt-dlp-invocation/SKILL.md](file:///workspace/src/rs-summarizer/.kiro/skills/yt-dlp-invocation/SKILL.md) | Skill documentation for `yt-dlp` invocation flags, cookies, and architecture. |
| [plan/20260818_01_po_provider/deps.md](file:///workspace/src/rs-summarizer/plan/20260818_01_po_provider/deps.md) | Dependencies document recording packages, runtimes, and containers. |

---

## 6. Conventional Commit Guidelines

All commits must strictly adhere to the **Conventional Commits** specification:

### Format:
```text
<type>(<scope>): <short summary in present imperative tense>

<comprehensive body explaining the motivation, architectural decisions, and specific changes>

- <bullet points for individual details if applicable>
```

### Allowed Types:
- `feat`: New feature or capability
- `fix`: Bug fix
- `test`: Adding or modifying tests
- `docs`: Documentation updates
- `refactor`: Code change that neither fixes a bug nor adds a feature
- `chore`: Tooling, dependencies, or environment updates

### Example:
```text
feat(transcript): integrate YouTube PO token provider and mweb player client in yt-dlp

Add bgutil-ytdlp-pot-provider plugin support to yt-dlp invocations via uvx
to bypass YouTube bot detection and caption fetch errors.

Key changes:
- Pass `--with bgutil-ytdlp-pot-provider` to uvx by default
- Add `--extractor-args "youtube:player_client=mweb"` by default
- Support configurable `POT_PROVIDER_URL` for remote host/container setups
- Support `DISABLE_POT_PROVIDER` and `YTDLP_EXTRACTOR_ARGS` environment variables
- Enrich error diagnostics when token generation or HTTP provider fails
- Add unit tests for command line argument building and env overrides
```

---

## 7. Testing Strategy

1. **Unit Tests** in `src/services/transcript.rs`:
   - `test_base_uvx_args_default`: Verifies `--with bgutil-ytdlp-pot-provider yt-dlp` is generated.
   - `test_base_uvx_args_disabled`: Verifies `DISABLE_POT_PROVIDER=1` falls back to `yt-dlp`.
   - `test_extractor_args_default`: Verifies `youtube:player_client=mweb` is included.
   - `test_extractor_args_custom_pot_url`: Verifies `youtubepot-bgutilhttp:base_url=<URL>` is included when `POT_PROVIDER_URL` is set.
   - `test_extractor_args_custom_player_client`: Verifies `YTDLP_PLAYER_CLIENT` overrides player client.
   - `test_extractor_args_extra_args`: Verifies `YTDLP_EXTRACTOR_ARGS` is appended.
2. **Integration Tests** in `tests/integration_transcript.rs`:
   - Update commands to include `--with bgutil-ytdlp-pot-provider` and `--extractor-args "youtube:player_client=mweb"`.
3. **Workspace Validation**:
   - `cargo test` (all unit and existing tests must pass).
   - `cargo clippy` and `cargo fmt`.
