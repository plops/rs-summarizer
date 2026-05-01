---
name: release-process
description: Use when creating a release, bumping the version, tagging a commit, or working with the GitHub Actions release workflow for rs-summarizer.
---

# Release Process

## Overview

rs-summarizer uses GitHub Actions to automatically build and publish releases for Linux and macOS when a git tag starting with `v` is pushed.

## How It Works

1. A developer pushes a tag like `v0.2.0` to the repository
2. GitHub Actions triggers the `.github/workflows/release.yml` workflow
3. The workflow builds release binaries for:
   - Linux x86_64 (`rs-summarizer-linux-amd64.tar.gz`)
   - macOS x86_64 (`rs-summarizer-macos-amd64.tar.gz`)
   - macOS ARM64 (`rs-summarizer-macos-arm64.tar.gz`)
4. Each archive includes the binary **and** the `static/` directory (CSS, JS) required at runtime
5. Built assets are uploaded to a GitHub Release

## Runtime Files

The release archive contains:
- `rs-summarizer` — the server binary
- `static/htmx.min.js` — HTMX library for frontend interactivity
- `static/pico.min.css` — Pico CSS framework for styling

Askama templates and SQLite migrations are compiled into the binary at build time and do not need separate files.

## Release Scripts

### Pre-release checks

```bash
./scripts/release-check.sh
```

Verifies:
- Clean working tree (no uncommitted changes)
- On the `main` branch
- Up to date with origin
- `cargo check` passes
- `cargo clippy` passes
- Unit tests pass

### Creating a release

```bash
./scripts/release.sh <version>
# Example:
./scripts/release.sh 0.2.0
```

This script:
1. Validates the version format (semver: `X.Y.Z`)
2. Updates `Cargo.toml` version
3. Runs `cargo check` to verify the build
4. Updates `Cargo.lock`
5. Commits the version bump
6. Creates a git tag `v<version>`
7. Pushes the commit and tag to origin

## Manual Release Steps

If you prefer to release manually:

```bash
# 1. Update version in Cargo.toml
sed -i 's/^version = ".*"/version = "0.2.0"/' Cargo.toml

# 2. Verify the build
cargo check
cargo test

# 3. Commit
git add Cargo.toml Cargo.lock
git commit -m "Release v0.2.0"

# 4. Tag and push
git tag v0.2.0
git push origin main
git push origin v0.2.0
```

## Monitoring

After pushing a tag, monitor the build at:
https://github.com/plops/rs-summarizer/actions

## Relevant Files

- `.github/workflows/release.yml` — GitHub Actions workflow definition
- `scripts/release.sh` — Automated release script
- `scripts/release-check.sh` — Pre-release validation
- `Cargo.toml` — Version is defined here

## Version Numbering

Follow semantic versioning:
- **MAJOR** (1.0.0) — Breaking changes to the web interface or API
- **MINOR** (0.2.0) — New features (new models, new endpoints, UI improvements)
- **PATCH** (0.1.1) — Bug fixes, dependency updates, minor tweaks
