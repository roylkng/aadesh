use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::StorageError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    pub ok: bool,
    pub output_kind: String,
    pub output_json: Value,
    pub content_ref: Option<String>,
    pub sensitivity_s: i64,
    pub taint_s: i64,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub attempts_used: i64,
}

#[async_trait]
pub trait ToolProvider: Send + Sync {
    async fn health(&self) -> Result<(), StorageError>;

    async fn execute_syscall(
        &self,
        syscall_id: &str,
        tool_name: &str,
        action_name: &str,
        args_schema_ref: &str,
        result_schema_ref: Option<&str>,
        args: &Value,
    ) -> Result<ToolExecutionResult, StorageError>;
}
