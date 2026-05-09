use thiserror::Error;

#[derive(Debug, Error)]
pub enum TranscriptError {
    #[error("Invalid YouTube URL: {0}")]
    InvalidUrl(String),
    #[error("No subtitles available for this video")]
    NoSubtitles,
    #[error("yt-dlp execution failed: {0}")]
    YtDlpFailed(String),
    #[error("Download timeout after {0}s")]
    Timeout(u64),
    #[error("VTT parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, Error)]
pub enum SummaryError {
    #[error("Gemini API error: {0}")]
    ApiError(String),
    #[error("Resource exhausted - rate limited")]
    RateLimited,
    #[error("Transcript too short (< 30 words)")]
    TranscriptTooShort,
    #[error("Transcript too long ({0} words, max {1})")]
    TranscriptTooLong(usize, usize),
}

#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error("Embedding API error: {0}")]
    ApiError(String),
    #[error("Empty text provided")]
    EmptyText,
    #[error("Database error: {0}")]
    DbError(#[from] sqlx::Error),
}

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("Row not found after max retries")]
    RowNotFound,
    #[error("Transcript too short")]
    TranscriptTooShort,
    #[error("Transcript too long ({0} words)")]
    TranscriptTooLong(usize),
    #[error("Transcript error: {0}")]
    Transcript(#[from] TranscriptError),
    #[error("Summary error: {0}")]
    Summary(#[from] SummaryError),
    #[error("Embedding error: {0}")]
    Embedding(#[from] EmbeddingError),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("Source file not found: {0}")]
    SourceNotFound(std::path::PathBuf),

    #[error("Output file already exists: {0}")]
    OutputExists(std::path::PathBuf),

    #[error("Output directory does not exist: {0}")]
    OutputDirMissing(std::path::PathBuf),

    #[error("No qualifying rows found (embedding IS NOT NULL AND summary_done = 1)")]
    NoQualifyingRows,

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
