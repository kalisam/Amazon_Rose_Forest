use hdk::prelude::*;

#[hdk_entry_helper]
#[derive(Clone)]
pub struct ModelUpdate {
    pub weights: Vec<f32>,
    pub bias: f32,
    pub version: u32,
    pub metadata: ModelMetadata,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModelMetadata {
    pub timestamp: u64,
    pub metrics: ModelMetrics,
    pub agent_id: AgentPubKey,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModelMetrics {
    pub loss: f32,
    pub accuracy: f32,
    pub samples_count: u32,
}