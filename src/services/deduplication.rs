use sqlx::SqlitePool;
use std::time::Duration;
use chrono::{TimeDelta, Utc};

pub struct DeduplicationService {
    window: Duration, // default: 5 minutes
}

impl DeduplicationService {
    pub fn new(window: Duration) -> Self {
        Self { window }
    }

    /// Check if a duplicate submission exists by URL + model within the time window.
    pub async fn check_duplicate(
        &self,
        db: &SqlitePool,
        url: &str,
        model: &str,
    ) -> Result<Option<i64>, sqlx::Error> {
        let delta = TimeDelta::from_std(self.window).unwrap();
        let cutoff = (Utc::now() - delta).to_rfc3339();

        let row = sqlx::query_scalar::<_, i64>(
            "SELECT identifier FROM summaries \
             WHERE original_source_link = ? AND model = ? AND summary_timestamp_start > ? \
             ORDER BY identifier DESC LIMIT 1",
        )
        .bind(url.trim())
        .bind(model)
        .bind(&cutoff)
        .fetch_optional(db)
        .await?;

        Ok(row)
    }

    /// Check if a duplicate submission exists by transcript + model within the time window.
    pub async fn check_duplicate_by_transcript(
        &self,
        db: &SqlitePool,
        transcript: &str,
        model: &str,
    ) -> Result<Option<i64>, sqlx::Error> {
        let delta = TimeDelta::from_std(self.window).unwrap();
        let cutoff = (Utc::now() - delta).to_rfc3339();

        let row = sqlx::query_scalar::<_, i64>(
            "SELECT identifier FROM summaries \
             WHERE transcript = ? AND model = ? AND summary_timestamp_start > ? \
             ORDER BY identifier DESC LIMIT 1",
        )
        .bind(transcript.trim())
        .bind(model)
        .bind(&cutoff)
        .fetch_optional(db)
        .await?;

        Ok(row)
    }
}
