---
name: yt-dlp-invocation
description: Use when invoking yt-dlp via uvx, handling subtitle download failures, configuring PO Token provider (bgutil), working with cookie authentication, or debugging format resolution errors.
---

# yt-dlp Invocation Patterns

## Overview

rs-summarizer uses `yt-dlp` (via `uvx`) to download YouTube video subtitles. Invocations load the `bgutil-ytdlp-pot-provider` plugin and target the `mweb` player client to bypass YouTube's bot detection and Proof-of-Origin (PO) token requirements. JavaScript challenges are solved via Deno (`[jsc:deno]`).

## Command Pattern

`yt-dlp` is run via `uvx --with bgutil-ytdlp-pot-provider yt-dlp` without requiring global Python package installations.

### Listing Subtitles

```rust
let mut args = base_uvx_args();
args.extend(cookie_args());
args.extend(extractor_args());
args.push("--list-subs".to_string());
args.push(url.to_string());
```

Command executed:
```bash
uvx --with bgutil-ytdlp-pot-provider yt-dlp \
    --extractor-args "youtube:player_client=mweb" \
    --extractor-args "youtubepot-bgutilhttp:base_url=http://host.docker.internal:4416" \
    --list-subs "<URL>"
```

### Downloading Subtitles

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

## Critical Flags & Arguments

| Flag / Option | Purpose |
|---------------|---------|
| `--with bgutil-ytdlp-pot-provider` | Ephemerally loads the PO token provider plugin for yt-dlp |
| `--extractor-args "youtube:player_client=mweb"` | Directs yt-dlp to YouTube mobile web client where PO tokens are resolved |
| `--extractor-args "youtubepot-bgutilhttp:base_url=..."` | Configures the HTTP endpoint of the `bgutil-ytdlp-pot-provider` container |
| `--write-sub` | Downloads manually-uploaded subtitles |
| `--write-auto-sub` | Downloads auto-generated captions (most videos only have these) |
| `--format "mhtml"` | Selects storyboard format to avoid "Requested format not available" errors |
| `--skip-download` | Don't download the actual video file |
| `--sub-format vtt` | Download subtitles in WebVTT format |
| `-o template` | Output filename template (yt-dlp appends `.lang.vtt`) |

## Environment Variables

- `POT_PROVIDER_URL` (or `BGUTIL_POT_PROVIDER_URL`, `YTDLP_POT_PROVIDER_URL`): Base URL for the PO token server (e.g. `http://host.docker.internal:4416` or `http://127.0.0.1:4416`). Auto-detects `host.docker.internal:4416` in container environments.
- `YTDLP_PLAYER_CLIENT`: Override player client (default: `mweb`).
- `YTDLP_EXTRACTOR_ARGS`: Additional custom extractor arguments.
- `DISABLE_POT_PROVIDER`: Set to `1` or `true` to omit `--with bgutil-ytdlp-pot-provider`.
- `YTDLP_COOKIES` / `COOKIES_FILE` / `cookies.txt`: Path to cookies text file.
- `YTDLP_COOKIES_FROM_BROWSER`: Browser name to extract cookies from (e.g. `firefox`). Automatically detected if Firefox profile directories exist in `$HOME`.

## System Dependencies

- **`deno`**: Installed in PATH (`/usr/local/bin/deno` and persistent `/root/.cargo/bin/deno`) for solving YouTube client JavaScript challenges (`[jsc:deno]`).
- **`ytdlp-pot-provider`**: Docker container (`brainicism/bgutil-ytdlp-pot-provider:latest`) running on port `4416` on the host.

## Output File Naming

yt-dlp creates files like: `{output_template}.{lang}.vtt`
Example: `-o "/dev/shm/transcript_42"` → `/dev/shm/transcript_42.en.vtt`

## Error Handling

The `list_subtitles()` and `download_subtitles()` functions check for specific error patterns:
- **Port 4416 / POT Provider connection errors** → Diagnostic error prompting to ensure `ytdlp-pot-provider` Docker container is running and `POT_PROVIDER_URL` is set.
- **"The page needs to be reloaded"** → Prompts refreshing YouTube session/cookies.
- **429 / Too Many Requests** → Rate limit notification.
- **"Sign in to confirm" / bot detection** → Prompts checking PO provider status and cookies.
- **Non-zero exit + has subtitle info** → Continues parsing if subtitle data is present.

## Relevant Files

- `src/services/transcript.rs` — All yt-dlp invocation logic
- `tests/integration_transcript.rs` — Integration tests for download
- `scripts/install_po_provider.sh` — Setup script for PO Token Provider container
