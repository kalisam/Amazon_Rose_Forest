//! Amazon Rose Forest - A decentralized AI and knowledge sharing system
//!
//! This library implements a Free Open Source Singularity (FOSS) through
//! decentralized AI and collaborative knowledge sharing.

pub mod core;
pub mod federated;
pub mod knowledge;
pub mod metrics;
pub mod query;

// Re-export commonly used types
pub use core::config::SystemConfig;
pub use federated::model::ModelUpdate;
pub use knowledge::representation::Knowledge;
pub use metrics::collector::MetricsCollector;
pub use query::router::QueryRouter;

/// Initialize the system with the given configuration
pub fn init(config: SystemConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize components
    let metrics = std::sync::Arc::new(metrics::collector::MetricsCollector::new());

    // Log initialization
    metrics.increment_counter("system.init", 1.0);

    Ok(())
}