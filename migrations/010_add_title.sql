-- Titles for YouTube videos and Hacker News posts, plus restart-proof
-- daily fetch budgets for the title backfill worker.
ALTER TABLE summaries ADD COLUMN title TEXT NOT NULL DEFAULT '';

CREATE TABLE IF NOT EXISTS title_fetch_budget (
    day TEXT NOT NULL,
    source TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (day, source)
);
