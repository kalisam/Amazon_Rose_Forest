//! DHT migration management
use super::*;
use crate::core::error::SystemError;
use std::sync::Arc;
use crate::core::vector_db::ShardMetrics;
pub struct MigrationManager {
    config: MigrationConfig,
    metrics: Arc<ShardMetrics>,
}

impl MigrationManager {
    pub async fn execute_migration(&self, plan: MigrationPlan) -> Result<(), SystemError> {
        let mut stream = StreamingMigration::new(plan);

        while let Some(batch) = stream.next_batch().await? {
            match self.transfer_batch(batch).await {
                Ok(_) => continue,
                Err(e) => {
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