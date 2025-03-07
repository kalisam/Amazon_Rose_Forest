//! Integration tests for vector database functionality
use crate::core::vector_db::{ShardManager, VectorEntry};
use crate::utils::test_helpers::*;

#[tokio::test]
async fn test_vector_insertion_and_retrieval() {
    let manager = setup_test_shard_manager().await;
    let test_vector = create_test_vector();

    let result = manager.insert_vector(test_vector.clone()).await.expect("Insertion should succeed");
    assert!(result.is_ok());

    let retrieved = manager.get_vector(test_vector.id()).await;
    assert_eq!(retrieved.expect("Vector should be present"), test_vector);
}

#[tokio::test]
async fn test_shard_splitting() {
    let mut manager = setup_test_shard_manager().await;

    // Fill shard until split is needed
    for _ in 0..1000 {
        let vector = create_test_vector();
        manager.insert_vector(vector).await.unwrap();
    }

    // Verify shard was split correctly
    let metrics = manager.get_metrics().await;
    assert!(metrics.shard_count > 1);
}