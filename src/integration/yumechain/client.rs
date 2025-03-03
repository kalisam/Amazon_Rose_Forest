//! YumeiCHAIN API client for knowledge exchange
//!
//! This module provides a client for interacting with the YumeiCHAIN API
//! to publish, query, update, and evaluate knowledge.

use std::sync::Arc;
use reqwest::{Client, StatusCode};
use serde_json::json;
use thiserror::Error;
use crate::metrics::collector::MetricsCollector;
use crate::core::fault::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use super::schema::{
    KnowledgePackage, KnowledgeQuery, KnowledgeEvaluation, ConflictResolution,
    ApiResponse, PublishResponse, QueryResponse, EvaluationResponse
};

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
}

impl Default for YumeiChainConfig {
    fn default() -> Self {
        Self {
            api_url: "http://localhost:8000".to_string(),
            api_key: None,
            node_id: "amazon-rose-forest".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
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
    circuit_breaker: CircuitBreaker,

    /// Metrics collector
    metrics: Arc<MetricsCollector>,
}

impl YumeiChainClient {
    /// Create a new YumeiCHAIN client with the specified configuration
    pub fn new(config: YumeiChainConfig, metrics: Arc<MetricsCollector>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .build()
            .expect("Failed to create HTTP client");

        let circuit_breaker_config = CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 3,
            max_half_open_attempts: 10,
            reset_timeout: std::time::Duration::from_secs(60),
        };

        Self {
            client,
            config,
            circuit_breaker: CircuitBreaker::new(circuit_breaker_config),
            metrics,
        }
    }

    /// Register this AI node with YumeiCHAIN
    pub async fn register_node(&self, public_key: &str) -> Result<(), YumeiChainError> {
        // Check circuit breaker
        if !self.circuit_breaker.allow_operation()? {
            return Err(YumeiChainError::CircuitBreakerOpen);
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
        let duration = start.elapsed();
        self.metrics.record_histogram("yumechain.register.duration", duration.as_millis() as f64, None);

        // Handle response
        match response {
            Ok(res) => {
                if res.status().is_success() {
                    self.circuit_breaker.record_success()?;
                    self.metrics.increment_counter("yumechain.register.success", 1.0);
                    Ok(())
                } else {
                    self.circuit_breaker.record_failure()?;
                    self.metrics.increment_counter("yumechain.register.error", 1.0);
                    Err(YumeiChainError::ApiError {
                        status: res.status(),
                        message: res.text().await.unwrap_or_else(|_| "Unknown error".to_string()),
                    })
                }
            }
            Err(e) => {
                self.circuit_breaker.record_failure()?;
                self.metrics.increment_counter("yumechain.register.error", 1.0);
                Err(YumeiChainError::NetworkError(e))
            }
        }
    }

    /// Publish a knowledge package to YumeiCHAIN
    pub async fn publish_knowledge(&self, mut knowledge: KnowledgePackage) -> Result<PublishResponse, YumeiChainError> {
        // Check circuit breaker
        if !self.circuit_breaker.allow_operation()? {
            return Err(YumeiChainError::CircuitBreakerOpen);
        }

        // Ensure knowledge has IDs
        knowledge.ensure_ids();

        // Set the generating node if not already set
        if knowledge.content.generated_by.is_empty() {
            knowledge.content.generated_by = self.config.node_id.clone();
        }

        // Start metrics timer
        let start = std::time::Instant::now();

        // Make API request
        let url = format!("{}/knowledge", self.config.api_url);
        let mut request = self.client.post(&url);

        // Add authorization if available
        if let Some(api_key) = &self.config.api_key {
            request = request.header("Authorization", api_key);
        }

        let response = request
            .json(&knowledge)
            .send()
            .await;

        // Record metrics
        let duration = start.elapsed();
        self.metrics.record_histogram("yumechain.publish.duration", duration.as_millis() as f64, None);

        // Handle response
        match response {
            Ok(res) => {
                if res.status().is_success() {
                    self.circuit_breaker.record_success()?;
                    self.metrics.increment_counter("yumechain.publish.success", 1.0);

                    let api_response: ApiResponse<PublishResponse> = res.json().await?;
                    Ok(api_response.data)
                } else {
                    self.circuit_breaker.record_failure()?;
                    self.metrics.increment_counter("yumechain.publish.error", 1.0);

                    Err(YumeiChainError::ApiError {
                        status: res.status(),
                        message: res.text().await.unwrap_or_else(|_| "Unknown error".to_string()),
                    })
                }
            }
            Err(e) => {
                self.circuit_breaker.record_failure()?;
                self.metrics.increment_counter("yumechain.publish.error", 1.0);

                Err(YumeiChainError::NetworkError(e))
            }
        }
    }

    /// Query knowledge packages from YumeiCHAIN
    pub async fn query_knowledge(&self, query: KnowledgeQuery) -> Result<QueryResponse, YumeiChainError> {
        // Check circuit breaker
        if !self.circuit_breaker.allow_operation()? {
            return Err(YumeiChainError::CircuitBreakerOpen);
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

        // Make API request
        let url = format!("{}/knowledge", self.config.api_url);
        let response = self.client
            .get(&url)
            .query(&params)
            .send()
            .await;

        // Record metrics
        let duration = start.elapsed();
        self.metrics.record_histogram("yumechain.query.duration", duration.as_millis() as f64, None);

        // Handle response
        match response {
            Ok(res) => {
                if res.status().is_success() {
                    self.circuit_breaker.record_success()?;
                    self.metrics.increment_counter("yumechain.query.success", 1.0);

                    let query_response: QueryResponse = res.json().await?;
                    Ok(query_response)
                } else {
                    self.circuit_breaker.record_failure()?;
                    self.metrics.increment_counter("yumechain.query.error", 1.0);

                    Err(YumeiChainError::ApiError {
                        status: res.status(),
                        message: res.text().await.unwrap_or_else(|_| "Unknown error".to_string()),
                    })
                }
            }
            Err(e) => {
                self.circuit_breaker.record_failure()?;
                self.metrics.increment_counter("yumechain.query.error", 1.0);

                Err(YumeiChainError::NetworkError(e))
            }
        }
    }

    /// Get a specific knowledge package by ID
    pub async fn get_knowledge(&self, knowledge_id: &str) -> Result<KnowledgePackage, YumeiChainError> {
        // Check circuit breaker
        if !self.circuit_breaker.allow_operation()? {
            return Err(YumeiChainError::CircuitBreakerOpen);
        }

        // Start metrics timer
        let start = std::time::Instant::now();

        // Make API request
        let url = format!("{}/knowledge/{}", self.config.api_url, knowledge_id);
        let response = self.client
            .get(&url)
            .send()
            .await;

        // Record metrics
        let duration = start.elapsed();
        self.metrics.record_histogram("yumechain.get.duration", duration.as_millis() as f64, None);

        // Handle response
        match response {
            Ok(res) => {
                if res.status().is_success() {
                    self.circuit_breaker.record_success()?;
                    self.metrics.increment_counter("yumechain.get.success", 1.0);

                    let knowledge: KnowledgePackage = res.json().await?;
                    Ok(knowledge)
                } else {
                    self.circuit_breaker.record_failure()?;
                    self.metrics.increment_counter("yumechain.get.error", 1.0);

                    Err(YumeiChainError::ApiError {
                        status: res.status(),
                        message: res.text().await.unwrap_or_else(|_| "Unknown error".to_string()),
                    })
                }
            }
            Err(e) => {
                self.circuit_breaker.record_failure()?;
                self.metrics.increment_counter("yumechain.get.error", 1.0);

                Err(YumeiChainError::NetworkError(e))
            }
        }
    }

    /// Update an existing knowledge package
    pub async fn update_knowledge(&self, knowledge_id: &str, mut knowledge: KnowledgePackage) -> Result<PublishResponse, YumeiChainError> {
        // Check circuit breaker
        if !self.circuit_breaker.allow_operation()? {
            return Err(YumeiChainError::CircuitBreakerOpen);
        }

        // Set the knowledge ID
        knowledge.knowledge_id = Some(knowledge_id.to_string());

        // Set the generating node if not already set
        if knowledge.content.generated_by.is_empty() {
            knowledge.content.generated_by = self.config.node_id.clone();
        }

        // Start metrics timer
        let start = std::time::Instant::now();

        // Make API request
        let url = format!("{}/knowledge/{}", self.config.api_url, knowledge_id);
        let mut request = self.client.put(&url);

        // Add authorization if available
        if let Some(api_key) = &self.config.api_key {
            request = request.header("Authorization", api_key);
        }

        let response = request
            .json(&knowledge)
            .send()
            .await;

        // Record metrics
        let duration = start.elapsed();
        self.metrics.record_histogram("yumechain.update.duration", duration.as_millis() as f64, None);

        // Handle response
        match response {
            Ok(res) => {
                if res.status().is_success() {
                    self.circuit_breaker.record_success()?;
                    self.metrics.increment_counter("yumechain.update.success", 1.0);

                    let api_response: ApiResponse<PublishResponse> = res.json().await?;
                    Ok(api_response.data)
                } else {
                    self.circuit_breaker.record_failure()?;
                    self.metrics.increment_counter("yumechain.update.error", 1.0);

                    Err(YumeiChainError::ApiError {
                        status: res.status(),
                        message: res.text().await.unwrap_or_else(|_| "Unknown error".to_string()),
                    })
                }
            }
            Err(e) => {
                self.circuit_breaker.record_failure()?;
                self.metrics.increment_counter("yumechain.update.error", 1.0);

                Err(YumeiChainError::NetworkError(e))
            }
        }
    }

    /// Evaluate (upvote/downvote) a knowledge package
    pub async fn evaluate_knowledge(&self, knowledge_id: &str, evaluation: KnowledgeEvaluation) -> Result<EvaluationResponse, YumeiChainError> {
        // Check circuit breaker
        if !self.circuit_breaker.allow_operation()? {
            return Err(YumeiChainError::CircuitBreakerOpen);
        }

        // Set the evaluating node if not already set
        let mut evaluation = evaluation;
        if evaluation.evaluating_node.is_empty() {
            evaluation.evaluating_node = self.config.node_id.clone();
        }

        // Start metrics timer
        let start = std::time::Instant::now();

        // Make API request
        let url = format!("{}/knowledge/{}/evaluate", self.config.api_url, knowledge_id);
        let mut request = self.client.post(&url);

        // Add authorization if available
        if let Some(api_key) = &self.config.api_key {
            request = request.header("Authorization", api_key);
        }

        let response = request
            .json(&evaluation)
            .send()
            .await;

        // Record metrics
        let duration = start.elapsed();
        self.metrics.record_histogram("yumechain.evaluate.duration", duration.as_millis() as f64, None);

        // Handle response
        match response {
            Ok(res) => {
                if res.status().is_success() {
                    self.circuit_breaker.record_success()?;
                    self.metrics.increment_counter("yumechain.evaluate.success", 1.0);

                    let api_response: ApiResponse<EvaluationResponse> = res.json().await?;
                    Ok(api_response.data)
                } else {
                    self.circuit_breaker.record_failure()?;
                    self.metrics.increment_counter("yumechain.evaluate.error", 1.0);

                    Err(YumeiChainError::ApiError {
                        status: res.status(),
                        message: res.text().await.unwrap_or_else(|_| "Unknown error".to_string()),
                    })
                }
            }
            Err(e) => {
                self.circuit_breaker.record_failure()?;
                self.metrics.increment_counter("yumechain.evaluate.error", 1.0);

                Err(YumeiChainError::NetworkError(e))
            }
        }
    }

    /// Resolve a conflict between knowledge packages
    pub async fn resolve_conflict(&self, knowledge_id: &str, resolution: ConflictResolution) -> Result<(), YumeiChainError> {
        // Check circuit breaker
        if !self.circuit_breaker.allow_operation()? {
            return Err(YumeiChainError::CircuitBreakerOpen);
        }

        // Set the resolving node if not already set
        let mut resolution = resolution;
        if resolution.resolving_node.is_empty() {
            resolution.resolving_node = self.config.node_id.clone();
        }

        // Start metrics timer
        let start = std::time::Instant::now();

        // Make API request
        let url = format!("{}/conflict/{}/resolve", self.config.api_url, knowledge_id);
        let mut request = self.client.post(&url);

        // Add authorization if available
        if let Some(api_key) = &self.config.api_key {
            request = request.header("Authorization", api_key);
        }

        let response = request
            .json(&resolution)
            .send()
            .await;

        // Record metrics
        let duration = start.elapsed();
        self.metrics.record_histogram("yumechain.resolve.duration", duration.as_millis() as f64, None);

        // Handle response
        match response {
            Ok(res) => {
                if res.status().is_success() {
                    self.circuit_breaker.record_success()?;
                    self.metrics.increment_counter("yumechain.resolve.success", 1.0);
                    Ok(())
                } else {
                    self.circuit_breaker.record_failure()?;
                    self.metrics.increment_counter("yumechain.resolve.error", 1.0);

                    Err(YumeiChainError::ApiError {
                        status: res.status(),
                        message: res.text().await.unwrap_or_else(|_| "Unknown error".to_string()),
                    })
                }
            }
            Err(e) => {
                self.circuit_breaker.record_failure()?;
                self.metrics.increment_counter("yumechain.resolve.error", 1.0);

                Err(YumeiChainError::NetworkError(e))
            }
        }
    }
}