-- no-transaction
-- Migration 011: widen the thinking_level CHECK to 'xhigh', add the
-- title backfill scan index, and persist daily per-model RPD counters.
--
-- The summaries table is rebuilt (instead of ALTER) so the CHECK
-- constraint can change. PRAGMA foreign_keys = OFF must run OUTSIDE a
-- transaction (hence -- no-transaction above): otherwise DROP TABLE
-- summaries would cascade through summary_ratings.summary_id
-- ON DELETE CASCADE and wipe every stored rating.
PRAGMA foreign_keys = OFF;

BEGIN IMMEDIATE;

CREATE TABLE summaries_new (
    identifier INTEGER PRIMARY KEY AUTOINCREMENT,
    model TEXT NOT NULL DEFAULT '',
    transcript TEXT NOT NULL DEFAULT '',
    host TEXT NOT NULL DEFAULT '',
    original_source_link TEXT NOT NULL DEFAULT '',
    include_comments BOOLEAN NOT NULL DEFAULT 0,
    include_timestamps BOOLEAN NOT NULL DEFAULT 0,
    include_glossary BOOLEAN NOT NULL DEFAULT 0,
    output_language TEXT NOT NULL DEFAULT '',
    summary TEXT NOT NULL DEFAULT '',
    summary_done BOOLEAN NOT NULL DEFAULT 0,
    summary_input_tokens INTEGER NOT NULL DEFAULT 0,
    summary_output_tokens INTEGER NOT NULL DEFAULT 0,
    summary_timestamp_start TEXT NOT NULL DEFAULT '',
    summary_timestamp_end TEXT NOT NULL DEFAULT '',
    timestamps TEXT NOT NULL DEFAULT '',
    timestamps_done BOOLEAN NOT NULL DEFAULT 0,
    timestamps_input_tokens INTEGER NOT NULL DEFAULT 0,
    timestamps_output_tokens INTEGER NOT NULL DEFAULT 0,
    timestamps_timestamp_start TEXT NOT NULL DEFAULT '',
    timestamps_timestamp_end TEXT NOT NULL DEFAULT '',
    timestamped_summary_in_youtube_format TEXT NOT NULL DEFAULT '',
    cost REAL NOT NULL DEFAULT 0.0,
    embedding BLOB,
    embedding_model TEXT NOT NULL DEFAULT '',
    full_embedding BLOB,
    google_search_grounding BOOLEAN NOT NULL DEFAULT 0,
    url_context BOOLEAN NOT NULL DEFAULT 0,
    thinking TEXT NOT NULL DEFAULT '',
    thinking_tokens INTEGER NOT NULL DEFAULT 0,
    thinking_level TEXT NOT NULL DEFAULT 'high'
        CHECK (thinking_level IN ('auto', 'minimal', 'low', 'medium', 'high', 'xhigh')),
    generation_status TEXT NOT NULL DEFAULT 'queued',
    generation_attempt INTEGER NOT NULL DEFAULT 0,
    generation_epoch INTEGER NOT NULL DEFAULT 0,
    generation_started_at TEXT NOT NULL DEFAULT '',
    generation_updated_at TEXT NOT NULL DEFAULT '',
    next_retry_at TEXT NOT NULL DEFAULT '',
    generation_error_code TEXT NOT NULL DEFAULT '',
    generation_error_message TEXT NOT NULL DEFAULT '',
    provider_interaction_id TEXT NOT NULL DEFAULT '',
    rs_summarizer_version TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL DEFAULT ''
);

-- Legacy production rows predate several NOT NULL constraints and carry
-- NULLs (see SUMMARY_COLUMNS in src/db.rs); COALESCE keeps them readable.
INSERT INTO summaries_new (
    identifier, model, transcript, host, original_source_link,
    include_comments, include_timestamps, include_glossary,
    output_language, summary, summary_done,
    summary_input_tokens, summary_output_tokens,
    summary_timestamp_start, summary_timestamp_end,
    timestamps, timestamps_done,
    timestamps_input_tokens, timestamps_output_tokens,
    timestamps_timestamp_start, timestamps_timestamp_end,
    timestamped_summary_in_youtube_format, cost,
    embedding, embedding_model, full_embedding,
    google_search_grounding, url_context, thinking, thinking_tokens,
    thinking_level, generation_status, generation_attempt, generation_epoch,
    generation_started_at, generation_updated_at, next_retry_at,
    generation_error_code, generation_error_message,
    provider_interaction_id, rs_summarizer_version, title
) SELECT
    identifier,
    COALESCE(model, ''), COALESCE(transcript, ''), COALESCE(host, ''),
    COALESCE(original_source_link, ''),
    COALESCE(include_comments, 0), COALESCE(include_timestamps, 0),
    COALESCE(include_glossary, 0), COALESCE(output_language, ''),
    COALESCE(summary, ''), COALESCE(summary_done, 0),
    COALESCE(summary_input_tokens, 0), COALESCE(summary_output_tokens, 0),
    COALESCE(summary_timestamp_start, ''), COALESCE(summary_timestamp_end, ''),
    COALESCE(timestamps, ''), COALESCE(timestamps_done, 0),
    COALESCE(timestamps_input_tokens, 0), COALESCE(timestamps_output_tokens, 0),
    COALESCE(timestamps_timestamp_start, ''),
    COALESCE(timestamps_timestamp_end, ''),
    COALESCE(timestamped_summary_in_youtube_format, ''),
    COALESCE(cost, 0.0),
    embedding, COALESCE(embedding_model, ''), full_embedding,
    COALESCE(google_search_grounding, 0), COALESCE(url_context, 0),
    COALESCE(thinking, ''), COALESCE(thinking_tokens, 0),
    COALESCE(thinking_level, 'high'),
    COALESCE(generation_status, 'queued'), COALESCE(generation_attempt, 0),
    COALESCE(generation_epoch, 0), COALESCE(generation_started_at, ''),
    COALESCE(generation_updated_at, ''), COALESCE(next_retry_at, ''),
    COALESCE(generation_error_code, ''),
    COALESCE(generation_error_message, ''),
    COALESCE(provider_interaction_id, ''),
    COALESCE(rs_summarizer_version, ''), COALESCE(title, '')
FROM summaries;

DROP TABLE summaries;

ALTER TABLE summaries_new RENAME TO summaries;

CREATE INDEX IF NOT EXISTS idx_dedup_lookup
    ON summaries (original_source_link, model, summary_timestamp_start);
CREATE INDEX IF NOT EXISTS idx_summaries_model_time
    ON summaries (model, summary_timestamp_start);
CREATE INDEX IF NOT EXISTS idx_summaries_lang_glossary
    ON summaries (output_language, include_glossary);
CREATE INDEX IF NOT EXISTS idx_summaries_generation_recovery
    ON summaries (generation_status, next_retry_at, generation_updated_at);
CREATE INDEX IF NOT EXISTS idx_summaries_title_url
    ON summaries (title, original_source_link);

-- Persistent daily per-model RPD counters: restarts stop resetting quotas.
CREATE TABLE IF NOT EXISTS model_daily_usage (
    day TEXT NOT NULL,
    model TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (day, model)
);

COMMIT;

PRAGMA foreign_key_check;

PRAGMA foreign_keys = ON;
