//! Error type definitions for the system
use thiserror::Error;
use std::time::Duration;

#[derive(Error, Debug)]
pub enum VectorDBError {
    #[error("Shard operation failed: {0}")]
    ShardError(String),

    #[error("Vector compression failed: {0}")]
    CompressionError(String),

    #[error("Cache operation failed: {0}")]
    CacheError(String),
}

#[derive(Error, Debug)]
pub enum FederatedLearningError {
    #[error("Model validation failed: {0}")]
    ValidationError(String),

    #[error("Aggregation failed: {0}")]
    AggregationError(String),

    #[error("Synchronization failed: {0}")]
    SyncError(String),
}

#[derive(Error, Debug)]
pub enum KnowledgeError {
    #[error("Knowledge representation failed: {0}")]
    RepresentationError(String),

    #[error("Knowledge transfer failed: {0}")]
    TransferError(String),
}