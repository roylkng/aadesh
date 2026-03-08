use async_trait::async_trait;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

use adesh_core::{StorageError, ports::storage::StorageProvider};

pub struct SqliteStorage {
    pool: SqlitePool,
}

impl SqliteStorage {
    pub async fn connect(database_url: &str) -> Result<Self, StorageError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[async_trait]
impl StorageProvider for SqliteStorage {
    async fn migrate(&self) -> Result<(), StorageError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))
    }

    async fn health(&self) -> Result<(), StorageError> {
        sqlx::query_scalar::<_, i64>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map(|_| ())
            .map_err(|err| StorageError::Unavailable(err.to_string()))
    }
}
