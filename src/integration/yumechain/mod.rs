//! YumeiCHAIN integration for AI knowledge exchange
//!
//! YumeiCHAIN provides a standardized framework for AI-to-AI knowledge sharing
//! with structured reasoning, confidence scoring, and trust mechanisms.

pub mod schema;
pub mod client;
pub mod converter;

// Re-export commonly used types
pub use schema::KnowledgePackage;
pub use client::YumeiChainClient;
pub use converter::KnowledgeConverter;