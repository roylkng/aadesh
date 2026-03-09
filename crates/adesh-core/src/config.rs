use std::{env, net::SocketAddr};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: SocketAddr,
    pub root_owner_token: String,
    pub database_url: String,
    pub server_version: String,
    pub capability_snapshot_version: String,
    pub model_provider_backend: String,
    pub model_provider_base_url: String,
    pub model_provider_model: String,
    pub email_provider_backend: String,
    pub email_from_address: String,
    pub email_smtp_host: String,
    pub email_smtp_port: u16,
    pub email_smtp_username: Option<String>,
    pub email_smtp_password: Option<String>,
    pub webhook_provider_backend: String,
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
        let model_provider_backend =
            env::var("ADESH_MODEL_PROVIDER_BACKEND").unwrap_or_else(|_| "fake".to_string());
        let model_provider_base_url = env::var("ADESH_MODEL_PROVIDER_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:1234".to_string());
        let model_provider_model = env::var("ADESH_MODEL_PROVIDER_MODEL")
            .unwrap_or_else(|_| "qwen/qwen3.5-35b-a3b".to_string());
        let email_provider_backend =
            env::var("ADESH_EMAIL_PROVIDER_BACKEND").unwrap_or_else(|_| "fake".to_string());
        let email_from_address = env::var("ADESH_EMAIL_FROM_ADDRESS")
            .unwrap_or_else(|_| "adesh@example.invalid".to_string());
        let email_smtp_host =
            env::var("ADESH_EMAIL_SMTP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let email_smtp_port = env::var("ADESH_EMAIL_SMTP_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1025);
        let email_smtp_username = env::var("ADESH_EMAIL_SMTP_USERNAME")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let email_smtp_password = env::var("ADESH_EMAIL_SMTP_PASSWORD")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let webhook_provider_backend =
            env::var("ADESH_WEBHOOK_PROVIDER_BACKEND").unwrap_or_else(|_| "fake".to_string());

        Ok(Self {
            bind_addr,
            root_owner_token,
            database_url,
            server_version,
            capability_snapshot_version,
            model_provider_backend,
            model_provider_base_url,
            model_provider_model,
            email_provider_backend,
            email_from_address,
            email_smtp_host,
            email_smtp_port,
            email_smtp_username,
            email_smtp_password,
            webhook_provider_backend,
        })
    }
}
