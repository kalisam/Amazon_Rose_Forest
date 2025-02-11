//! Error handling for the entire system
use thiserror::Error;
use std::time::Duration;

#[derive(Error, Debug)]
pub enum SystemError {
    #[error("DHT operation failed: {0}")]
    DHTError(String),

    #[error("Vector database error: {0}")]
    VectorDBError(String),

    #[error("Migration failed: {context}")]
    MigrationError {
        context: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Operation timed out after {duration:?}")]
    Timeout {
        duration: Duration,
        operation: String,
    },
}