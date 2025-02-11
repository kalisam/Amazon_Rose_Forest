//! DHT routing and query handling
use super::*;
use crate::error::SystemError;

pub struct QueryRouter {
    cache: Arc<RwLock<LruCache<QueryHash, Vec<SearchResult>>>>,
    routing_strategy: RoutingStrategy,
    health_monitor: HealthMonitor,
}

impl QueryRouter {
    pub async fn route_query(&mut self, query: Query) -> Result<Vec<SearchResult>, SystemError> {
        // Check cache first
        if let Some(results) = self.cache.get(&query.hash()) {
            return Ok(results.clone());
        }

        // Find candidate nodes using LSH
        let candidates = self.find_candidate_nodes(&query).await?;

        // Execute parallel query with health-aware routing
        let results = self.execute_parallel_query(query, candidates).await?;

        // Update cache
        self.cache.put(query.hash(), results.clone());

        Ok(results)
    }
}