# Export Viewer

This is a helper script to view and query `rs-summarizer` SQLite database exports using `pandas` and `uv`.

## Running the Viewer

1. Decompress your exported database (e.g., `zstd -d summaries_compact.db.zst`).
2. Place `summaries_compact.db` in this directory.
3. Run the script using `uv`:
   ```bash
   uv run python read_summaries.py
   ```

Alternatively, you can specify a custom database path using the `COMPACT_DB_PATH` environment variable:
```bash
COMPACT_DB_PATH=/path/to/my_data.db uv run python read_summaries.py
```
