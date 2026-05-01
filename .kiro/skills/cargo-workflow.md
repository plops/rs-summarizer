---
name: cargo-workflow
description: Use when building, running, linting with clippy, formatting with rustfmt, or running cargo commands for the rs-summarizer project.
inclusion: manual
---

# Cargo Workflow

## Overview

Standard development commands for building, testing, linting, and running rs-summarizer.

## Quick Reference

```bash
# Build (debug)
cargo build

# Build (release, optimized)
cargo build --release

# Run the server (release mode, port 5001)
export GEMINI_API_KEY=$(cat ~/api_key.txt)
cargo run --release

# Lint with clippy
cargo clippy -- -W clippy::all

# Format code
cargo fmt

# Check formatting without modifying
cargo fmt -- --check

# Type-check without building
cargo check
```

## Testing Commands

```bash
# Unit tests only (fast, no network)
cargo test

# Specific module tests
cargo test utils::url_validator
cargo test services::embedding
cargo test cache

# Integration tests (require network + API key)
GEMINI_API_KEY=$(cat ~/api_key.txt) cargo test --test integration_pipeline -- --ignored

# Transcript tests (require Firefox cookies)
cargo test --test integration_transcript -- --ignored

# Browser tests (require geckodriver + Firefox)
cargo test --test integration_browser -- --ignored

# All tests including integration
GEMINI_API_KEY=$(cat ~/api_key.txt) cargo test -- --include-ignored

# With output visible
cargo test -- --nocapture
```

## Dependency Management

```bash
# Check for available updates (dry run)
cargo update --dry-run --verbose

# Apply semver-compatible updates
cargo update

# Search for a crate's latest version
cargo search <crate-name>
```

## Build Notes

- The project uses `sqlx` with compile-time query checking. If you get sqlx errors, ensure the database exists or use `SQLX_OFFLINE=true`.
- Askama templates are checked at compile time. Template syntax errors show as compile errors.
- The `static/` directory must be present at runtime (relative to CWD) for `tower_http::ServeDir`.

## Environment Variables

| Variable | Required | Purpose |
|----------|----------|---------|
| `GEMINI_API_KEY` | Yes (for API calls) | Google Gemini API authentication |
| `RUST_LOG` | No | Tracing log level (e.g., `info`, `debug`) |

## Relevant Files

- `Cargo.toml` — Dependencies and project metadata
- `Cargo.lock` — Locked dependency versions
- `src/main.rs` — Entry point (server on port 5001)
- `src/lib.rs` — Library crate (used by integration tests)
