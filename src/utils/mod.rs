//! Common utility functions and types
use std::time::{Duration, Instant};

pub mod compression;
pub mod validation;
pub mod metrics;

pub fn exponential_backoff(attempt: u32, base: Duration) -> Duration {
    let backoff = base * 2u32.pow(attempt);
    let jitter = rand::random::<f32>() * backoff.as_secs_f32() * 0.1;
    Duration::from_secs_f32(backoff.as_secs_f32() + jitter)
}

pub fn calculate_metrics<T: AsRef<[f32]>>(vectors: &[T]) -> Vec<f32> {
    // Calculate statistical metrics for a collection of vectors
    todo!()
}