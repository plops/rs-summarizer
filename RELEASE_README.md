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
