# Dependencies & System Components: YouTube PO Token Provider & yt-dlp Integration

This document records all dependencies, packages, system tools, and container images associated with the YouTube Proof-of-Origin (PO) Token provider and `yt-dlp` transcript acquisition pipeline.

---

## 1. Rust Crate Dependencies

**No new Rust crates are required in `Cargo.toml`.**
The existing workspace dependencies (`tokio`, `thiserror`, `tracing`, `tempfile`, `regex`) provide all required primitives for subprocess execution, environment variable handling, structured logging, and VTT parsing.

Existing relevant dependencies:
- **`tokio`** (`1.52`, Org: `tokio-rs/tokio`): Async subprocess management (`tokio::process::Command`).
- **`tracing`** (`0.1`, Org: `tokio-rs/tracing`): Structured logging for commands and diagnostic telemetry.
- **`thiserror`** (`2.0`, Org: `dtolnay/thiserror`): Domain error definitions (`TranscriptError`).

---

## 2. Python Packages & yt-dlp Plugins

### 2.1 `bgutil-ytdlp-pot-provider` (Python / PyPI)
- **Package Name**: `bgutil-ytdlp-pot-provider`
- **GitHub Repository**: [brainicism/bgutil-ytdlp-pot-provider](https://github.com/brainicism/bgutil-ytdlp-pot-provider)
- **GitHub Organization / Owner**: `brainicism`
- **PyPI URL**: [pypi.org/project/bgutil-ytdlp-pot-provider/](https://pypi.org/project/bgutil-ytdlp-pot-provider/)
- **License**: MIT
- **Purpose**: `yt-dlp` plugin that interfaces with the external PO Token Provider HTTP server to dynamically acquire Proof-of-Origin tokens for YouTube's `mweb` and `web` player clients.
- **Invocation**: Ephemerally installed and loaded via `uvx --with bgutil-ytdlp-pot-provider yt-dlp`.

#### DeepWiki Query Reference:
- **Repo Target**: `brainicism/bgutil-ytdlp-pot-provider`
- **Query Pattern**: `"How to configure base_url and player_client for bgutil-ytdlp-pot-provider with yt-dlp?"`

### 2.2 `yt-dlp` (Python / PyPI)
- **Package Name**: `yt-dlp`
- **GitHub Repository**: [yt-dlp/yt-dlp](https://github.com/yt-dlp/yt-dlp)
- **GitHub Organization / Owner**: `yt-dlp`
- **PyPI URL**: [pypi.org/project/yt-dlp/](https://pypi.org/project/yt-dlp/)
- **License**: Unlicense
- **Purpose**: Command-line program and library to download videos, metadata, and subtitles/captions from YouTube.
- **Invocation**: Invoked via `uvx` without global pip installation.

---

## 3. Host & Container Services

### 3.1 `brainicism/bgutil-ytdlp-pot-provider` (Docker Image)
- **Image**: `brainicism/bgutil-ytdlp-pot-provider:latest`
- **Upstream Project**: [brainicism/bgutil-ytdlp-pot-provider](https://github.com/brainicism/bgutil-ytdlp-pot-provider)
- **Deployment Script**: [scripts/install_po_provider.sh](file:///workspace/src/rs-summarizer/scripts/install_po_provider.sh)
- **Default Port**: `4416` (HTTP)
- **Host Location Note**: Runs on the host computer (`pumba host`), accessible either on `http://127.0.0.1:4416` (when co-located) or via remote URL/IP (`http://<host-ip>:4416` or `http://host.docker.internal:4416` configured via `POT_PROVIDER_URL`).

---

## 4. System-Level Dependencies & Runtimes

### 4.1 `deno` (JavaScript / TypeScript Runtime)
- **Program Name**: `deno`
- **GitHub Repository**: [denoland/deno](https://github.com/denoland/deno)
- **GitHub Organization / Owner**: `denoland`
- **Installed Version**: `2.9.5+` (installed at `/usr/local/bin/deno`)
- **Purpose**: High-performance JavaScript runtime utilized by `yt-dlp` (`[jsc:deno]`) to solve YouTube's client-side JavaScript challenges and signature deciphering routines.
- **Docker Installation**:
  ```bash
  curl -fsSL https://deno.land/install.sh | sh
  cp /root/.deno/bin/deno /usr/local/bin/deno
  ```

### 4.2 `uv` / `uvx` (Python Package Manager & Tool Runner)
- **Program Name**: `uv` / `uvx`
- **GitHub Repository**: [astral-sh/uv](https://github.com/astral-sh/uv)
- **GitHub Organization / Owner**: `astral-sh`
- **Purpose**: Fast Python package runner to execute `yt-dlp` alongside ephemeral plugin packages (`--with bgutil-ytdlp-pot-provider`) in isolated caching environments without polluting global Python.
