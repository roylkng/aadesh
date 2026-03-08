use std::{env, net::SocketAddr};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: SocketAddr,
    pub root_owner_token: String,
    pub database_url: String,
    pub server_version: String,
    pub capability_snapshot_version: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, std::net::AddrParseError> {
        let bind_addr = env::var("ADESH_BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:7777".to_string())
            .parse()?;
        let root_owner_token = env::var("ADESH_ROOT_OWNER_TOKEN")
            .unwrap_or_else(|_| "dev-root-owner-token".to_string());
        let database_url = env::var("ADESH_DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://adesh.db?mode=rwc".to_string());
        let server_version = env::var("ADESH_SERVER_VERSION")
            .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
        let capability_snapshot_version = env::var("ADESH_BOOTSTRAP_CAPABILITY_SNAPSHOT")
            .unwrap_or_else(|_| "cap:bootstrap".to_string());

        Ok(Self {
            bind_addr,
            root_owner_token,
            database_url,
            server_version,
            capability_snapshot_version,
        })
    }
}
