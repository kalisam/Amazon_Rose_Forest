use amazon_rose_forest::{
    integration::yumechain::{
        client::{YumeiChainClient, YumeiChainConfig, YumeiChainError},
        schema::{KnowledgePackage, KnowledgeQuery, KnowledgeEvaluation},
    },
    metrics::collector::MetricsCollector,
};
use std::sync::Arc;
use mockito::{mock, server_url};

/// Test fixture for YumeiCHAIN client tests
struct TestFixture {
    client: YumeiChainClient,
    metrics: Arc<MetricsCollector>,
}

impl TestFixture {
    fn new() -> Self {
        let metrics = Arc::new(MetricsCollector::new());
        let config = YumeiChainConfig {
            api_url: server_url(), // Use mockito server URL
            api_key: Some("test-api-key".to_string()),
            node_id: "test-node-id".to_string(),
            timeout_seconds: 5,
            max_retries: 1,
        };

        let client = YumeiChainClient::new(config, Arc::clone(&metrics));

        Self {
            client,
            metrics,
        }
    }

    fn create_test_knowledge() -> KnowledgePackage {
        KnowledgePackage::new(
            "Test knowledge content".to_string(),
            "test-node-id".to_string(),
            0.95,
            "test-reasoning".to_string(),
            vec!["Step 1".to_string(), "Step 2".to_string()],
            "test-domain".to_string(),
            "test-type".to_string(),
        )
    }
}

#[tokio::test]
async fn test_register_node() {
    let fixture = TestFixture::new();

    // Setup mock response
    let _m = mock("POST", "/register")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("node_id".into(), "test-node-id".into()),
            mockito::Matcher::UrlEncoded("public_key".into(), "test-public-key".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"status": "success", "message": "Node registered successfully"}"#)
        .create();

    // Execute test
    let result = fixture.client.register_node("test-public-key").await;

    // Verify result
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_publish_knowledge() {
    let fixture = TestFixture::new();
    let knowledge = fixture.create_test_knowledge();

    // Setup mock response
    let _m = mock("POST", "/knowledge")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"status": "success", "message": "Knowledge published", "knowledge_id": "test-id-123", "version_id": "v1"}"#)
        .create();

    // Execute test
    let result = fixture.client.publish_knowledge(knowledge).await;

    // Verify result
    assert!(result.is_ok());
    let publish_response = result.unwrap();
    assert_eq!(publish_response.knowledge_id, "test-id-123");
    assert_eq!(publish_response.version_id, "v1");
}

#[tokio::test]
async fn test_query_knowledge() {
    let fixture = TestFixture::new();

    // Create query
    let query = KnowledgeQuery {
        domain: Some("test-domain".to_string()),
        type_: Some("test-type".to_string()),
        tags: None,
        ai_node: None,
        min_confidence: 0.8,
        limit: 10,
    };

    // Setup mock response
    let _m = mock("GET", "/knowledge")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("domain".into(), "test-domain".into()),
            mockito::Matcher::UrlEncoded("type".into(), "test-type".into()),
            mockito::Matcher::UrlEncoded("min_confidence".into(), "0.8".into()),
            mockito::Matcher::UrlEncoded("limit".into(), "10".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"count": 1, "results": [{"content": {"text": "Test content", "format": "markdown", "generated_by": "test-node"}, "confidence": {"overall": 0.9}, "reasoning_trace": {"method": "test", "steps": ["Step 1"]}, "metadata": {"domain": "test-domain", "type_": "test-type"}}]}"#)
        .create();

    // Execute test
    let result = fixture.client.query_knowledge(query).await;

    // Verify result
    assert!(result.is_ok());
    let query_response = result.unwrap();
    assert_eq!(query_response.count, 1);
    assert_eq!(query_response.results.len(), 1);
}

#[tokio::test]
async fn test_evaluate_knowledge() {
    let fixture = TestFixture::new();

    // Create evaluation
    let evaluation = KnowledgeEvaluation {
        evaluating_node: "test-node-id".to_string(),
        vote: "upvote".to_string(),
        reason: Some("Good knowledge".to_string()),
        confidence: 0.9,
    };

    // Setup mock response
    let _m = mock("POST", "/knowledge/test-id-123/evaluate")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"status": "success", "message": "Knowledge evaluated", "knowledge_id": "test-id-123", "new_score": 0.85, "votes": {"upvotes": 2, "downvotes": 0}}"#)
        .create();

    // Execute test
    let result = fixture.client.evaluate_knowledge("test-id-123", evaluation).await;

    // Verify result
    assert!(result.is_ok());
    let eval_response = result.unwrap();
    assert_eq!(eval_response.knowledge_id, "test-id-123");
    assert_eq!(eval_response.new_score, 0.85);
    assert_eq!(eval_response.votes.upvotes, 2);
    assert_eq!(eval_response.votes.downvotes, 0);
}

#[tokio::test]
async fn test_circuit_breaker_open() {
    let fixture = TestFixture::new();

    // Force circuit breaker to open
    for _ in 0..6 {
        let _ = fixture.client.circuit_breaker.record_failure();
    }

    // Attempt operation with open circuit breaker
    let result = fixture.client.register_node("test-public-key").await;

    // Verify circuit breaker prevented the operation
    assert!(matches!(result, Err(YumeiChainError::CircuitBreakerOpen)));
}