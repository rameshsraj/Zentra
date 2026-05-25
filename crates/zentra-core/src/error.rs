use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Plugin error: {plugin_id} — {message}")]
    Plugin { plugin_id: String, message: String },

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Ingestion error: {0}")]
    Ingestion(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn plugin(plugin_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Plugin {
            plugin_id: plugin_id.into(),
            message: message.into(),
        }
    }
}

// Convert from sqlx errors
impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        Error::Database(e.to_string())
    }
}
