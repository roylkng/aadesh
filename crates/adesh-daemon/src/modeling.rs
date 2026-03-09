use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Value, json};
use uuid::Uuid;

use adesh_core::{
    AppConfig, StorageError,
    ports::model::{ModelGenerateInput, ModelGenerateOutput, ModelProvider},
};

pub struct FakeModelProvider;

#[async_trait]
impl ModelProvider for FakeModelProvider {
    async fn health(&self) -> Result<(), StorageError> {
        Ok(())
    }

    async fn generate(
        &self,
        input: ModelGenerateInput,
    ) -> Result<ModelGenerateOutput, StorageError> {
        let content = input.request_content.trim();
        let subject = if content.to_lowercase().contains("follow-up") {
            "Follow-up"
        } else if content.to_lowercase().contains("reply") {
            "Draft reply"
        } else {
            "Draft email"
        };

        let reasoning_output = json!({
            "schema_version": "0.1",
            "operation_id": input.operation_id,
            "intent": {
                "goal": "draft_email",
                "constraints_ack": [
                    "tools require syscall proposals",
                    "approval required for gated actions",
                    "no disclosure beyond audience ceilings"
                ],
                "risk_posture": "conservative",
                "sensitivity_posture": "minimize"
            },
            "plan": {
                "plan_steps": [],
                "stop_condition": "draft_ready"
            },
            "drafts": [
                {
                    "draft_id": "draft:1",
                    "channel": "draft",
                    "format": "plain_text",
                    "title": subject,
                    "content": format!("Subject: {subject}\n\n{}\n\nContext artifacts: {}", content, input.attachment_count)
                }
            ],
            "proposed_syscalls": [],
            "ipc_requests": [],
            "self_check": {
                "grounding": "provided_artifacts_only",
                "contains_unverified_high_stakes_facts": false
            }
        });

        Ok(ModelGenerateOutput {
            schema_version: "0.1".to_string(),
            operation_id: input.operation_id,
            reasoning_output,
            model_id: "fake-model-v1".to_string(),
            provider_trace_id: Some(format!("model-trace:{}", Uuid::new_v4())),
        })
    }
}

pub struct LmStudioModelProvider {
    base_url: String,
    model: String,
    client: Client,
}

impl LmStudioModelProvider {
    pub fn new(base_url: String, model: String) -> Result<Self, StorageError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(45))
            .build()
            .map_err(|err| StorageError::Unavailable(format!("model client init failed: {err}")))?;

        Ok(Self {
            base_url,
            model,
            client,
        })
    }
}

#[async_trait]
impl ModelProvider for LmStudioModelProvider {
    async fn health(&self) -> Result<(), StorageError> {
        let response = self
            .client
            .get(format!("{}/v1/models", self.base_url.trim_end_matches('/')))
            .send()
            .await
            .map_err(|err| {
                StorageError::Unavailable(format!("model health probe failed: {err}"))
            })?;

        if !response.status().is_success() {
            return Err(StorageError::Unavailable(format!(
                "model health probe returned HTTP {}",
                response.status()
            )));
        }

        let body: Value = response.json().await.map_err(|err| {
            StorageError::Corruption(format!("model health response was not valid JSON: {err}"))
        })?;

        let has_model = body
            .get("data")
            .and_then(Value::as_array)
            .is_some_and(|models| {
                models.iter().any(|entry| {
                    entry
                        .get("id")
                        .and_then(Value::as_str)
                        .is_some_and(|id| id == self.model)
                })
            });

        if !has_model {
            return Err(StorageError::Unavailable(format!(
                "configured model not advertised by provider: {}",
                self.model
            )));
        }

        Ok(())
    }

