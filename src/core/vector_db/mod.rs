// Core vector database functionality
pub mod shard;
pub mod hilbert;
pub mod circuit_breaker;

// Re-export main components
pub use shard::ShardManager;
pub use hilbert::HilbertCurve;
pub use circuit_breaker::CircuitBreaker;