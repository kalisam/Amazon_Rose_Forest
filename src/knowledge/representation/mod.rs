//! Knowledge representation and management
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Knowledge {
    pub id: String,
    pub content: Vec<u8>,
    pub metadata: KnowledgeMetadata,
    pub vectors: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeMetadata {
    pub timestamp: u64,
    pub source: String,
    pub confidence: f32,
    pub tags: Vec<String>,
}

pub trait KnowledgeProcessor {
    fn process(&self, knowledge: &Knowledge) -> Result<Vec<f32>, Box<dyn std::error::Error>>;
    fn combine(&self, vectors: &[Vec<f32>]) -> Result<Vec<f32>, Box<dyn std::error::Error>>;
}