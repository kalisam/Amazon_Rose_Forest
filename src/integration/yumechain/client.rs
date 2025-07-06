//! YumeiCHAIN API client for knowledge exchange
//!
//! This module provides a client for interacting with the YumeiCHAIN API
//! to publish, query, update, and evaluate knowledge.

use std::sync::Arc;
use reqwest::{Client, Response, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::json;
use thiserror::Error;
use std::time::{Duration, Instant};
use crate::metrics::collector::MetricsCollector;
use crate::core::fault::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use super::schema::{
    KnowledgePackage, KnowledgeQuery, KnowledgeEvaluation, ConflictResolution,
    ApiResponse, PublishResponse, QueryResponse, EvaluationResponse
};
use rate_limiter::{Quota, RateLimiter};

const MAX_REQUESTS_PER_MINUTE: u32 = 100;

/// Errors that can occur during YumeiCHAIN API operations
#[derive(Debug, Error)]
pub enum YumeiChainError {
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("API error: {status} - {message}")]
    ApiError {
        status: StatusCode,
        message: String,
    },

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Circuit breaker is open")]
    CircuitBreakerOpen,

    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    #[error("Retry limit exceeded: {0}")]
    RetryLimitExceeded(String),
  
    #[error("Client creation failed")]
    ClientCreationFailed,
}

/// Vote type for knowledge evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoteType {
    /// Upvote (agree with knowledge)
    Upvote,
    /// Downvote (disagree with knowledge)
    Downvote,
}

impl ToString for VoteType {
    fn to_string(&self) -> String {
        match self {
            VoteType::Upvote => "upvote".to_string(),
            VoteType::Downvote => "downvote".to_string(),
        }
    }
}

/// Resolution type for conflict resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolutionType {
    /// Accept the knowledge
    Accept,
    /// Reject the knowledge
    Reject,
    /// Merge conflicting knowledge
    Merge,
}

impl ToString for ResolutionType {
    fn to_string(&self) -> String {
        match self {
            ResolutionType::Accept => "accept".to_string(),
            ResolutionType::Reject => "reject".to_string(),
            ResolutionType::Merge => "merge".to_string(),
        }
    }

}

/// Configuration for the YumeiCHAIN client
#[derive(Debug, Clone)]
pub struct YumeiChainConfig {
    /// Base URL for the YumeiCHAIN API
    pub api_url: String,

    /// API key for authentication
    pub api_key: Option<String>,

    /// Node ID for this AI node
    pub node_id: String,

    /// Timeout for API requests in seconds
    pub timeout_seconds: u64,

    /// Maximum retries for failed requests
    pub max_retries: u32,

    /// Backoff strategy for retries (in milliseconds)
    pub retry_backoff_ms: u64,
    /// Circuit breaker configuration
    pub circuit_breaker_config: CircuitBreakerConfig,
}

impl Default for YumeiChainConfig {
    fn default() -> Self {
        Self {
            api_url: "http://localhost:8000".to_string(),
            api_key: None,
            node_id: "amazon-rose-forest".to_string(),
            timeout_seconds: 30,
            max_retries: 3,

            retry_backoff_ms: 500,
            circuit_breaker_config: CircuitBreakerConfig {
                failure_threshold: 5,
                success_threshold: 3,
                max_half_open_attempts: 10,
                reset_timeout: std::time::Duration::from_secs(60),
            },
        }
    }
}

/// Client for interacting with the YumeiCHAIN API
pub struct YumeiChainClient {
    /// HTTP client for making requests
    client: Client,

    /// Configuration for the client
    config: YumeiChainConfig,

    /// Circuit breaker for fault tolerance
    pub(crate) circuit_breaker: CircuitBreaker,

    /// Metrics collector
    metrics: Arc<MetricsCollector>,

    /// Rate limiter
    rate_limiter: RateLimiter,
}

