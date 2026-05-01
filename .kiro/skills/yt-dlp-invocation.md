---
name: yt-dlp-invocation
description: Use when invoking yt-dlp via uvx, handling subtitle download failures, working with Firefox cookie authentication, or debugging format resolution errors.
inclusion: manual
---

# yt-dlp Invocation Patterns

## Overview

rs-summarizer uses yt-dlp (via `uvx`) to download YouTube video subtitles. All invocations use Firefox cookies for authentication and specific flags to avoid format resolution errors.

## Command Pattern

yt-dlp is never installed globally. It's run via `uvx yt-dlp` which downloads and executes it through the `uv` Python package manager.

### Listing Subtitles

```rust
Command::new("uvx")
    .args(["yt-dlp", "--cookies-from-browser", "firefox", "--list-subs", url])
```

### Downloading Subtitles

```rust
Command::new("uvx")
    .args([
        "yt-dlp",
        "--cookies-from-browser", "firefox",
        "--write-sub",
        "--write-auto-sub",
        "--sub-lang", lang,
        "--sub-format", "vtt",
        "--skip-download",
        "--format", "mhtml",
        "-o", output_template,
        url,
    ])
```

## Critical Flags

| Flag | Purpose |
|------|---------|
| `--cookies-from-browser firefox` | Authenticates with YouTube to avoid bot detection / 429 errors |
| `--write-sub` | Downloads manually-uploaded subtitles |
| `--write-auto-sub` | Downloads auto-generated captions (most videos only have these) |
| `--format "mhtml"` | Selects storyboard format to avoid "Requested format not available" errors when `--skip-download` can't resolve a video format |
| `--skip-download` | Don't download the actual video file |
| `--sub-format vtt` | Download subtitles in WebVTT format |
| `-o template` | Output filename template (yt-dlp appends `.lang.vtt`) |

## Why `--format "mhtml"`?

When using `--cookies-from-browser`, yt-dlp sometimes gets a different format list from YouTube that doesn't include standard video formats. The `--skip-download` flag still tries to resolve a format, and fails with "Requested format is not available". Using `--format "mhtml"` (storyboard) bypasses this because storyboards are always available.

## Output File Naming

yt-dlp creates files like: `{output_template}.{lang}.vtt`

Example: `-o "/dev/shm/transcript_42"` → `/dev/shm/transcript_42.en.vtt`

The code finds VTT files by scanning the temp directory for files matching the prefix and `.vtt` extension.

## Error Handling

The `list_subtitles()` function checks for specific error patterns:

- **429 / Too Many Requests** → `TranscriptError::YtDlpFailed` with rate limit message
- **"Sign in to confirm" / bot detection** → `TranscriptError::YtDlpFailed` with auth message
- **Non-zero exit + no subtitle info** → `TranscriptError::YtDlpFailed` with stderr
- **Non-zero exit + has subtitle info** → Continue parsing (yt-dlp sometimes exits non-zero but still outputs useful data)

## Language Selection Priority

The `pick_best_language()` function selects from available languages:

1. `-orig` languages matching preferred base order (en, de, fr, es, pt, it, nl, ja, ko, zh)
2. Any `-orig` language (sorted alphabetically)
3. Non-orig languages matching preferred base order
4. Any language with `en` prefix (e.g. `en-US`)
5. First language sorted alphabetically

## Temp File Cleanup

VTT files are stored in `/dev/shm` (tmpfs) and cleaned up via a `TempFileGuard` RAII struct that removes files on drop, ensuring cleanup even on error paths.

## Relevant Files

- `src/services/transcript.rs` — All yt-dlp invocation logic
- `tests/integration_transcript.rs` — Integration tests for download
