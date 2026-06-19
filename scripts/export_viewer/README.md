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

## How to Generate the Export on the VPS

To generate the `.zst` database file on your Hetzner VPS, run the `export-db` task:

* **Export WITHOUT Embeddings (Recommended, ~25MB)**:
  ```bash
  /home/kiel/host/rs-summarizer export-db --source /home/kiel/host/data/summaries.db --output /home/kiel/host/data/summaries_compact.db --compress
  ```

* **Export WITH Embeddings (~170MB)**:
  ```bash
  /home/kiel/host/rs-summarizer export-db --source /home/kiel/host/data/summaries.db --output /home/kiel/host/data/summaries_compact.db --include-embeddings --compress
  ```

This creates a `/home/kiel/host/data/summaries_compact.db.zst` file, which you can copy to your local machine using `scp`.