impl YumeiChainClient {
    /// Create a new YumeiCHAIN client with the specified configuration
    pub fn new(config: YumeiChainConfig, metrics: Arc<MetricsCollector>) -> Result<Self, YumeiChainError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|_| YumeiChainError::ClientCreationFailed)?;

        Ok(Self {
            client,
            config,
            circuit_breaker: CircuitBreaker::new(config.circuit_breaker_config),
            metrics,
            rate_limiter: RateLimiter::direct(Quota::per_minute(non_zero!(MAX_REQUESTS_PER_MINUTE))),
        })
    }

    /// Unified response handler for API requests
    async fn handle_response<T: DeserializeOwned>(
        &self,
        response_result: Result<Response, reqwest::Error>,
        metric_name: &str,
        start_time: Instant,
    ) -> Result<T, YumeiChainError> {
        let duration = start_time.elapsed();
        self.metrics
            .record_histogram(&format!("{}.duration", metric_name), duration.as_millis() as f64, None);

        let response = response_result.map_err(YumeiChainError::NetworkError)?;

        if !response.status().is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            self.metrics
                .increment_counter(&format!("{}.error", metric_name), 1.0);
            return Err(YumeiChainError::ApiError { status: response.status(), message });
        }

        self.metrics
            .increment_counter(&format!("{}.success", metric_name), 1.0);
        let data = response.json::<T>().await.map_err(YumeiChainError::NetworkError)?;
        Ok(data)
    }

    /// Register this AI node with YumeiCHAIN
    pub async fn register_node(&self, public_key: &str) -> Result<(), YumeiChainError> {
        // Check circuit breaker
        if !self.circuit_breaker.allow_operation()? {
            return Err(YumeiChainError::CircuitBreakerOpen);
        }

        // Check rate limiter
        if !self.rate_limiter.acquire() {
            return Err(YumeiChainError::AuthenticationError("Rate limit exceeded".to_string()));
        }

        // Start metrics timer
        let start = std::time::Instant::now();

        // Make API request
        let url = format!("{}/register", self.config.api_url);
        let response = self.client
            .post(&url)
            .query(&[("node_id", &self.config.node_id), ("public_key", &public_key)])
            .send()
            .await;

        // Record metrics
        let duration = start_time.elapsed();
        self.metrics.record_histogram(&format!("{}.duration", metric_name), duration.as_millis() as f64, None);

        // Handle response
        match response_result {
            Ok(res) => {
                if res.status().is_success() {
                    self.circuit_breaker.record_success()?;
                    self.metrics.increment_counter(&format!("{}.success", metric_name), 1.0);

                    let json_result = res.json::<T>().await;
                    match json_result {
                        Ok(data) => Ok(data),
                        Err(e) => {
                            self.circuit_breaker.record_failure()?;
                            self.metrics.increment_counter(&format!("{}.error", metric_name), 1.0);
                            Err(YumeiChainError::SerializationError(serde_json::Error::custom(format!("Failed to parse response: {}", e))))
                        }
                    }
                } else {
                    self.circuit_breaker.record_failure()?;
                    self.metrics.increment_counter(&format!("{}.error", metric_name), 1.0);

                    let error_message = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                    Err(YumeiChainError::ApiError {
                        status: res.status(),
                        message: error_message,
                    })
                }
            }
            Err(e) => {
                self.circuit_breaker.record_failure()?;
                self.metrics.increment_counter(&format!("{}.error", metric_name), 1.0);

                Err(YumeiChainError::NetworkError(e))
            }
        }
    }

    /// Execute a request with retry logic for transient failures
    async fn execute_with_retry<T, F, Fut>(&self, operation: F, metric_name: &str) -> Result<T, YumeiChainError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<Response, reqwest::Error>>,
        T: DeserializeOwned,
    {
        // Check circuit breaker
        if !self.circuit_breaker.allow_operation()? {
            return Err(YumeiChainError::CircuitBreakerOpen);
        }

        // Start metrics timer
        let start = Instant::now();

        // Try the operation with retries for transient errors
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                // Apply backoff for retries
                let backoff = self.config.retry_backoff_ms * (1 << (attempt - 1));
                tokio::time::sleep(Duration::from_millis(backoff)).await;

                self.metrics.increment_counter(&format!("{}.retry", metric_name), 1.0);
            }

            match operation().await {
                Ok(response) => {
                    // Check if this is a retryable status code (5xx)
                    if response.status().is_server_error() && attempt < self.config.max_retries {
                        last_error = Some(YumeiChainError::ApiError {
                            status: response.status(),
                            message: format!("Server error (attempt {}/{})", attempt + 1, self.config.max_retries + 1),
                        });
                        continue;
                    }

                    // Process the response
                    return self.handle_response::<T>(Ok(response), metric_name, start).await;
                }
                Err(e) => {
                    // Check if this is a retryable error (timeout, connection reset)
                    if (e.is_timeout() || e.is_connect()) && attempt < self.config.max_retries {
                        last_error = Some(YumeiChainError::NetworkError(e));
                        continue;
                    }

                    // Process the error
                    return self.handle_response::<T>(Err(e), metric_name, start).await;
                }
            }
        }

        // If we get here, we've exhausted all retries
        Err(YumeiChainError::RetryLimitExceeded(format!(
            "Failed after {} attempts: {:?}",
            self.config.max_retries + 1,
            last_error.unwrap_or(YumeiChainError::NetworkError(reqwest::Error::from(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unknown error"
            ))))
        )))
    }

    /// Register this AI node with YumeiCHAIN
    pub async fn register_node(&self, public_key: &str) -> Result<(), YumeiChainError> {
        let url = format!("{}/register", self.config.api_url);
        let node_id = self.config.node_id.clone();
        let public_key_str = public_key.to_string();

        // Define the operation
        let operation = || async {
            self.client
                .post(&url)
                .query(&[("node_id", &node_id), ("public_key", &public_key_str)])
                .send()
                .await
        };

        // Execute with retry
        self.execute_with_retry::<serde_json::Value, _, _>(operation, "yumechain.register").await?;

        Ok(())
    }

    /// Publish a knowledge package to YumeiCHAIN
    pub async fn publish_knowledge(&self, mut knowledge: KnowledgePackage) -> Result<PublishResponse, YumeiChainError> {
        // Check rate limiter
        if !self.rate_limiter.acquire() {
            return Err(YumeiChainError::AuthenticationError("Rate limit exceeded".to_string()));
        }

        // Ensure knowledge has IDs
        knowledge.ensure_ids();

        // Set the generating node if not already set
        if knowledge.content.generated_by.is_empty() {
            knowledge.content.generated_by = self.config.node_id.clone();
        }

        let url = format!("{}/knowledge", self.config.api_url);
        let api_key = self.config.api_key.clone();
        let knowledge_clone = knowledge.clone();

        // Define the operation
        let operation = || async {
            let mut request = self.client.post(&url);

            // Add authorization if available
            if let Some(key) = &api_key {
                request = request.header("Authorization", key);
            }

            request.json(&knowledge_clone).send().await
        };

        // Execute with retry
        let response: ApiResponse<PublishResponse> = self.execute_with_retry(operation, "yumechain.publish").await?;

        Ok(response.data)
    }

    /// Query knowledge packages from YumeiCHAIN
    pub async fn query_knowledge(&self, query: KnowledgeQuery) -> Result<QueryResponse, YumeiChainError> {

        // Check circuit breaker
        if !self.circuit_breaker.allow_operation()? {
            return Err(YumeiChainError::CircuitBreakerOpen);
        }

        // Check rate limiter
        if !self.rate_limiter.acquire() {
            return Err(YumeiChainError::AuthenticationError("Rate limit exceeded".to_string()));
        }

        // Start metrics timer
        let start = std::time::Instant::now();

        // Build query parameters
        let mut params = Vec::new();
        if let Some(domain) = &query.domain {
            params.push(("domain", domain.clone()));
        }
        if let Some(type_) = &query.type_ {
            params.push(("type", type_.clone()));
        }
        if let Some(tags) = &query.tags {
            params.push(("tags", tags.join(",")));
        }
        if let Some(ai_node) = &query.ai_node {
            params.push(("ai_node", ai_node.clone()));
        }
        params.push(("min_confidence", query.min_confidence.to_string()));
        params.push(("limit", query.limit.to_string()));

        let url = format!("{}/knowledge", self.config.api_url);
        let params_clone = params.clone();

        // Define the operation
        let operation = || async {
            self.client
                .get(&url)
                .query(&params_clone)
                .send()
                .await
        };

        // Execute with retry
        let response = self.execute_with_retry::<QueryResponse, _, _>(operation, "yumechain.query").await?;

        Ok(response)
    }

    /// Get a specific knowledge package by ID
    pub async fn get_knowledge(&self, knowledge_id: &str) -> Result<KnowledgePackage, YumeiChainError> {

        // Check circuit breaker
        if !self.circuit_breaker.allow_operation()? {
            return Err(YumeiChainError::CircuitBreakerOpen);
        }

        // Check rate limiter
        if !self.rate_limiter.acquire() {
            return Err(YumeiChainError::AuthenticationError("Rate limit exceeded".to_string()));
        }

        // Start metrics timer
        let start = std::time::Instant::now();

        // Make API request

        let url = format!("{}/knowledge/{}", self.config.api_url, knowledge_id);
        let knowledge_id_str = knowledge_id.to_string();

        // Define the operation
        let operation = || async {
            self.client
                .get(&url)
                .send()
                .await
        };

        // Execute with retry
        let response = self.execute_with_retry::<KnowledgePackage, _, _>(operation, "yumechain.get").await?;

        Ok(response)
    }

    /// Update an existing knowledge package
    pub async fn update_knowledge(&self, knowledge_id: &str, mut knowledge: KnowledgePackage) -> Result<PublishResponse, YumeiChainError> {

        // Check circuit breaker
        if !self.circuit_breaker.allow_operation()? {
            return Err(YumeiChainError::CircuitBreakerOpen);
        }

        // Check rate limiter
        if !self.rate_limiter.acquire() {
            return Err(YumeiChainError::AuthenticationError("Rate limit exceeded".to_string()));
        }


        // Set the knowledge ID
        knowledge.knowledge_id = Some(knowledge_id.to_string());

        // Set the generating node if not already set
        if knowledge.content.generated_by.is_empty() {
            knowledge.content.generated_by = self.config.node_id.clone();
        }

        let url = format!("{}/knowledge/{}", self.config.api_url, knowledge_id);
        let api_key = self.config.api_key.clone();
        let knowledge_clone = knowledge.clone();

        // Define the operation
        let operation = || async {
            let mut request = self.client.put(&url);

            // Add authorization if available
            if let Some(key) = &api_key {
                request = request.header("Authorization", key);
            }

            request.json(&knowledge_clone).send().await
        };

        // Execute with retry
        let response: ApiResponse<PublishResponse> = self.execute_with_retry(operation, "yumechain.update").await?;

        Ok(response.data)
    }

    /// Evaluate (upvote/downvote) a knowledge package
    pub async fn evaluate_knowledge(&self, knowledge_id: &str, mut evaluation: KnowledgeEvaluation) -> Result<EvaluationResponse, YumeiChainError> {
        // Check circuit breaker
        if !self.circuit_breaker.allow_operation()? {
            return Err(YumeiChainError::CircuitBreakerOpen);
        }

        // Check rate limiter
        if !self.rate_limiter.acquire() {
            return Err(YumeiChainError::AuthenticationError("Rate limit exceeded".to_string()));
        }


        // Set the evaluating node if not already set
        if evaluation.evaluating_node.is_empty() {
            evaluation.evaluating_node = self.config.node_id.clone();
        }

        let url = format!("{}/knowledge/{}/evaluate", self.config.api_url, knowledge_id);
        let api_key = self.config.api_key.clone();
        let evaluation_clone = evaluation.clone();

        // Define the operation
        let operation = || async {
            let mut request = self.client.post(&url);

            // Add authorization if available
            if let Some(key) = &api_key {
                request = request.header("Authorization", key);
            }

            request.json(&evaluation_clone).send().await
        };

        // Execute with retry
        let response: ApiResponse<EvaluationResponse> = self.execute_with_retry(operation, "yumechain.evaluate").await?;

        Ok(response.data)
    }

    /// Resolve a conflict between knowledge packages
    pub async fn resolve_conflict(&self, knowledge_id: &str, mut resolution: ConflictResolution) -> Result<(), YumeiChainError> {
       // Check circuit breaker
        if !self.circuit_breaker.allow_operation()? {
            return Err(YumeiChainError::CircuitBreakerOpen);
        }

        // Check rate limiter
        if !self.rate_limiter.acquire() {
            return Err(YumeiChainError::AuthenticationError("Rate limit exceeded".to_string()));
        }

        // Set the resolving node if not already set
        if resolution.resolving_node.is_empty() {
            resolution.resolving_node = self.config.node_id.clone();
        }

        let url = format!("{}/conflict/{}/resolve", self.config.api_url, knowledge_id);
        let api_key = self.config.api_key.clone();
        let resolution_clone = resolution.clone();

        // Define the operation
        let operation = || async {
            let mut request = self.client.post(&url);

            // Add authorization if available
            if let Some(key) = &api_key {
                request = request.header("Authorization", key);
            }

            request.json(&resolution_clone).send().await
        };

        // Execute with retry
        let _: serde_json::Value = self.execute_with_retry(operation, "yumechain.resolve").await?;

        Ok(())
    }

    /// Get client metrics
    pub fn get_metrics(&self) -> Result<serde_json::Value, YumeiChainError> {
        let metrics = self.metrics.get_all_metrics();
        Ok(serde_json::to_value(metrics).map_err(YumeiChainError::SerializationError)?)
    }
}