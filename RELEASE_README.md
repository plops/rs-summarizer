# rs-summarizer Release Package

This release archive contains the compiled binaries and necessary assets to run the `rs-summarizer` server.

## Package Contents

- `rs-summarizer`: The compiled server binary.
- `static/`: Static assets (styles, javascript) for the web page.
- `migrations/`: SQL database migration scripts (required for first-time setup or schema upgrades).
- `README.md`: This instruction file.

## Setup and DB Migrations

The `migrations` directory contains SQL files that define the database schema and updates. 

> [!IMPORTANT]
> The `migrations` directory must be located in the same directory as the `rs-summarizer` binary (i.e., at `./migrations` relative to the binary path).
>
> When the `rs-summarizer` server starts up, it automatically checks the `./migrations` folder for any new database scripts (such as `002_add_grounding_and_url_context.sql`) and applies them to the database file (`data/summaries.db`).

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
   Create a file named `.env` in the same directory as the server binary (usually `/home/kiel/host/.env`) and add your API key:
   ```env
   GEMINI_API_KEY=your_gemini_api_key_here
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

