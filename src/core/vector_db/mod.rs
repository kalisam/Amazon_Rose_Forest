mod entry_types;
mod metrics;
mod sharding;

pub use entry_types::*;
pub use metrics::*;
pub use sharding::*;

// Re-export main components
pub use self::sharding::ShardManager;
pub use self::metrics::ShardMetrics;
// Define Holochain entry types
#[hdk_entry(id = "vector")]
#[derive(Clone)]
pub struct VectorEntry {
    vector_data: Vec<u8>,  // Compressed vector data
    metadata: VectorMetadata,
    timestamp: Timestamp,
}

#[hdk_entry(id = "centroid")]
#[derive(Clone)]
// Centroid structure used to represent clusters in the DHT for hierarchical sharding
pub struct CentroidEntry {
    centroid: Vec<f32>,
    level: u8,  // Hierarchy level (0 for global, 1 for local)
    cluster_size: u32,
    version: VersionVector,
    responsible_agents: BTreeSet<AgentPubKey>,
}

#[hdk_entry(id = "node_metadata")]
#[derive(Clone)]
pub struct NodeMetadataEntry {
    health_metrics: HealthMetrics,
    vector_count: u32,
    last_heartbeat: Timestamp,
    capabilities: NodeCapabilities,
}

// Define ShardMetrics for tracking shard performance metrics such as load, latency, and failure counts for monitoring and optimization
pub struct ShardMetrics {
    load: u32, // Tracks the current number of vectors or operations in the shard
    latency: u32, // Records the average time taken for operations on this shard
    failures: u32, // Counts the number of failed operations to monitor shard health
}

// Advanced sharding implementation with Hilbert curve-based partitioning
pub struct ShardManager {
    config: ShardConfig,
    circuit_breaker: CircuitBreaker,
    metrics: Arc<ShardMetrics>,
}

impl ShardManager {
    /// Handle shard split using Hilbert curve partitioning
    pub async fn handle_shard_split(&mut self, shard: Shard) -> Result<(), ShardError> {
        let hilbert = HilbertCurve::new(self.config.dimensions);

        // Calculate split points based on vector distribution
        let split_points = self.calculate_split_points(&shard, &hilbert)?;

        // Prepare migration plan
        let migration_plan = self.prepare_migration(split_points, &shard).await?;

        // Execute migration with circuit breaker pattern
        self.execute_migration(migration_plan).await
    }