    async fn generate(
        &self,
        input: ModelGenerateInput,
    ) -> Result<ModelGenerateOutput, StorageError> {
        let payload = json!({
            "model": self.model,
            "input": build_lm_studio_prompt(&input),
        });

        let response = self
            .client
            .post(format!(
                "{}/v1/responses",
                self.base_url.trim_end_matches('/')
            ))
            .json(&payload)
            .send()
            .await
            .map_err(|err| StorageError::Unavailable(format!("model request failed: {err}")))?;

        let status = response.status();
        let provider_body: Value = response.json().await.map_err(|err| {
            StorageError::Corruption(format!("model response was not valid JSON: {err}"))
        })?;

        if !status.is_success() {
            return Err(StorageError::Unavailable(format!(
                "model provider returned HTTP {status}"
            )));
        }

        let output_text = extract_output_text(&provider_body)?;
        let reasoning_output = parse_reasoning_output(&input, &output_text)?;

        Ok(ModelGenerateOutput {
            schema_version: "0.1".to_string(),
            operation_id: input.operation_id,
            reasoning_output,
            model_id: self.model.clone(),
            provider_trace_id: provider_body
                .get("id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        })
    }
}

pub fn build_model_provider(config: &AppConfig) -> Result<Arc<dyn ModelProvider>, StorageError> {
    match config.model_provider_backend.as_str() {
        "lm_studio" => LmStudioModelProvider::new(
            config.model_provider_base_url.clone(),
            config.model_provider_model.clone(),
        )
        .map(|provider| Arc::new(provider) as Arc<dyn ModelProvider>),
        "fake" => Ok(Arc::new(FakeModelProvider)),
        other => Err(StorageError::InvalidInput(format!(
            "unknown model provider backend: {other}"
        ))),
    }
}

fn build_lm_studio_prompt(input: &ModelGenerateInput) -> String {
    format!(
        concat!(
            "Return exactly one JSON object and no markdown.\n",
            "Schema:\n",
            "{{\n",
            "  \"schema_version\": \"0.1\",\n",
            "  \"operation_id\": \"{operation_id}\",\n",
            "  \"intent\": {{\n",
            "    \"goal\": \"draft_email\",\n",
            "    \"constraints_ack\": [\"tools require syscall proposals\", \"approval required for gated actions\", \"no disclosure beyond audience ceilings\"],\n",
            "    \"risk_posture\": \"conservative\",\n",
            "    \"sensitivity_posture\": \"minimize\"\n",
            "  }},\n",
            "  \"plan\": {{\"plan_steps\": [], \"stop_condition\": \"draft_ready\"}},\n",
            "  \"drafts\": [{{\n",
            "    \"draft_id\": \"draft:1\",\n",
            "    \"channel\": \"draft\",\n",
            "    \"format\": \"plain_text\",\n",
            "    \"title\": \"Draft email\",\n",
            "    \"content\": \"Subject: ...\\n\\n...\"\n",
            "  }}],\n",
            "  \"proposed_syscalls\": [],\n",
            "  \"ipc_requests\": [],\n",
            "  \"self_check\": {{\n",
            "    \"grounding\": \"provided_artifacts_only\",\n",
            "    \"contains_unverified_high_stakes_facts\": false\n",
            "  }}\n",
            "}}\n",
            "Use the exact operation_id shown above.\n",
            "Request content: {request_content}\n",
            "Attachment count: {attachment_count}\n"
        ),
        operation_id = input.operation_id,
        request_content = input.request_content,
        attachment_count = input.attachment_count,
    )
}

fn extract_output_text(provider_body: &Value) -> Result<String, StorageError> {
    let output = provider_body
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            StorageError::Corruption("model response missing output array".to_string())
        })?;

    for item in output {
        if item.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }

        let Some(content) = item.get("content").and_then(Value::as_array) else {
            continue;
        };

        for part in content {
            if part.get("type").and_then(Value::as_str) == Some("output_text") {
                let text = part
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        StorageError::Corruption("model response output_text was empty".to_string())
                    })?;
                return Ok(text.to_string());
            }
        }
    }

    Err(StorageError::Corruption(
        "model response missing assistant output_text".to_string(),
    ))
}

