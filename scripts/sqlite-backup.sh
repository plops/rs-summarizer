#!/usr/bin/env bash
# safe SQLite WAL backup script for rs-summarizer
# Usage: ./sqlite-backup.sh [db_path] [backup_path]

set -euo pipefail

DB_PATH="${1:-data/summaries.db}"
BACKUP_PATH="${2:-data/summaries_backup.db}"

# Verify sqlite3 command is available
if ! command -v sqlite3 &> /dev/null; then
    echo "Error: sqlite3 command not found. Please install sqlite3." >&2
    exit 1
fi

# Verify source database file exists
if [ ! -f "$DB_PATH" ]; then
    echo "Error: Source database '$DB_PATH' does not exist." >&2
    exit 1
fi

# Ensure destination directory exists
mkdir -p "$(dirname "$BACKUP_PATH")"

echo "Starting hot-backup of WAL database '$DB_PATH' to '$BACKUP_PATH'..."
if sqlite3 "$DB_PATH" ".backup '$BACKUP_PATH'"; then
    echo "✓ Backup completed successfully: $BACKUP_PATH"
else
    echo "✗ Backup failed!" >&2
    exit 1
fi
