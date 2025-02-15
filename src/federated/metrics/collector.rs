//! Metrics collection for federated learning
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct LearningMetrics {
    pub loss: f32,
    pub accuracy: f32,
    pub training_time: Duration,
    pub model_size: usize,
}

pub struct MetricsCollector {
    metrics: Arc<Mutex<HashMap<String, Vec<LearningMetrics>>>>,
}

impl MetricsCollector {
    pub fn record(&self, agent_id: &str, metrics: LearningMetrics) {
        let mut data = self.metrics.lock().unwrap();
        data.entry(agent_id.to_string())
            .or_insert_with(Vec::new)
            .push(metrics);
    }

    pub fn get_agent_metrics(&self, agent_id: &str) -> Option<Vec<LearningMetrics>> {
        self.metrics.lock().unwrap()
            .get(agent_id)
            .cloned()
    }
}