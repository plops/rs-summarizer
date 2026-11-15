# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.11.0] - 2026-11-15

### Added

- New model catalog: `gemini-4-hydrogen` (flagship with `xhigh` thinking),
  `gemini-3.5-flash-lite` (newest 500-RPD model, now backing `auto`),
  alongside `gemini-3.8-flash`, `gemini-3.6-flash`, `gemini-3-flash-preview`,
  `gemini-3.1-flash-lite`, `gemini-2.5-flash-lite`, `gemma-4-31b-it`,
  `gemma-4-26b-a4b-it`, `hetzner-qwen-3.8-27b`, and `hetzner-qwen-3.6-35b`.
- `ThinkingPreference::XHigh` (`xhigh`) persisted per summary; the UI offers
  an `Extra High (xhigh)` level driven by per-model `data-thinking-levels`
  / `data-default-thinking` attributes instead of prefix sniffing.
- Persistent daily RPD counters in the new `model_daily_usage` table, so
  server restarts no longer reset quotas.
- Per-model sliding 60-second TPM windows (`TpmTracker`); prompts that can
  never fit a model's per-minute budget are rejected with a German
  user-facing message.
- `scripts/extract-changelog.sh` renders release notes from this file for
  the GitHub release workflow.
- SSRF-hardened Hacker News article fetch (scheme check, validated
  redirects, public-IP-only resolution, 5 MiB body cap).

### Changed

- `AUTO_MODEL` is now `gemini-3.5-flash-lite`.
- Fallback chains rebuilt around the invariant that a cheap or free model
  never falls back onto an expensive-tier model; `gemma-4-*` models are in
  no automatic chain.
- `xhigh` requests stream through a raw Interactions SSE call because
  `gemini-rust 2.0.0` has no `XHigh` variant; other levels clamp across
  generations (`xhigh` to `high`, `minimal` to `low`) instead of HTTP 400.
- `today_la()` now uses the exact US Pacific switchover instants (02:00
  local on the 2nd Sunday of March / 1st Sunday of November).
- Hetzner streaming is epoch-guarded, requires a terminal `finish_reason`,
  and separates leading `<think>...</think>` reasoning into thinking text.
- `yt-dlp` runs strictly serially (`MAX_CONCURRENT_YTDLP = 1`,
  `YTDLP_DELAY_MS = 2000`) with a `YTDLP_TIMEOUT_SECS` timeout (default
  90 s) and no zombie processes.
- The submit route resolves `auto` before quota checks and rejects only when
  the whole fallback chain is RPD-exhausted.
- Costs assume `total_output_tokens` already includes thinking tokens (per
  Google billing docs); thinking tokens stay visible in the database.
- `doc/rate_limiting_error_handling_architecture.md` rewritten to match the
  actual architecture.
- Release pipeline: notes are extracted from this changelog and shown above
  the artifacts; packaging guards migrations `006`, `010`, and `011`.

### Fixed

- Migration `011` rebuilds `summaries` with `foreign_keys = OFF` outside a
  transaction, preserving `summary_ratings` rows that `ON DELETE CASCADE`
  would otherwise wipe, and keeps legacy NULL rows readable via `COALESCE`.

### Removed

- Deprecated models `gemini-3.7-flash`, `gemini-3.5-flash`, and
  `gemini-2.5-flash`.
- Dead `MetadataCache` module (`src/cache.rs`).

### Security

- HN article fetcher validates every redirect hop and resolved IP, and caps
  bodies at 5 MiB.

## [1.10.1] - 2026-10-24

### Fixed

- Vendor the `third_party/fast-umap` tree so release builds are reproducible.

## [1.10.0] - 2026-10-24

### Added

- YouTube/HN titles stored per summary and shown in the UI, with a
  rate-limited background backfill worker (`title_fetch_budget`).

## [1.9.0] - 2026-10-23

### Added

- Lightweight all-entries browser (`/entries`) with lazy detail loading.

## [1.8.5] - 2026-10-08

### Added

- Form settings (model, thinking effort, language, glossary, search flags)
  persist in browser `localStorage`.

### Changed

- `auto` always resolves to `gemini-3.1-flash-lite` (no duration heuristic).

## [1.8.4] - 2026-10-06

### Added

- `rocketrecap.com` version line in the YouTube clipboard footer.

## [1.8.3] - 2026-10-06

### Added

- Model and cost footer appended to the YouTube clipboard text.

## [1.8.2] - 2026-10-06

### Added

- Model name and cost shown at the end of each summary.

## [1.8.1] - 2026-10-06

### Added

- Deduplicated rendering with a YouTube copy button.

### Fixed

- Legacy summary rows with NULLs no longer fail decoding (`COALESCE`
  read projection).
- German user-facing error messages instead of raw API internals.
- Friendly retry times instead of raw ISO timestamps; costs formatted with
  2 or 3 decimals; single-digit minute timestamps link correctly.

## [1.8.0] - 2026-09-04

### Added

- Generator provenance: compile-time application version displayed in the
  UI and persisted per summary (`rs_summarizer_version`).

## [1.7.5] - 2026-09-04

### Fixed

- Trust completed Gemini streams instead of discarding them.

## [1.7.4] - 2026-09-04

### Fixed

- Trust completed Gemini streams instead of discarding them.

## [1.7.3] - 2026-09-04

### Fixed

- Handle unsupported thinking levels and stream failures.

## [1.7.2] - 2026-09-04

### Fixed

- Handle unsupported thinking levels and stream failures.

## [1.7.1] - 2026-09-04

### Fixed

- Quarantine the legacy queued-generation backlog instead of replaying it
  on startup.

## [1.7.0] - 2026-09-04

### Fixed

- Retry transient Gemini stream failures.

[Unreleased]: https://github.com/plops/rs-summarizer/compare/v1.11.0...HEAD
[1.11.0]: https://github.com/plops/rs-summarizer/compare/v1.10.1...v1.11.0
[1.10.1]: https://github.com/plops/rs-summarizer/compare/v1.10.0...v1.10.1
[1.10.0]: https://github.com/plops/rs-summarizer/compare/v1.9.0...v1.10.0
[1.9.0]: https://github.com/plops/rs-summarizer/compare/v1.8.5...v1.9.0
[1.8.5]: https://github.com/plops/rs-summarizer/compare/v1.8.4...v1.8.5
[1.8.4]: https://github.com/plops/rs-summarizer/compare/v1.8.3...v1.8.4
[1.8.3]: https://github.com/plops/rs-summarizer/compare/v1.8.2...v1.8.3
[1.8.2]: https://github.com/plops/rs-summarizer/compare/v1.8.1...v1.8.2
[1.8.1]: https://github.com/plops/rs-summarizer/compare/v1.8.0...v1.8.1
[1.8.0]: https://github.com/plops/rs-summarizer/compare/v1.7.5...v1.8.0
[1.7.5]: https://github.com/plops/rs-summarizer/compare/v1.7.4...v1.7.5
[1.7.4]: https://github.com/plops/rs-summarizer/compare/v1.7.3...v1.7.4
[1.7.3]: https://github.com/plops/rs-summarizer/compare/v1.7.2...v1.7.3
[1.7.2]: https://github.com/plops/rs-summarizer/compare/v1.7.1...v1.7.2
[1.7.1]: https://github.com/plops/rs-summarizer/compare/v1.7.0...v1.7.1
[1.7.0]: https://github.com/plops/rs-summarizer/compare/v1.6.2...v1.7.0
