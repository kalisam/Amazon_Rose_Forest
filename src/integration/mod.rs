//! Integration with external systems and protocols
//!
//! This module provides integration points with other AI knowledge systems
//! and protocols for interoperability.

pub mod yumechain;

// Re-export commonly used types
pub use yumechain::client::YumeiChainClient;
pub use yumechain::schema::KnowledgePackage;