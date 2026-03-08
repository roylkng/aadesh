use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Meta {
    pub request_id: String,
    pub ts: DateTime<Utc>,
    pub audit_trace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiSuccess<T> {
    pub ok: bool,
    pub data: T,
    pub meta: Meta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiErrorResponse {
    pub ok: bool,
    pub error: ApiErrorBody,
    pub meta: Meta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub storage: String,
    pub model_provider: String,
    pub tool_provider: String,
    pub queue: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestEnvelope {
    pub request_id: String,
    pub source: RequestSource,
    pub received_at: DateTime<Utc>,
    pub requesting_principal: RequestingPrincipal,
    pub requesting_audience_id: String,
    pub input: RequestInput,
    pub constraints: RequestConstraints,
    pub conversation: Option<RequestConversation>,
    pub intent_anchor: Option<IntentAnchor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestSource {
    pub channel: String,
    pub transport: String,
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestingPrincipal {
    pub principal_type: String,
    pub principal_id: String,
    pub owner_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestConversation {
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    #[serde(default)]
    pub history_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestInput {
    pub kind: String,
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<AttachmentRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentRef {
    pub ref_id: String,
    pub ref_type: String,
    pub sensitivity_hint: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestConstraints {
    pub policy_mode: String,
    pub budgets: RequestBudgets,
    pub preferred_model: Option<String>,
    pub allow_multi_operation: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestBudgets {
    pub token_budget: i64,
    pub latency_ms: Option<i64>,
    pub cost_cents: Option<i64>,
    pub compute_units: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentAnchor {
    pub goal: Option<String>,
    #[serde(default)]
    pub success_criteria: Vec<String>,
    #[serde(default)]
    pub forbidden_outcomes: Vec<String>,
    #[serde(default)]
    pub scope_limits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestAcceptedResponse {
    pub request_id: String,
    pub operation_ids: Vec<String>,
    pub primary_operation_id: String,
    pub audit_trace_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WsEnvelope<T> {
    pub event_id: String,
    pub ts: DateTime<Utc>,
    pub r#type: String,
    pub request_id: Option<String>,
    pub operation_id: Option<String>,
    pub isolation_id: Option<String>,
    pub audit_trace_id: Option<String>,
    pub data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WsHelloData {
    pub message: String,
    pub server_version: String,
    pub capability_snapshot_version: String,
}
