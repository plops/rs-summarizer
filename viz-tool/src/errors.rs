use thiserror::Error;

#[derive(Error, Debug)]
pub enum VizError {
    #[error("Datenbankfehler: {0}")]
    Database(#[from] sqlx::Error),

    #[error("E/A-Fehler: {0}")]
    Io(#[from] std::io::Error),

    #[error("UMAP-Berechnungsfehler: {0}")]
    Umap(String),

    #[error("DBSCAN-Clustering-Fehler: {0}")]
    Dbscan(String),

    #[error("API-Fehler: {0}")]
    Api(String),

    #[error("Dimensionsfehler: Erwartet {expected}, erhalten {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Ungültige BLOB-Länge: {length} Bytes (kein Vielfaches von 4)")]
    InvalidBlobLength { length: usize },

    #[error("BLOB zu kurz: {actual} Bytes, mindestens {required} Bytes benötigt")]
    BlobTooShort { actual: usize, required: usize },

    #[error("Modell-Ladefehler: {0}")]
    ModelLoadError(String),

    #[error("Keine Embeddings in den Daten gefunden")]
    NoEmbeddings,

    #[error("UMAP wurde noch nicht berechnet")]
    UmapNotComputed,

    #[error("Nicht genügend Punkte für UMAP: {actual} Punkte, mindestens {required} benötigt")]
    InsufficientPoints { actual: usize, required: usize },
}
