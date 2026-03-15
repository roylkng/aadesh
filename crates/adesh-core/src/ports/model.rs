use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::StorageError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelGenerateInput {
    pub operation_id: String,
    pub isolation_id: String,
    pub audit_trace_id: String,
    pub request_content: String,
    pub attachment_count: usize,
    pub attachment_context: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelGenerateOutput {
    pub schema_version: String,
    pub operation_id: String,
    pub reasoning_output: Value,
    pub model_id: String,
    pub provider_trace_id: Option<String>,
}

#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn health(&self) -> Result<(), StorageError>;

    async fn generate(
        &self,
        input: ModelGenerateInput,
    ) -> Result<ModelGenerateOutput, StorageError>;
}
