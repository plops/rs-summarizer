# rs-summarizer Release Package

This release archive contains the compiled binaries and necessary assets to run the `rs-summarizer` server.

## Package Contents

- `rs-summarizer`: The compiled server binary.
- `config/`: Model configuration files (`config/models.json`).
- `static/`: Static assets (styles, javascript) for the web page.
- `migrations/`: SQL database migration scripts (required for first-time setup or schema upgrades).
- `install_po_provider.sh`: Helper script to start the YouTube Proof-of-Origin (PO) Token Provider Docker container.
- `sqlite-backup.sh`: Online hot-backup script for SQLite WAL database.
- `rs-summarizer.service`: Systemd service unit template.
- `README.md`: This instruction file.

## Setup and DB Migrations

The `migrations` directory contains SQL files that define the database schema and updates. 

> [!IMPORTANT]
> The `migrations` directory must be located in the same directory as the `rs-summarizer` binary (i.e., at `./migrations` relative to the binary path).
>
> When the `rs-summarizer` server starts up, it automatically checks the `./migrations` folder for any new database scripts (such as `002_add_grounding_and_url_context.sql`) and applies them to the database file (`data/summaries.db`).

## YouTube Captions & Proof-of-Origin (PO) Token Setup

YouTube enforces strict bot-detection and client-verification measures. To reliably download subtitles and transcripts without receiving "Sign in to confirm you're not a bot" or empty captions, `rs-summarizer` integrates an external PO Token Provider service (`bgutil-ytdlp-pot-provider`) and uses the `mweb` (Mobile Web) player client alongside the Deno JavaScript challenge solver.

### 1. Starting the PO Token Provider Container

Run the included `install_po_provider.sh` script on the host to start the background container:

```bash
chmod +x install_po_provider.sh
./install_po_provider.sh
```

This launches the `brainicism/bgutil-ytdlp-pot-provider:latest` container listening on port `4416`. It binds strictly to host loopback (`127.0.0.1:4416`) and the Docker bridge gateway (`172.17.0.1:4416`), ensuring that local processes and Docker containers can generate tokens while keeping port 4416 secure and closed to external/public network interfaces.

### 2. JavaScript Engine Requirement (`deno`)

`yt-dlp` automatically uses `deno` (`[jsc:deno]`) to solve YouTube's JavaScript challenges. Ensure `deno` is installed and available in `PATH`:

```bash
# Install Deno (if not already present):
curl -fsSL https://deno.land/install.sh | sh
sudo cp /root/.deno/bin/deno /usr/local/bin/deno
```

### 3. Environment Variables for YouTube & POT Provider

Configure these in your `.env` file as needed:

| Variable | Default | Description |
|:---|:---|:---|
| `POT_PROVIDER_URL` | `http://127.0.0.1:4416` | Endpoint URL of the PO Token Provider. When running inside Docker, defaults automatically to `http://host.docker.internal:4416` if resolved. (Aliases: `BGUTIL_POT_PROVIDER_URL`, `YTDLP_POT_PROVIDER_URL`). |
| `YTDLP_PLAYER_CLIENT` | `mweb` | YouTube player client used by yt-dlp (`mweb`, `web`, `android`, etc.). `mweb` is recommended for PO token compatibility. |
| `YTDLP_EXTRACTOR_ARGS` | *(empty)* | Optional extra extractor arguments passed directly to `yt-dlp`. |
| `DISABLE_POT_PROVIDER` | `false` | Set to `1` or `true` as an emergency fallback to omit the `bgutil-ytdlp-pot-provider` plugin. |
| `MAX_CONCURRENT_YTDLP` | `3` | Maximum number of simultaneous `yt-dlp` download processes. |
| `YTDLP_COOKIES` / `COOKIES_FILE` | *(auto)* | Path to a Netscape format `cookies.txt` file. |
| `YTDLP_COOKIES_FROM_BROWSER` | *(auto)* | Browser name for cookies (e.g. `firefox`). Automatically detected if Firefox profile directories exist in `$HOME`. |

## Graceful Shutdown (WAL File Cleanup)

