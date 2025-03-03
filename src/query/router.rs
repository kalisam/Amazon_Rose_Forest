//! Query routing system for efficient vector search
//! Implements caching and health-aware node selection

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use lru::LruCache;
use serde::{Serialize, Deserialize};
use crate::metrics::collector::MetricsCollector;

/// A query for vector search
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Query {
    /// The vector to search for
    pub vector: Vec<f32>,
    /// Maximum number of results to return
    pub limit: usize,
    /// Minimum similarity threshold
    pub threshold: f32,
    /// Additional filters
    pub filters: HashMap<String, String>,
}

impl Query {
    /// Compute a hash of the query for caching
    pub fn hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.vector.hash(&mut hasher);
        self.limit.hash(&mut hasher);
        (self.threshold * 1000.0) as u32.hash(&mut hasher);
        self.filters.hash(&mut hasher);

        hasher.finish()
    }
}

/// A search result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    /// The vector ID
    pub id: String,
    /// The similarity score
    pub score: f32,
    /// The vector data
    pub vector: Vec<f32>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Node health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealth {
    /// CPU usage (0.0 to 1.0)
    pub cpu_usage: f32,
    /// Memory usage (0.0 to 1.0)
    pub memory_usage: f32,
    /// Average query latency in milliseconds
    pub avg_latency: f32,
    /// Error rate (0.0 to 1.0)
    pub error_rate: f32,
    /// Last update timestamp
    pub last_update: u64,
}

impl NodeHealth {
    /// Calculate a health score (higher is better)
    pub fn score(&self) -> f32 {
        let cpu_score = 1.0 - self.cpu_usage;
        let memory_score = 1.0 - self.memory_usage;
        let latency_score = 1.0 / (1.0 + (self.avg_latency / 1000.0));
        let error_score = 1.0 - self.error_rate;

        // Weighted average
        (cpu_score * 0.3) + (memory_score * 0.3) + (latency_score * 0.2) + (error_score * 0.2)
    }

    /// Check if the node is healthy
    pub fn is_healthy(&self) -> bool {
        self.score() > 0.5
    }
}

/// A node in the network
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

/// Configuration for the query router
#[derive(Debug, Clone)]
pub struct QueryRouterConfig {
    /// Size of the result cache
    pub cache_size: usize,
    /// Cache TTL in seconds
    pub cache_ttl: u64,
    /// Number of nodes to query in parallel
    pub parallel_queries: usize,
    /// Query timeout in milliseconds
    pub query_timeout: u64,
    /// Retry count for failed queries
    pub retry_count: usize,
}

impl Default for QueryRouterConfig {
    fn default() -> Self {
        Self {
            cache_size: 1000,
            cache_ttl: 300, // 5 minutes
            parallel_queries: 3,
            query_timeout: 5000, // 5 seconds
            retry_count: 2,
        }
    }
}

/// A cached search result with expiration
struct CachedResult {
    /// The search results
    results: Vec<SearchResult>,
    /// When the cache entry expires
    expires_at: Instant,
}

/// Query router for efficient vector search
pub struct QueryRouter {
    /// Configuration for the router
    config: QueryRouterConfig,
    /// Cache for search results
    cache: Arc<RwLock<LruCache<u64, CachedResult>>>,
    /// Health metrics for nodes
    node_health: Arc<RwLock<HashMap<NodeId, NodeHealth>>>,
    /// Metrics collector
    metrics: Arc<MetricsCollector>,
}

impl QueryRouter {
    /// Create a new query router with the given configuration
    pub fn new(config: QueryRouterConfig, metrics: Arc<MetricsCollector>) -> Self {
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(config.cache_size))),
            node_health: Arc::new(RwLock::new(HashMap::new())),
            metrics,
            config,
        }
    }

    /// Route a query to the appropriate nodes and return results
    pub async fn route_query(&self, query: Query) -> Result<Vec<SearchResult>, String> {
        // Check cache first
        let query_hash = query.hash();
        if let Some(cached) = self.get_cached_result(query_hash) {
            self.metrics.increment_counter("query.cache.hit", 1.0);
            return Ok(cached);
        }
        self.metrics.increment_counter("query.cache.miss", 1.0);

        // Find candidate nodes
        let candidate_nodes = self.find_candidate_nodes(&query).await?;

        // Execute parallel queries
        let results = self.execute_parallel_query(query.clone(), candidate_nodes).await?;

        // Cache results
        self.cache_results(query_hash, results.clone());

        Ok(results)
    }

    /// Find candidate nodes for a query
    async fn find_candidate_nodes(&self, query: &Query) -> Result<Vec<NodeId>, String> {
        // This would normally use LSH or other techniques to find relevant nodes
        // For now, we'll just return all healthy nodes

        let node_health = self.node_health.read().map_err(|e| e.to_string())?;

        let healthy_nodes: Vec<NodeId> = node_health.iter()
            .filter(|(_, health)| health.is_healthy())
            .map(|(id, _)| id.clone())
            .collect();

        if healthy_nodes.is_empty() {
            return Err("No healthy nodes available".to_string());
        }

        // Record metrics
        self.metrics.set_gauge("query.candidate_nodes", healthy_nodes.len() as f64);

        Ok(healthy_nodes)
    }

    /// Execute a query in parallel across multiple nodes
    async fn execute_parallel_query(&self, query: Query, nodes: Vec<NodeId>) -> Result<Vec<SearchResult>, String> {
        // In a real implementation, this would send the query to multiple nodes in parallel
        // For now, we'll just simulate results

        // Record start time for latency measurement
        let start = Instant::now();

        // Simulate query execution
        let mut results = Vec::new();
        for i in 0..5 {
            results.push(SearchResult {
                id: format!("result_{}", i),
                score: 0.9 - (i as f32 * 0.1),
                vector: query.vector.clone(),
                metadata: HashMap::new(),
            });
        }

        // Record metrics
        let duration = start.elapsed();
        self.metrics.record_histogram("query.latency", duration.as_millis() as f64, None);
        self.metrics.increment_counter("query.count", 1.0);

        Ok(results)
    }

    /// Update health metrics for a node
    pub fn update_node_health(&self, node: NodeId, health: NodeHealth) -> Result<(), String> {
        let mut node_health = self.node_health.write().map_err(|e| e.to_string())?;
        node_health.insert(node, health);
        Ok(())
    }

    /// Get a cached result if available and not expired
    fn get_cached_result(&self, query_hash: u64) -> Option<Vec<SearchResult>> {
        let cache = self.cache.read().ok()?;

        if let Some(cached) = cache.peek(&query_hash) {
            if cached.expires_at > Instant::now() {
                return Some(cached.results.clone());
            }
        }

        None
    }

    /// Cache query results
    fn cache_results(&self, query_hash: u64, results: Vec<SearchResult>) {
        if let Ok(mut cache) = self.cache.write() {
            let cached = CachedResult {
                results,
                expires_at: Instant::now() + Duration::from_secs(self.config.cache_ttl),
            };

            cache.put(query_hash, cached);
        }
    }

    /// Clear the cache
    pub fn clear_cache(&self) -> Result<(), String> {
        let mut cache = self.cache.write().map_err(|e| e.to_string())?;
        cache.clear();
        Ok(())
    }
}