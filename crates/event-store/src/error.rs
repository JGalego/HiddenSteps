#[derive(Debug, thiserror::Error)]
pub enum EventStoreError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error(
        "could not open the encrypted store — wrong key, or the file is not a HiddenSteps database"
    )]
    InvalidKeyOrCorruptFile,

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("timestamp error: {0}")]
    Timestamp(String),

    #[error("stored value is not valid for its column: {0}")]
    InvalidStoredValue(String),
}