fn parse_reasoning_output(
    input: &ModelGenerateInput,
    output_text: &str,
) -> Result<Value, StorageError> {
    let parsed = serde_json::from_str::<Value>(output_text)
        .or_else(|_| {
            let start =
                output_text
                    .find('{')
                    .ok_or(serde_json::Error::io(std::io::Error::other(
                        "missing json start",
                    )))?;
            let end =
                output_text
                    .rfind('}')
                    .ok_or(serde_json::Error::io(std::io::Error::other(
                        "missing json end",
                    )))?;
            serde_json::from_str::<Value>(&output_text[start..=end])
        })
        .map_err(|err| {
            StorageError::Corruption(format!("model output was not parseable JSON: {err}"))
        })?;

    validate_reasoning_output(input, &parsed)?;
    Ok(parsed)
}

fn validate_reasoning_output(
    input: &ModelGenerateInput,
    reasoning_output: &Value,
) -> Result<(), StorageError> {
    let schema_version = reasoning_output
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            StorageError::Corruption("reasoning output missing schema_version".to_string())
        })?;
    if schema_version != "0.1" {
        return Err(StorageError::Corruption(format!(
            "reasoning output schema_version must be 0.1, got {schema_version}"
        )));
    }

    let operation_id = reasoning_output
        .get("operation_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            StorageError::Corruption("reasoning output missing operation_id".to_string())
        })?;
    if operation_id != input.operation_id {
        return Err(StorageError::Corruption(format!(
            "reasoning output operation_id mismatch: expected {}, got {operation_id}",
            input.operation_id
        )));
    }

    if !reasoning_output.get("drafts").is_some_and(Value::is_array) {
        return Err(StorageError::Corruption(
            "reasoning output missing drafts array".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{LmStudioModelProvider, extract_output_text, parse_reasoning_output};
    use adesh_core::ports::model::{ModelGenerateInput, ModelProvider};
    use axum::{Json, routing::get};
    use serde_json::json;
    use tokio::net::TcpListener;

    fn sample_input() -> ModelGenerateInput {
        ModelGenerateInput {
            operation_id: "op-1".to_string(),
            isolation_id: "iso-1".to_string(),
            audit_trace_id: "audit-1".to_string(),
            request_content: "draft an email".to_string(),
            attachment_count: 0,
        }
    }

    #[test]
    fn extracts_output_text_from_lm_studio_shape() {
        let body = json!({
            "output": [
                {"type": "reasoning", "content": [{"type": "reasoning_text", "text": "thinking"}]},
                {"type": "message", "content": [{"type": "output_text", "text": "\n {\"ok\":true} \n"}]}
            ]
        });

        let text = extract_output_text(&body).unwrap();
        assert_eq!(text, "{\"ok\":true}");
    }

    #[test]
    fn parses_json_wrapped_in_extra_text() {
        let parsed = parse_reasoning_output(
            &sample_input(),
            "Here is the result:\n{\"schema_version\":\"0.1\",\"operation_id\":\"op-1\",\"drafts\":[]}",
        )
        .unwrap();

        assert_eq!(parsed["operation_id"], "op-1");
    }

    #[test]
    fn rejects_missing_required_fields() {
        let err = parse_reasoning_output(&sample_input(), "{\"schema_version\":\"0.1\"}")
            .unwrap_err()
            .to_string();
        assert!(err.contains("operation_id") || err.contains("drafts"));
    }

    #[tokio::test]
    async fn lm_studio_health_accepts_configured_model() {
        let app = axum::Router::new().route(
            "/v1/models",
            get(|| async {
                Json(json!({
                    "data": [
                        {"id": "qwen/qwen3.5-35b-a3b"},
                        {"id": "other-model"}
                    ]
                }))
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let provider = LmStudioModelProvider::new(
            format!("http://{addr}"),
            "qwen/qwen3.5-35b-a3b".to_string(),
        )
        .unwrap();

        provider.health().await.unwrap();
        handle.abort();
    }
}