The application database uses SQLite in WAL (Write-Ahead Log) mode.
To ensure that all temporary transaction files (like `summaries.db-wal` and `summaries.db-shm`) are successfully merged and deleted, the server intercepts termination signals (`Ctrl-C` / `SIGINT` or `SIGTERM`) and performs a clean shutdown.

To stop the server cleanly:
- Press `Ctrl-C` (or send `SIGTERM`).
- Wait for the server to log `Cleaning up database connections...` and `Shutdown complete`.
- This will merge all transaction data back into the main `summaries.db` file and cleanly remove the auxiliary files.

## Systemd Service Installation

To run `rs-summarizer` as a background system service that starts automatically on system boot and restarts if it crashes, use the included `rs-summarizer.service` template.

### Steps to Install:

1. **Set Up the Environment File**:
   Create a file named `.env` in the same directory as the server binary (usually `/home/kiel/host/.env`) and add your configuration:
   ```env
   GEMINI_API_KEY=your_gemini_api_key_here
   HETZNER_API_KEY=your_hetzner_api_key_here
   # Optional custom base URL:
   # HETZNER_BASE_URL=https://inference.hetzner.com/api/v1
   # Optional POT provider URL (if on another machine or container bridge):
   # POT_PROVIDER_URL=http://127.0.0.1:4416
   ```
   Restrict file permissions to protect your secrets:
   ```bash
   chmod 600 /home/kiel/host/.env
   ```

2. **Copy the Service File**:
   Copy the `rs-summarizer.service` file to systemd:
   ```bash
   sudo cp rs-summarizer.service /etc/systemd/system/
   ```

3. **Reload and Enable the Service**:
   Reload systemd to detect the new service configuration, then enable it to start on boot:
   ```bash
   sudo systemctl daemon-reload
   sudo systemctl enable rs-summarizer.service
   ```

4. **Start and Monitor the Service**:
   Start the service and check its logs using journalctl:
   ```bash
   sudo systemctl start rs-summarizer.service
   sudo systemctl status rs-summarizer.service
   # View live logs:
   sudo journalctl -u rs-summarizer.service -f
   ```

## SQLite Database Backups

Because the database runs in SQLite WAL (Write-Ahead Log) mode, standard file-copy backups (`cp`) of `summaries.db` might produce an inconsistent or corrupted backup copy if write transactions are active.

A safe backup script `sqlite-backup.sh` is included in this package. It performs an online "hot-backup" using SQLite's backup API, safely incorporating any active transactions.

### Running a Backup:

```bash
# Execute backup to default path (data/summaries_backup.db)
./sqlite-backup.sh

# Or specify custom source and destination paths:
./sqlite-backup.sh /path/to/source.db /path/to/destination.db
```

### Automating with Cron:

To automate backups every day at 2:00 AM, add a cron job:
```bash
0 2 * * * /home/kiel/host/sqlite-backup.sh /home/kiel/host/data/summaries.db /home/kiel/backups/summaries_$(date +\%F).db
```

## Database Exports (Compact & Compressed)

To download your summaries for offline use or local experimentation without downloading heavy YouTube transcripts, you can run the `export-db` CLI task. This runs safely in the background alongside the hosting process.

### 1. Export WITHOUT Embeddings (Recommended, ~25MB)
Excludes raw transcripts and float vector embeddings. This is the smallest file format, ideal for text-based analysis:
```bash
./rs-summarizer export-db \
  --source data/summaries.db \
  --output data/summaries_compact.db \
  --compress
```
* **Output**: `data/summaries_compact.db.zst` (~25.7 MB)

### 2. Export WITH Embeddings (~170MB)
Excludes raw transcripts but retains dense vector embeddings. Ideal for semantic search and clustering experiments locally:
```bash
./rs-summarizer export-db \
  --source data/summaries.db \
  --output data/summaries_compact.db \
  --include-embeddings \
  --compress
```
* **Output**: `data/summaries_compact.db.zst` (~170.0 MB)

### Decompression at Home:
Download the `.zst` file from the host and run `zstd -d` to extract it:
```bash
zstd -d summaries_compact.db.zst
```
This yields a standard SQLite database (`summaries_compact.db`) compatible with Python, pandas, or standard SQLite GUI editors.
