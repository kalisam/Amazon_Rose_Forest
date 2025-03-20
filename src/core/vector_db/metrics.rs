//! Vector database metrics collection
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ShardMetrics {
    pub load: u32,
    pub latency: u32,
    pub failures: u32,
}

#[derive(Debug, Clone)]
pub struct HealthMetrics {
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub network_latency: u32,
    pub error_rate: f32,
}

#[derive(Debug, Clone)]
pub struct NodeCapabilities {
    pub max_vectors: u32,
    pub max_memory: u64,
    pub supported_operations: Vec<String>,
}