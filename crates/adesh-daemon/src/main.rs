use std::sync::Arc;

use adesh_contracts::{
    PrepareTaskContextRequest, RecallRelevantMemoryRequest, StoreWorkEpisodeRequest,
};
use adesh_core::{AppConfig, ports::storage::StorageProvider};
use adesh_daemon::{cognition, host_cli, http};
use adesh_storage_sqlite::SqliteStorage;
use anyhow::{Context, bail};
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let config = AppConfig::from_env().context("invalid configuration")?;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if matches!(args.first().map(String::as_str), Some("cognitive")) {
        return run_cognitive_cli(config, &args[1..]).await;
    }
    if matches!(args.first().map(String::as_str), Some("host")) {
        return host_cli::run_host_cli(config, &args[1..]).await;
    }

    if matches!(args.first().map(String::as_str), Some("serve")) {
        return run_server(config).await;
    }

    run_server(config).await
}

async fn run_server(config: AppConfig) -> anyhow::Result<()> {
    let storage = Arc::new(SqliteStorage::connect(&config.database_url).await?);
    storage.migrate().await?;

    let listener = TcpListener::bind(config.bind_addr).await?;
    let app = http::app(config.clone(), storage).context("failed to build daemon app")?;

    info!(addr = %config.bind_addr, "starting adesh daemon");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn run_cognitive_cli(config: AppConfig, args: &[String]) -> anyhow::Result<()> {
    let Some(command) = args.first().map(String::as_str) else {
        bail!("missing cognitive subcommand");
    };
    let json_arg = parse_json_flag(&args[1..])?;

    let storage = Arc::new(SqliteStorage::connect(&config.database_url).await?);
    storage.migrate().await?;

    match command {
        "store-work-episode" => {
            let request: StoreWorkEpisodeRequest =
                serde_json::from_str(json_arg).context("invalid store_work_episode payload")?;
            let response = cognition::store_work_episode(storage.as_ref(), request)
                .await
                .context("store_work_episode failed")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&response)
                    .context("failed to serialize store_work_episode response")?
            );
        }
        "prepare-task-context" => {
            let request: PrepareTaskContextRequest =
                serde_json::from_str(json_arg).context("invalid prepare_task_context payload")?;
            let response = cognition::prepare_task_context(storage.as_ref(), request)
                .await
                .context("prepare_task_context failed")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&response)
                    .context("failed to serialize prepare_task_context response")?
            );
        }
        "recall-relevant-memory" => {
            let request: RecallRelevantMemoryRequest =
                serde_json::from_str(json_arg).context("invalid recall_relevant_memory payload")?;
            let response = cognition::recall_relevant_memory(storage.as_ref(), request)
                .await
                .context("recall_relevant_memory failed")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&response)
                    .context("failed to serialize recall_relevant_memory response")?
            );
        }
        other => bail!("unsupported cognitive subcommand: {other}"),
    }

    Ok(())
}

fn parse_json_flag(args: &[String]) -> anyhow::Result<&str> {
    if args.len() != 2 || args[0] != "--json" {
        bail!("usage: adesh-daemon cognitive <subcommand> --json '<payload>'");
    }
    Ok(args[1].as_str())
}
