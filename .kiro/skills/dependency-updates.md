---
name: dependency-updates
description: Use when updating Cargo dependencies, checking for new crate versions, evaluating semver bumps, or deciding whether to upgrade a specific crate.
inclusion: manual
---

# Updating Dependencies

## Checking for Updates

Use `cargo update --dry-run --verbose` to see what's available:
- Shows compatible updates (within current semver specs) that can be applied immediately
- Shows "Unchanged" packages that have newer versions outside the current spec
- No extra tooling needed — this is built into cargo

## Applying Compatible Updates

Run `cargo update` to lock in all semver-compatible patches without changing Cargo.toml.

## Checking Latest Versions on crates.io

Use `cargo search <crate-name>` to see the latest published version of a specific crate.

## Update Strategy

1. **Patch/minor updates** (within semver spec): Apply freely with `cargo update`. Low risk.
2. **Major version bumps** (require Cargo.toml change): Evaluate individually.
   - Check the crate's changelog for breaking changes
   - Update in a separate PR with focused testing
   - Prioritize security-related updates

## Project-Specific Notes

- **yt-dlp crate**: Currently pinned to 1.x. The 2.x line has API changes — test transcript download thoroughly if upgrading.
- **askama**: Currently on 0.12. Newer versions (0.14+) changed derive macro behavior. Template compilation may need adjustments.
- **fantoccini** (dev-dep): Used for browser integration tests. Keep at latest (currently 0.22).

## Do NOT Install

- `cargo-outdated` — takes too long to compile and `cargo update --dry-run --verbose` provides the same information.
