---
name: environment-setup
description: Use when setting up the development environment, installing prerequisites, configuring API keys, or troubleshooting missing dependencies like uvx, geckodriver, or Firefox cookies.
---

# Environment Setup

## Overview

rs-summarizer requires several external tools beyond the Rust toolchain. This skill documents the full prerequisites checklist and how to verify each one.

## Prerequisites Checklist

| Tool | Purpose | Verify Command |
|------|---------|----------------|
| Rust (stable) | Build the project | `rustc --version` |
| `uv` / `uvx` | Run yt-dlp without global install | `uvx --version` |
| Firefox | Cookie source for YouTube auth | `firefox --version` |
| `GEMINI_API_KEY` | Google Gemini API access | `echo $GEMINI_API_KEY` |
| geckodriver | Browser integration tests only | `geckodriver --version` |

## Installing Prerequisites

### Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### uv (Python package runner)

```bash
# Via pip
pip install uv

# Via Homebrew (macOS/Linux)
brew install uv

# Or see: https://docs.astral.sh/uv/getting-started/installation/
```

Once installed, `uvx yt-dlp` will download and run yt-dlp automatically — no separate yt-dlp install needed.

### Gemini API Key

```bash
# Store key in a file
echo "your-key-here" > ~/api_key.txt

# Export for the session
export GEMINI_API_KEY=$(cat ~/api_key.txt)
```

Get a key from: https://aistudio.google.com/apikey

### geckodriver (for browser tests only)

```bash
# Download from: https://github.com/mozilla/geckodriver/releases
# Place in ~/bin/ or anywhere in PATH
chmod +x geckodriver
```

## Firefox Cookie Authentication

yt-dlp uses `--cookies-from-browser firefox` to authenticate with YouTube. This requires:

1. Firefox installed and has been used to visit YouTube
2. The user is logged into YouTube in Firefox
3. Firefox is **not running** when yt-dlp reads cookies (it locks the cookie DB)

If you get 429 errors or "Sign in to confirm" messages, ensure Firefox has fresh YouTube cookies.

## Database Setup

The SQLite database is created automatically on first run at `data/summaries.db`. The `data/` directory must exist:

```bash
mkdir -p data
```

Migrations run automatically via `sqlx::migrate!()`.

## Running the Server

```bash
export GEMINI_API_KEY=$(cat ~/api_key.txt)
cargo run --release
# Server starts on http://localhost:5001
```

## Verifying the Setup

```bash
# 1. Check Rust builds
cargo check

# 2. Check yt-dlp works
uvx yt-dlp --version

# 3. Check unit tests pass
cargo test

# 4. Check API key works (optional)
curl -s "https://generativelanguage.googleapis.com/v1beta/models?key=$GEMINI_API_KEY" | head -c 200
```

## Common Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| `uvx: command not found` | uv not installed | Install uv (see above) |
| `429 Too Many Requests` from yt-dlp | YouTube rate limiting | Wait, or refresh Firefox cookies |
| `Sign in to confirm` | Bot detection | Log into YouTube in Firefox, close Firefox, retry |
| `Requested format is not available` | Missing `--format mhtml` flag | Already handled in code — check yt-dlp version |
| sqlx compile errors | Missing database | Run `mkdir -p data` then `cargo run` once |
| Template compile errors | Askama syntax issue | Check `templates/` directory exists at project root |

## Relevant Files

- `src/main.rs` — Server startup, port 5001, env var loading
- `src/services/transcript.rs` — yt-dlp invocation (uses `uvx`)
- `migrations/001_initial.sql` — Database schema
- `Cargo.toml` — Rust dependencies
