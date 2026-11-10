//! One error type for the whole engine.

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("storage: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("tags: {0}")]
    Tag(#[from] weaver::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// The app's `Schema` itself is malformed, or the database is incompatible with it.
    #[error("schema: {0}")]
    Schema(String),
    /// The caller supplied something the schema or the data model rejects.
    #[error("invalid: {0}")]
    Invalid(String),
    #[error("item {0} not found")]
    NotFound(i64),
}

pub(crate) fn invalid<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Invalid(msg.into()))
}
