use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::SqlitePool;

/// Metadata for a single summary entry (lightweight, no full text).
#[derive(Debug, Clone)]
pub struct SummaryMetadata {
    pub identifier: i64,
    pub model: String,
    pub cost: f64,
    pub original_source_link: String,
    pub summary_timestamp_start: String,
    pub summary_done: bool,
    pub has_embedding: bool,
    pub summary_preview: String, // first 200 chars of summary for grouping
}

/// In-memory cache of summary metadata for fast browse/filter operations.
#[derive(Clone)]
pub struct MetadataCache {
    entries: Arc<RwLock<Vec<SummaryMetadata>>>,
}

impl MetadataCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Load all summary metadata from the database at startup.
    pub async fn load_from_db(&self, db: &SqlitePool) -> Result<(), sqlx::Error> {
        let rows: Vec<(i64, String, f64, String, String, bool, Option<Vec<u8>>, String)> =
            sqlx::query_as(
                "SELECT identifier, model, cost, original_source_link, summary_timestamp_start, \
                 summary_done, embedding, substr(summary, 1, 200) \
                 FROM summaries ORDER BY identifier DESC",
            )
            .fetch_all(db)
            .await?;

        let entries: Vec<SummaryMetadata> = rows
            .into_iter()
            .map(|(id, model, cost, link, ts, done, emb, preview)| SummaryMetadata {
                identifier: id,
                model,
                cost,
                original_source_link: link,
                summary_timestamp_start: ts,
                summary_done: done,
                has_embedding: emb.is_some(),
                summary_preview: preview,
            })
            .collect();

        let mut cache = self.entries.write().await;
        *cache = entries;
        Ok(())
    }

    /// Refresh the cache by reloading from DB.
    pub async fn refresh(&self, db: &SqlitePool) -> Result<(), sqlx::Error> {
        self.load_from_db(db).await
    }

    /// Get all entries (for browse/filter).
    pub async fn get_all(&self) -> Vec<SummaryMetadata> {
        self.entries.read().await.clone()
    }

    /// Get a page of entries with pagination.
    /// Returns (page_entries, has_next_page).
    pub async fn get_browse_page(&self, page: u32, page_size: usize) -> (Vec<SummaryMetadata>, bool) {
        let entries = self.entries.read().await;
        let offset = (page as usize) * page_size;

        if offset >= entries.len() {
            return (Vec::new(), false);
        }

        let end = (offset + page_size).min(entries.len());
        let page_entries = entries[offset..end].to_vec();
        let has_next = end < entries.len();

        (page_entries, has_next)
    }

    /// Group consecutive entries with identical summaries.
    /// Returns groups where each group is a vec of entries with the same summary preview.
    /// Empty previews are never grouped together (each stands alone).
    pub fn group_duplicates(entries: &[SummaryMetadata]) -> Vec<Vec<&SummaryMetadata>> {
        let mut groups: Vec<Vec<&SummaryMetadata>> = Vec::new();

        for entry in entries {
            if let Some(last_group) = groups.last_mut() {
                if let Some(last_entry) = last_group.last() {
                    if last_entry.summary_preview == entry.summary_preview
                        && !entry.summary_preview.is_empty()
                    {
                        last_group.push(entry);
                        continue;
                    }
                }
            }
            groups.push(vec![entry]);
        }

        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: i64, preview: &str) -> SummaryMetadata {
        SummaryMetadata {
            identifier: id,
            model: "test-model".to_string(),
            cost: 0.01,
            original_source_link: format!("https://youtube.com/watch?v=test{:03}", id),
            summary_timestamp_start: "2024-01-01T00:00:00Z".to_string(),
            summary_done: true,
            has_embedding: false,
            summary_preview: preview.to_string(),
        }
    }

    #[test]
    fn test_group_duplicates_no_duplicates() {
        let entries = vec![
            make_entry(1, "Summary A"),
            make_entry(2, "Summary B"),
            make_entry(3, "Summary C"),
        ];
        let groups = MetadataCache::group_duplicates(&entries);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].len(), 1);
        assert_eq!(groups[1].len(), 1);
        assert_eq!(groups[2].len(), 1);
    }

    #[test]
    fn test_group_duplicates_consecutive() {
        let entries = vec![
            make_entry(1, "Summary A"),
            make_entry(2, "Summary A"),
            make_entry(3, "Summary A"),
            make_entry(4, "Summary B"),
        ];
        let groups = MetadataCache::group_duplicates(&entries);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 3);
        assert_eq!(groups[0][0].identifier, 1);
        assert_eq!(groups[0][2].identifier, 3);
        assert_eq!(groups[1].len(), 1);
        assert_eq!(groups[1][0].identifier, 4);
    }

    #[test]
    fn test_group_duplicates_non_consecutive_same_preview() {
        let entries = vec![
            make_entry(1, "Summary A"),
            make_entry(2, "Summary B"),
            make_entry(3, "Summary A"),
        ];
        let groups = MetadataCache::group_duplicates(&entries);
        // Non-consecutive duplicates are NOT grouped
        assert_eq!(groups.len(), 3);
    }

    #[test]
    fn test_group_duplicates_empty_preview_not_grouped() {
        let entries = vec![
            make_entry(1, ""),
            make_entry(2, ""),
            make_entry(3, "Summary A"),
        ];
        let groups = MetadataCache::group_duplicates(&entries);
        // Empty previews should each stand alone
        assert_eq!(groups.len(), 3);
    }

    #[test]
    fn test_group_duplicates_empty_input() {
        let entries: Vec<SummaryMetadata> = vec![];
        let groups = MetadataCache::group_duplicates(&entries);
        assert_eq!(groups.len(), 0);
    }

    #[tokio::test]
    async fn test_get_browse_page_basic() {
        let cache = MetadataCache::new();
        {
            let mut entries = cache.entries.write().await;
            for i in 0..25 {
                entries.push(make_entry(i, &format!("Summary {}", i)));
            }
        }

        let (page, has_next) = cache.get_browse_page(0, 20).await;
        assert_eq!(page.len(), 20);
        assert!(has_next);

        let (page, has_next) = cache.get_browse_page(1, 20).await;
        assert_eq!(page.len(), 5);
        assert!(!has_next);
    }

    #[tokio::test]
    async fn test_get_browse_page_beyond_end() {
        let cache = MetadataCache::new();
        {
            let mut entries = cache.entries.write().await;
            for i in 0..5 {
                entries.push(make_entry(i, &format!("Summary {}", i)));
            }
        }

        let (page, has_next) = cache.get_browse_page(1, 20).await;
        assert_eq!(page.len(), 0);
        assert!(!has_next);
    }

    #[tokio::test]
    async fn test_get_all() {
        let cache = MetadataCache::new();
        {
            let mut entries = cache.entries.write().await;
            entries.push(make_entry(1, "Test"));
            entries.push(make_entry(2, "Test 2"));
        }

        let all = cache.get_all().await;
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].identifier, 1);
        assert_eq!(all[1].identifier, 2);
    }
}
