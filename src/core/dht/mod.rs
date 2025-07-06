//! DHT management and sharding implementation
use hdk::prelude::*;

mod routing;
mod migration;

pub use crate::core::sharding::ShardManager;
pub use routing::QueryRouter;
pub use migration::MigrationManager;

// Core DHT configuration
#[derive(Debug, Clone)]
pub struct DHTConfig {
    pub shard_size: usize,
    pub replication_factor: u8,
    pub migration_batch_size: usize,
}

// DHT metrics for monitoring
#[derive(Debug, Clone)]
pub struct DHTMetrics {
    pub total_vectors: usize,
    pub shard_count: usize,
    pub query_latency_ms: f64,
}