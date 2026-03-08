use async_trait::async_trait;

use crate::StorageError;

#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn migrate(&self) -> Result<(), StorageError>;
    async fn health(&self) -> Result<(), StorageError>;
}