    /// Execute migration with streaming and retry mechanisms
    pub async fn execute_migration(&self, plan: MigrationPlan) -> Result<(), ShardError> {
        let mut stream = StreamingMigration::new(plan);

        while let Some(batch) = stream.next_batch().await? {
            if !self.circuit_breaker.allow_operation()? {
                return Err(ShardError::CircuitBreakerOpen);
            }

            match self.transfer_batch(batch).await {
                Ok(_) => {
                    self.circuit_breaker.record_success();
                    continue;
                }
                Err(e) => {
                    self.circuit_breaker.record_failure();
                    if self.should_retry(&e) {
                        stream.retry_batch().await?;
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }
}

// Improved circuit breaker implementation
#[derive(Debug)]
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    config: CircuitBreakerConfig,
    metrics: Arc<ShardMetrics>,
}

impl CircuitBreaker {
    pub async fn allow_operation(&self) -> Result<bool, CircuitError> {
        let state = self.state.read().await;
        match *state {
            CircuitState::Closed => Ok(true),
            CircuitState::Open { since } => {
                if since.elapsed() > self.config.reset_timeout {
                    drop(state);
                    self.half_open().await?;
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

    pub async fn record_result(&self, success: bool) {
        let mut state = self.state.write().await;
        match *state {
            CircuitState::Closed => {
                if !success {
                    self.metrics.increment_failure();
                    if self.should_open() {
                        *state = CircuitState::Open { since: Instant::now() };
                    }
                }
            }
            CircuitState::HalfOpen { ref mut attempts } => {
                if success {
                    *attempts += 1;
                    if *attempts >= self.config.success_threshold {
                        *state = CircuitState::Closed;
                    }
                } else {
                    *state = CircuitState::Open { since: Instant::now() };
                }
            }
            _ => {}
        }
    }
}

// Advanced retry logic with decorrelated jitter
pub struct RetryStrategy {
    base: Duration,
    cap: Duration,
    attempts: u32,
    rng: ThreadRng,
}

impl RetryStrategy {
    pub fn next_delay(&mut self) -> Duration {
        let temp = min(self.cap, self.base * 2u32.pow(self.attempts));
        // Introduces a delay between retries using decorrelated jitter to prevent synchronized retry storms
        Duration::from_millis(
            self.rng.gen_range(self.base.as_millis() as u64..=temp.as_millis() as u64)
        )
    }
}

// Improved error handling with context
#[derive(Debug, thiserror::Error)]
pub enum ShardError {
    #[error("Shard migration failed: {context}")]
    MigrationFailed {
        context: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Circuit breaker is open")]
    CircuitBreakerOpen,

    #[error("Operation timed out after {duration:?}")]
    Timeout {
        duration: Duration,
        operation: String,
    },
}

// Enhanced logging with context
pub struct ContextualLogger {
    logger: Logger,
    context: HashMap<String, String>,
}

impl ContextualLogger {
    pub fn log_error(&self, error: &ShardError, attempt: u32) {
        let mut fields = self.context.clone();
        fields.insert("attempt".into(), attempt.to_string());
        fields.insert("error".into(), error.to_string());

        self.logger.error("Operation failed", fields);
    }
}
// DHT operations manager
pub struct DHTManager {
    cache: Arc<RwLock<LruCache<EntryHash, Entry>>>,
    validation_rules: ValidationRules,
    shard_metrics: Arc<RwLock<HashMap<u8, ShardMetrics>>>,
    retry_strategy: RetryStrategy,
    circuit_breaker: CircuitBreaker,
}

impl DHTManager {
    /// Handle shard splits when shard size exceeds threshold
    pub async fn split_shard(&self, shard_id: u8) -> ExternResult<()> {
        let vectors = self.get_vectors_in_shard(shard_id)?;
        let (left, right) = vectors.split_at(vectors.len() / 2);

        let new_shard_id = self.create_new_shard()?;
        self.assign_vectors_to_shard(right, new_shard_id)?;
        self.update_routing_table(shard_id, new_shard_id)?;
        Ok(())
    }

    /// Handle shard merges for underutilized shards
    pub async fn merge_shards(&self, shard_a: u8, shard_b: u8) -> ExternResult<()> {
        let vectors_a = self.get_vectors_in_shard(shard_a)?;
        let vectors_b = self.get_vectors_in_shard(shard_b)?;

        // Combine all vectors into shard_a
        self.assign_vectors_to_shard(vectors_b, shard_a)?;
        self.update_routing_table(shard_a, shard_b)?;
        Ok(())
    }

    /// Redistribute shards during migrations
    pub async fn redistribute_shards(&self) -> ExternResult<()> {
        let overloaded_nodes = self.get_overloaded_nodes()?;

        for node in overloaded_nodes {
            let shard_ids = self.get_shards_for_node(&node)?;

            for shard_id in shard_ids {
                let new_node = self.find_optimal_node_for_shard(shard_id)?;
                self.migrate_shard(shard_id, &new_node).await?;
            }
        }
        Ok(())
    }

    /// Lazy load vectors on demand
    pub fn lazy_load_vector(&self, vector_id: &str) -> ExternResult<Vector> {
        let vector = self.query_vector_from_dht(vector_id)?;
        Ok(vector)
    }

    /// Implement exponential backoff with jitter for retries
    pub async fn safe_create_entry(&self, entry: &Entry) -> ExternResult<EntryHash> {
        for attempt in 0..self.retry_strategy.attempts {
            match create_entry(entry).await {
                Ok(hash) => return Ok(hash),
                Err(e) if self.is_transient_error(&e) => {
                    tokio::time::sleep(self.retry_strategy.next_delay()).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Err(anyhow!("Max retries exceeded"))
    }

    /// Log failures for monitoring
    pub fn log_failure(&self, error: &Error, context: &str) {
        log::error!("Failure in {}: {:?}", context, error);
    }

    /// Adaptive compression for vector data
    pub fn compress_vector_adaptively(&self, vector: &Vector) -> ExternResult<Vec<u8>> {
        if self.is_high_bandwidth() {
            Ok(vector.values.clone()) // No compression needed
        } else {
            self.compress_vector(&vector) // Apply compression
        }
    }
}
