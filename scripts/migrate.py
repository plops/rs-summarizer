#!/usr/bin/env python3
"""
Migrate old Python 'items' table to Rust 'summaries' table.

The Rust app must have been run at least once so that the 'summaries' table
and '_sqlx_migrations' table already exist.

Usage:
    python3 migrate.py [path_to_db]
"""

import sqlite3
import sys

DB_PATH = sys.argv[1] if len(sys.argv) > 1 else "data/summaries.db"

# Columns in the Rust 'summaries' table (in order)
COLUMNS = [
    "identifier",
    "model",
    "transcript",
    "host",
    "original_source_link",
    "include_comments",
    "include_timestamps",
    "include_glossary",
    "output_language",
    "summary",
    "summary_done",
    "summary_input_tokens",
    "summary_output_tokens",
    "summary_timestamp_start",
    "summary_timestamp_end",
    "timestamps",
    "timestamps_done",
    "timestamps_input_tokens",
    "timestamps_output_tokens",
    "timestamps_timestamp_start",
    "timestamps_timestamp_end",
    "timestamped_summary_in_youtube_format",
    "cost",
    "embedding",
    "embedding_model",
    "full_embedding",
]

# NOT NULL TEXT columns -> default ''
TEXT_COLS = {
    "model", "transcript", "host", "original_source_link", "output_language",
    "summary", "summary_timestamp_start", "summary_timestamp_end", "timestamps",
    "timestamps_timestamp_start", "timestamps_timestamp_end",
    "timestamped_summary_in_youtube_format", "embedding_model",
}

# NOT NULL INTEGER columns -> default 0
INT_COLS = {
    "summary_input_tokens", "summary_output_tokens",
    "timestamps_input_tokens", "timestamps_output_tokens",
}

# NOT NULL BOOLEAN columns -> default 0
BOOL_COLS = {
    "include_comments", "include_timestamps", "include_glossary",
    "summary_done", "timestamps_done",
}

# NOT NULL REAL columns -> default 0.0
REAL_COLS = {"cost"}


def coalesce_expr(col):
    """Build a COALESCE expression to handle NULLs from the old schema."""
    if col in TEXT_COLS:
        return f"COALESCE(\"{col}\", '')"
    if col in INT_COLS:
        return f"COALESCE(\"{col}\", 0)"
    if col in BOOL_COLS:
        return f"COALESCE(\"{col}\", 0)"
    if col in REAL_COLS:
        return f"COALESCE(\"{col}\", 0.0)"
    # identifier, embedding, full_embedding — pass through as-is
    return f"\"{col}\""


def main():
    print(f"Opening database: {DB_PATH}")
    con = sqlite3.connect(DB_PATH)
    cur = con.cursor()

    # Verify source table exists
    cur.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='items'")
    if not cur.fetchone():
        print("ERROR: 'items' table not found. Is this the right database?")
        sys.exit(1)

    # Verify destination table exists
    cur.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='summaries'")
    if not cur.fetchone():
        print("ERROR: 'summaries' table not found. Run the Rust app once first so migrations apply.")
        sys.exit(1)

    # Check if summaries already has data
    cur.execute("SELECT COUNT(*) FROM summaries")
    existing = cur.fetchone()[0]
    if existing > 0:
        print(f"WARNING: 'summaries' already has {existing} rows. Aborting to avoid duplicates.")
        sys.exit(1)

    # Count source rows
    cur.execute("SELECT COUNT(*) FROM items")
    total = cur.fetchone()[0]
    print(f"Found {total} rows in 'items' table.")

    if total == 0:
        print("Nothing to migrate.")
        sys.exit(0)

    # Build the INSERT ... SELECT statement
    select_exprs = ", ".join(coalesce_expr(c) for c in COLUMNS)
    col_list = ", ".join(f'"{c}"' for c in COLUMNS)

    sql = f'INSERT INTO summaries ({col_list}) SELECT {select_exprs} FROM items'
    print("Executing migration...")
    cur.execute(sql)
    migrated = cur.rowcount
    print(f"Inserted {migrated} rows into 'summaries'.")

    # Note: sqlite_sequence for AUTOINCREMENT is handled automatically by SQLite.
    # When the next auto-generated insert happens, SQLite will use MAX(rowid)+1
    # which is correct since we preserved the original identifier values.
    cur.execute("SELECT MAX(identifier) FROM summaries")
    max_id = cur.fetchone()[0] or 0

    con.commit()
    con.close()
    print(f"Done. Migrated {migrated} rows. Max identifier is {max_id}. Next new id will be > {max_id}.")


if __name__ == "__main__":
    main()
