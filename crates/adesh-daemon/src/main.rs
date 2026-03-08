mod http;

use std::sync::Arc;

use adesh_core::{AppConfig, ports::storage::StorageProvider};
use adesh_storage_sqlite::SqliteStorage;
use anyhow::Context;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let config = AppConfig::from_env().context("invalid configuration")?;
    let storage = Arc::new(SqliteStorage::connect(&config.database_url).await?);
    storage.migrate().await?;

    let listener = TcpListener::bind(config.bind_addr).await?;
    let app = http::app(config.clone(), storage);

    info!(addr = %config.bind_addr, "starting adesh daemon");
    axum::serve(listener, app).await?;
    Ok(())
}
