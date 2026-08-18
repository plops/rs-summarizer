# Walkthrough: YouTube PO Token Provider & yt-dlp Subtitle Acquisition Integration

**Client**: Wol Pumba (`wolpumba@gmail.com`)  
**Project**: `plops/rs-summarizer`  
**Plan & Specs**: [plan.md](file:///workspace/src/rs-summarizer/plan/20260818_01_po_provider/plan.md) | [deps.md](file:///workspace/src/rs-summarizer/plan/20260818_01_po_provider/deps.md) | [task.md](file:///workspace/src/rs-summarizer/plan/20260818_01_po_provider/task.md)  

---

## 1. Executive Summary

YouTube recently increased automated scraping prevention, blocking standard caption downloads with "Sign in to confirm you're not a bot" or empty subtitle listings. To resolve this:

1. A dedicated Proof-of-Origin (PO) Token provider (`brainicism/bgutil-ytdlp-pot-provider:latest`) runs as a background service on the host.
2. `rs-summarizer` now invokes `yt-dlp` with the `bgutil-ytdlp-pot-provider` plugin loaded ephemerally via `uvx` and sets the player client to `mweb`.
3. The JavaScript challenge solver `deno` (`[jsc:deno]`) is installed and configured in the execution environment.
4. The codebase and integration tests were updated, verified against real YouTube videos, and fully validated.

---

## 2. Implemented Code Changes

### 2.1 Subprocess & Extractor Argument Builders in `src/services/transcript.rs`
- **`base_uvx_args()`**: Automatically adds `--with bgutil-ytdlp-pot-provider` before `yt-dlp`, with an optional `DISABLE_POT_PROVIDER=1` escape hatch.
- **`resolve_pot_provider_url()`**: Resolves the POT Provider endpoint in priority order:
  1. `POT_PROVIDER_URL` (or aliases `BGUTIL_POT_PROVIDER_URL`, `YTDLP_POT_PROVIDER_URL`)
  2. Auto-discovery of `http://host.docker.internal:4416` when running inside a container with `host-gateway` configured
  3. Falls back to default `http://127.0.0.1:4416` when co-located on the host
- **`extractor_args()`**: Constructs `--extractor-args` flags for:
  - `youtube:player_client=mweb` (configurable via `YTDLP_PLAYER_CLIENT`)
  - `youtubepot-bgutilhttp:base_url=<URL>`
  - Custom extra args via `YTDLP_EXTRACTOR_ARGS`
- **`cookie_args()`**: Intelligently inspects cookie files (`YTDLP_COOKIES`, `COOKIES_FILE`, `cookies.txt`) and checks for actual Firefox profile directories on disk before attempting to pass `--cookies-from-browser firefox`. In headless containers where no Firefox profile exists, the POT provider generates tokens without requiring browser cookies.
- **Enhanced Diagnostics**: Provides explicit error reporting in `TranscriptError::YtDlpFailed` when the port 4416 PO token server is unreachable or YouTube bot detection is triggered.

### 2.2 Host POT Provider Security & Port Binding in `scripts/install_po_provider.sh`
- Updated port bindings to:
  `-p 127.0.0.1:4416:4416 -p ${DOCKER_BRIDGE_IP}:4416:4416`
- **Security benefit**: Port 4416 is strictly bound to host localhost and the internal Docker bridge (`172.17.0.1`), ensuring the service is accessible by local containers while remaining completely closed to external/public network interfaces.

### 2.3 Container Host-Gateway Configuration in `setup02_run.sh`
- In `example/05_dockerfile_meta/source01/examples/03_ai_env/setup02_run.sh`:
  - Added `--add-host host.docker.internal:host-gateway` so the container can resolve `host.docker.internal` to the host's bridge IP.
  - Added `-p 5001:5001` to expose the `rs-summarizer` web application.
  - Added support for `-p` / `--port` CLI flags.

### 2.4 Documentation & Skill Updates
- Updated [.kiro/skills/yt-dlp-invocation/SKILL.md](file:///workspace/src/rs-summarizer/.kiro/skills/yt-dlp-invocation/SKILL.md) with the new argument structures, environment variables, Deno requirement, and POT provider configuration.

---

## 3. Test & Verification Results

### 3.1 Unit Tests
All 133 unit tests pass cleanly:
```bash
cargo test
# Output: test result: ok. 133 passed; 0 failed; 0 ignored
```

Includes new unit tests for:
- `test_base_uvx_args_default` (includes `--with bgutil-ytdlp-pot-provider`)
- `test_base_uvx_args_disabled` (`DISABLE_POT_PROVIDER=1` fallback)
- `test_extractor_args_default` (`youtube:player_client=mweb`)
- `test_extractor_args_custom_pot_url_and_player_client`
- `test_resolve_pot_provider_url_env_precedence`
- `test_cookie_args_defaults`

### 3.2 Live Integration Tests (Against YouTube)
Ran `cargo test --test integration_transcript -- --ignored`:
```text
running 3 tests
test test_list_subtitles_real_video ... ok
test test_download_auto_subtitles ... ok
test test_full_transcript_pipeline ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.06s
```

All 3 live transcript tests successfully fetched, parsed, and validated real YouTube captions via the POT token provider.

### 3.3 Code Quality & Formatting
- `cargo fmt --check`: Passed with 0 diffs.
- `cargo clippy --lib -- -D warnings`: Passed with 0 warnings.

---

## 4. Key Learnings & Container Requirements

### 4.1 Programs to Include in the Container / Dockerfile
When building or updating the container image (e.g. via `gen_ai_env.lisp` / `Dockerfile`), include the following tools:

1. **`deno`** (Critical):
   - Used by `yt-dlp` as the JavaScript engine (`[jsc:deno]`) to solve client-side YouTube challenges.
   - Installation snippet:
     ```bash
     curl -fsSL https://deno.land/install.sh | sh
     cp /root/.deno/bin/deno /usr/local/bin/deno
     ```
2. **`uv` / `uvx`**:
   - For ephemeral package execution and caching (`astral-sh/uv`).
3. **Host-Gateway Mapping**:
   - Container execution must include `--add-host host.docker.internal:host-gateway` to allow seamless communication between containerized workloads and host helper services.
