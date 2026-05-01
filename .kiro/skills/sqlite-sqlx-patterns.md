---
name: sqlite-sqlx-patterns
description: Use when writing sqlx queries, working with SQLite WAL mode, storing or loading embedding blobs, or modifying the database schema.
inclusion: manual
---

# SQLite WAL Mode + sqlx Patterns

## Overview

rs-summarizer uses SQLite with WAL (Write-Ahead Logging) mode via the `sqlx` crate for async database access. This enables concurrent reads during writes — critical for HTMX polling while background tasks update summaries.

## Connection Pool Setup

```rust
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

let options = SqliteConnectOptions::from_str("sqlite:data/summaries.db")?
    .create_if_missing(true)
    .journal_mode(SqliteJournalMode::Wal);

let pool = SqlitePoolOptions::new()
    .max_connections(5)
    .connect_with(options)
    .await?;

sqlx::migrate!("./migrations").run(&pool).await?;
```

## Migration

Single migration file at `migrations/001_initial.sql`:
- `PRAGMA journal_mode=WAL;` at the top
- Full `summaries` table schema (26 columns)
- Composite index `idx_dedup_lookup` on `(original_source_link, model, summary_timestamp_start)`

## Query Patterns

### Parameterized queries (always — no string interpolation)

```rust
sqlx::query("UPDATE summaries SET summary = summary || ? WHERE identifier = ?")
    .bind(chunk)
    .bind(identifier)
    .execute(db)
    .await?;
```

### Fetch with struct mapping

```rust
let row = sqlx::query_as::<_, Summary>("SELECT * FROM summaries WHERE identifier = ?")
    .bind(identifier)
    .fetch_optional(db)
    .await?;
```

### Scalar queries

```rust
let id = sqlx::query_scalar::<_, i64>(
    "SELECT identifier FROM summaries WHERE original_source_link = ? AND model = ? LIMIT 1"
)
.bind(url)
.bind(model)
.fetch_optional(db)
.await?;
```

## Key Operations (src/db.rs)

| Function | Purpose |
|----------|---------|
| `init_db()` | Create pool, run migrations |
| `insert_new_summary()` | Insert row, return `last_insert_rowid()` |
| `fetch_summary()` | Get full row by identifier |
| `update_transcript()` | Set transcript field |
| `update_summary_chunk()` | Append chunk: `summary = summary \|\| ?` |
| `mark_summary_done()` | Set `summary_done=1`, tokens, cost, timestamp |
| `mark_timestamps_done()` | Set `timestamps_done=1`, YouTube format text |
| `store_embedding()` | Store embedding blob + model name |
| `fetch_all_embeddings()` | Get all `(id, blob)` pairs for similarity search |
| `fetch_browse_page()` | Paginated query: `ORDER BY id DESC LIMIT 20 OFFSET ?` |

## In-Memory Database for Tests

```rust
let db_pool = db::init_db("sqlite::memory:").await?;
```

## Embedding Storage

Embeddings are stored as raw f32 byte blobs (little-endian):
- Store: `embedding.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>()`
- Load: `bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0],c[1],c[2],c[3]])).collect()`
- Size invariant: `blob.len() == dimensions * 4`

## Relevant Files

- `src/db.rs` — All database operations
- `migrations/001_initial.sql` — Schema definition
- `src/models.rs` — `Summary` struct with `sqlx::FromRow`
