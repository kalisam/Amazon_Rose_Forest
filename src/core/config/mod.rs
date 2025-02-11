//! Configuration management for the system
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub vector_db: VectorDBConfig,
    pub federated: FederatedConfig,
    pub knowledge: KnowledgeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorDBConfig {
    pub shard_size: usize,
    pub replication_factor: u8,
    pub cache_size: usize,
    pub compression_threshold: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederatedConfig {
    pub min_participants: usize,
    pub aggregation_timeout: Duration,
    pub sync_interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeConfig {
    pub vector_dimensions: usize,
    pub similarity_threshold: f32,
    pub transfer_batch_size: usize,
}