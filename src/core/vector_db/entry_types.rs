//! Vector database entry types
use hdk::prelude::*;

#[hdk_entry(id = "vector")]
#[derive(Clone)]
pub struct VectorEntry {
    pub vector_data: Vec<u8>,  // Compressed vector data
    pub metadata: VectorMetadata,
    pub timestamp: Timestamp,
}

#[hdk_entry(id = "centroid")]
#[derive(Clone)]
pub struct CentroidEntry {
    pub centroid: Vec<f32>,
    pub level: u8,  // Hierarchy level (0 for global, 1 for local)
    pub cluster_size: u32,
    pub version: VersionVector,
    pub responsible_agents: BTreeSet<AgentPubKey>,
}

#[hdk_entry(id = "node_metadata")]
#[derive(Clone)]
pub struct NodeMetadataEntry {
    pub health_metrics: HealthMetrics,
    pub vector_count: u32,
    pub last_heartbeat: Timestamp,
    pub capabilities: NodeCapabilities,
}