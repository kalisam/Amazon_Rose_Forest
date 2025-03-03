//! Circuit breaker implementation for fault tolerance
//! Prevents cascading failures by stopping operations when errors occur

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use rand::{thread_rng, Rng};
use thiserror::Error;
use crate::core::sharding::hilbert::CircuitState;

/// Error types for circuit breaker operations
#[derive(Debug, Error)]
pub enum CircuitError {
    #[error("Circuit is open")]
    CircuitOpen,

    #[error("Failed to acquire lock: {0}")]
    LockError(String),

    #[error("Invalid state transition: {0}")]
    InvalidStateTransition(String),
}

/// Configuration for the circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Failure threshold to open the circuit
    pub failure_threshold: u32,
    /// Success threshold to close the circuit from half-open state
    pub success_threshold: u32,
    /// Maximum attempts in half-open state
    pub max_half_open_attempts: u32,
    /// Timeout before transitioning from open to half-open
    pub reset_timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            max_half_open_attempts: 10,
            reset_timeout: Duration::from_secs(30),
        }
    }
}

/// Metrics for circuit breaker monitoring
#[derive(Debug, Default)]
pub struct CircuitMetrics {
    /// Count of successful operations
    pub success_count: u64,
    /// Count of failed operations
    pub failure_count: u64,
    /// Count of circuit open events
    pub open_count: u64,
    /// Count of circuit half-open events
    pub half_open_count: u64,
    /// Count of circuit closed events
    pub closed_count: u64,
    /// Timestamp of last state change
    pub last_state_change: Option<Instant>,
}

/// Circuit breaker for fault tolerance
pub struct CircuitBreaker {
    /// Current state of the circuit breaker
    state: Arc<RwLock<CircuitState>>,
    /// Configuration parameters
    config: CircuitBreakerConfig,
    /// Metrics for monitoring
    metrics: Arc<RwLock<CircuitMetrics>>,
    /// Recent failure count (reset on success)
    recent_failures: Arc<RwLock<u32>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the specified configuration
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            config,
            metrics: Arc::new(RwLock::new(CircuitMetrics::default())),
            recent_failures: Arc::new(RwLock::new(0)),
        }
    }

    /// Check if an operation is allowed based on the current circuit state
    pub fn allow_operation(&self) -> Result<bool, CircuitError> {
        let state = self.state.read().map_err(|e| CircuitError::LockError(e.to_string()))?;

        match *state {
            CircuitState::Closed => Ok(true),
            CircuitState::Open { since } => {
                if since.elapsed() > self.config.reset_timeout {
                    // Time to try half-open
                    drop(state);
                    self.transition_to_half_open()?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            CircuitState::HalfOpen { attempts } => {
                Ok(attempts < self.config.max_half_open_attempts)
            }
        }
    }

    /// Record the result of an operation (success or failure)
    pub fn record_result(&self, success: bool) -> Result<(), CircuitError> {
        if success {
            self.record_success()
        } else {
            self.record_failure()
        }
    }

    /// Record a successful operation
    pub fn record_success(&self) -> Result<(), CircuitError> {
        // Update metrics
        {
            let mut metrics = self.metrics.write().map_err(|e| CircuitError::LockError(e.to_string()))?;
            metrics.success_count += 1;
        }

        // Reset failure counter
        {
            let mut failures = self.recent_failures.write().map_err(|e| CircuitError::LockError(e.to_string()))?;
            *failures = 0;
        }

        // Update state if needed
        let mut state = self.state.write().map_err(|e| CircuitError::LockError(e.to_string()))?;

        match *state {
            CircuitState::HalfOpen { ref mut attempts } => {
                *attempts += 1;
                if *attempts >= self.config.success_threshold {
                    // Transition to closed
                    *state = CircuitState::Closed;

                    // Update metrics
                    let mut metrics = self.metrics.write().map_err(|e| CircuitError::LockError(e.to_string()))?;
                    metrics.closed_count += 1;
                    metrics.last_state_change = Some(Instant::now());
                }
            }
            _ => {} // No state change for other states
        }

        Ok(())
    }

    /// Record a failed operation
    pub fn record_failure(&self) -> Result<(), CircuitError> {
        // Update metrics
        {
            let mut metrics = self.metrics.write().map_err(|e| CircuitError::LockError(e.to_string()))?;
            metrics.failure_count += 1;
        }

        // Update state based on current state
        let mut state = self.state.write().map_err(|e| CircuitError::LockError(e.to_string()))?;

        match *state {
            CircuitState::Closed => {
                // Increment failure counter
                let mut failures = self.recent_failures.write().map_err(|e| CircuitError::LockError(e.to_string()))?;
                *failures += 1;

                // Check if we need to open the circuit
                if *failures >= self.config.failure_threshold {
                    *state = CircuitState::Open { since: Instant::now() };

                    // Update metrics
                    let mut metrics = self.metrics.write().map_err(|e| CircuitError::LockError(e.to_string()))?;
                    metrics.open_count += 1;
                    metrics.last_state_change = Some(Instant::now());
                }
            }
            CircuitState::HalfOpen { .. } => {
                // Any failure in half-open state opens the circuit
                *state = CircuitState::Open { since: Instant::now() };

                // Update metrics
                let mut metrics = self.metrics.write().map_err(|e| CircuitError::LockError(e.to_string()))?;
                metrics.open_count += 1;
                metrics.last_state_change = Some(Instant::now());
            }
            _ => {} // No state change for other states
        }

        Ok(())
    }

    /// Transition to half-open state
    fn transition_to_half_open(&self) -> Result<(), CircuitError> {
        let mut state = self.state.write().map_err(|e| CircuitError::LockError(e.to_string()))?;

        match *state {
            CircuitState::Open { .. } => {
                *state = CircuitState::HalfOpen { attempts: 0 };

                // Update metrics
                let mut metrics = self.metrics.write().map_err(|e| CircuitError::LockError(e.to_string()))?;
                metrics.half_open_count += 1;
                metrics.last_state_change = Some(Instant::now());

                Ok(())
            }
            _ => Err(CircuitError::InvalidStateTransition(
                "Can only transition to half-open from open state".to_string()
            )),
        }
    }

    /// Get current circuit breaker metrics
    pub fn get_metrics(&self) -> Result<CircuitMetrics, CircuitError> {
        let metrics = self.metrics.read().map_err(|e| CircuitError::LockError(e.to_string()))?;
        Ok(metrics.clone())
    }

    /// Get current circuit state
    pub fn get_state(&self) -> Result<CircuitState, CircuitError> {
        let state = self.state.read().map_err(|e| CircuitError::LockError(e.to_string()))?;
        Ok(state.clone())
    }
}