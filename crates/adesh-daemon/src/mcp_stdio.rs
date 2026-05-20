use std::path::Path;

use adesh_contracts::{
    ConnectorEventRequest, PrepareTaskContextRequest, RecallRelevantMemoryRequest,
    StoreWorkEpisodeRequest,
};
use adesh_core::ports::storage::StorageProvider;
use anyhow::{Context, bail};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

use crate::{cognition, connector_adapter};

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
const MCP_SERVER_NAME: &str = "adesh-cognition-bridge";
const MCP_SERVER_VERSION: &str = "0.1.0";

pub async fn run_mcp_stdio<S: StorageProvider + ?Sized>(
    storage: &S,
    current_dir: Option<&Path>,
) -> anyhow::Result<()> {
    let cwd = current_dir.map(Path::to_path_buf);
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut stdout = tokio::io::stdout();

    loop {
        let Some(payload) = read_mcp_payload(&mut reader).await? else {
            break;
        };
        let request = match serde_json::from_slice::<Value>(&payload) {
            Ok(value) => value,
            Err(err) => {
                let response =
                    mcp_error_response(Value::Null, -32700, format!("invalid JSON payload: {err}"));
                write_mcp_payload(&mut stdout, &response).await?;
                continue;
            }
        };

        if let Some(response) = handle_mcp_request(storage, cwd.as_deref(), &request).await? {
            write_mcp_payload(&mut stdout, &response).await?;
        }
    }

    Ok(())
}

async fn handle_mcp_request<S: StorageProvider + ?Sized>(
    storage: &S,
    current_dir: Option<&Path>,
    request: &Value,
) -> anyhow::Result<Option<Value>> {
    let Some(method) = request
        .get("method")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return Ok(Some(mcp_error_response(
            request.get("id").cloned().unwrap_or(Value::Null),
            -32600,
            "missing method".to_string(),
        )));
    };

    if method.starts_with("notifications/") {
        return Ok(None);
    }

    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));

    let response = match method.as_str() {
        "initialize" => mcp_result_response(
            id,
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": MCP_SERVER_NAME,
                    "version": MCP_SERVER_VERSION
                }
            }),
        ),
        "ping" => mcp_result_response(id, json!({})),
        "tools/list" => mcp_result_response(id, json!({ "tools": mcp_tools_catalog() })),
        "tools/call" => {
            let call_result = handle_tools_call(storage, current_dir, &params).await;
            match call_result {
                Ok(result) => mcp_result_response(id, result),
                Err(err) => mcp_error_response(id, -32602, err.to_string()),
            }
        }
        _ => mcp_error_response(id, -32601, format!("unsupported MCP method: {method}")),
    };

    Ok(Some(response))
}

async fn handle_tools_call<S: StorageProvider + ?Sized>(
    storage: &S,
    _current_dir: Option<&Path>,
    params: &Value,
) -> anyhow::Result<Value> {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        bail!("tools/call params must include tool name");
    };
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match name {
        "adesh.prepare_task_context" => {
            let request: PrepareTaskContextRequest =
                serde_json::from_value(arguments).context("invalid prepare_task_context args")?;
            let response = cognition::prepare_task_context(storage, request)
                .await
                .context("prepare_task_context failed")?;
            Ok(mcp_tool_result(serde_json::to_value(response)?))
        }
        "adesh.store_work_episode" => {
            let request: StoreWorkEpisodeRequest =
                serde_json::from_value(arguments).context("invalid store_work_episode args")?;
            let response = cognition::store_work_episode(storage, request)
                .await
                .context("store_work_episode failed")?;
            Ok(mcp_tool_result(serde_json::to_value(response)?))
        }
        "adesh.recall_relevant_memory" => {
            let request: RecallRelevantMemoryRequest =
                serde_json::from_value(arguments).context("invalid recall_relevant_memory args")?;
            let response = cognition::recall_relevant_memory(storage, request)
                .await
                .context("recall_relevant_memory failed")?;
            Ok(mcp_tool_result(serde_json::to_value(response)?))
        }
        "adesh.connector_event" => {
            let request: ConnectorEventRequest =
                serde_json::from_value(arguments).context("invalid connector_event args")?;
            let response = connector_adapter::handle_connector_event(storage, request)
                .await
                .context("connector_event failed")?;
            Ok(mcp_tool_result(serde_json::to_value(response)?))
        }
        other => bail!("unsupported MCP tool: {other}"),
    }
}

fn mcp_tool_result(structured: Value) -> Value {
    let text = serde_json::to_string_pretty(&structured).unwrap_or_else(|_| structured.to_string());
    json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "structuredContent": structured,
        "isError": false
    })
}

fn mcp_tools_catalog() -> Vec<Value> {
    vec![
        json!({
            "name": "adesh.prepare_task_context",
            "description": "Return compact, evidence-grounded guidance for the current task using cross-session memory.",
            "inputSchema": {
                "type": "object",
                "required": ["workspace", "task_prompt"],
                "properties": {
                    "workspace": { "type": "object" },
                    "task_prompt": { "type": "string" },
                    "files_in_focus": { "type": "array", "items": { "type": "string" } },
                    "task_hint": { "type": ["string", "null"] }
                },
                "additionalProperties": true
            }
        }),
        json!({
            "name": "adesh.store_work_episode",
            "description": "Store one work episode and promote scoped memory candidates for later retrieval.",
            "inputSchema": {
                "type": "object",
                "required": ["workspace", "task_prompt", "summary"],
                "properties": {
                    "workspace": { "type": "object" },
                    "task_prompt": { "type": "string" },
                    "summary": { "type": "string" }
                },
                "additionalProperties": true
            }
        }),
        json!({
            "name": "adesh.recall_relevant_memory",
            "description": "Return focused scoped memory for a query without full task-context assembly.",
            "inputSchema": {
                "type": "object",
                "required": ["workspace", "query"],
                "properties": {
                    "workspace": { "type": "object" },
                    "query": { "type": "string" },
                    "task_hint": { "type": ["string", "null"] },
                    "memory_types": { "type": "array", "items": { "type": "string" } },
                    "limit": { "type": ["integer", "null"] }
                },
                "additionalProperties": true
            }
        }),
        json!({
            "name": "adesh.connector_event",
            "description": "Normalize host lifecycle events into prepare/store cognition calls.",
            "inputSchema": {
                "type": "object",
                "required": ["connector_id", "connector_kind", "event_kind", "workspace", "task_prompt"],
                "properties": {
                    "connector_id": { "type": "string" },
                    "connector_kind": { "type": "string" },
                    "host_agent_id": { "type": ["string", "null"] },
                    "host_agent_kind": { "type": ["string", "null"] },
                    "host_model": { "type": ["string", "null"] },
                    "context_id": { "type": ["string", "null"] },
                    "selected_next_direction": { "type": ["string", "null"] },
                    "outcome": { "type": ["string", "null"] },
                    "correction_summary": { "type": ["string", "null"] },
                    "event_kind": { "type": "string", "enum": ["task_start", "task_checkpoint", "task_end"] },
                    "workspace": { "type": "object" },
                    "task_prompt": { "type": "string" }
                },
                "additionalProperties": true
            }
        }),
    ]
}

fn mcp_result_response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn mcp_error_response(id: Value, code: i64, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

async fn read_mcp_payload<R>(reader: &mut BufReader<R>) -> anyhow::Result<Option<Vec<u8>>>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = Vec::new();
        let bytes_read = reader
            .read_until(b'\n', &mut line)
            .await
            .context("failed to read MCP header")?;

        if bytes_read == 0 {
            if content_length.is_none() {
                return Ok(None);
            }
            bail!("unexpected EOF while reading MCP headers");
        }

        if line == b"\r\n" {
            break;
        }

        let header = String::from_utf8(line).context("MCP header is not valid UTF-8")?;
        let header = header.trim();
        if let Some(value) = header.strip_prefix("Content-Length:") {
            let parsed = value
                .trim()
                .parse::<usize>()
                .context("invalid Content-Length value")?;
            content_length = Some(parsed);
        }
    }

    let Some(length) = content_length else {
        bail!("missing Content-Length header");
    };

    let mut payload = vec![0_u8; length];
    reader
        .read_exact(&mut payload)
        .await
        .context("failed to read MCP payload body")?;

    Ok(Some(payload))
}

async fn write_mcp_payload<W>(writer: &mut W, value: &Value) -> anyhow::Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let payload = serde_json::to_vec(value).context("failed to serialize MCP payload")?;
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n", payload.len()).as_bytes())
        .await
        .context("failed to write MCP header")?;
    writer
        .write_all(&payload)
        .await
        .context("failed to write MCP payload")?;
    writer
        .flush()
        .await
        .context("failed to flush MCP payload")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use adesh_contracts::{StoreWorkEpisodeRequest, WorkspaceDescriptor};
    use adesh_core::ports::storage::StorageProvider;
    use adesh_storage_sqlite::SqliteStorage;
    use serde_json::{Value, json};

    use super::{MCP_PROTOCOL_VERSION, handle_mcp_request};

    #[tokio::test]
    async fn initialize_returns_tools_capability() {
        let storage = SqliteStorage::connect("sqlite::memory:").await.unwrap();
        storage.migrate().await.unwrap();

        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05"
            }
        });
        let response = handle_mcp_request(&storage, None, &request)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response["result"]["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert!(response["result"]["capabilities"]["tools"].is_object());
    }

    #[tokio::test]
    async fn tools_list_includes_cognition_tools() {
        let storage = SqliteStorage::connect("sqlite::memory:").await.unwrap();
        storage.migrate().await.unwrap();

        let request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });
        let response = handle_mcp_request(&storage, None, &request)
            .await
            .unwrap()
            .unwrap();
        let tools = response["result"]["tools"].as_array().unwrap();
        let names = tools
            .iter()
            .filter_map(|item| item.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert!(names.contains(&"adesh.prepare_task_context"));
        assert!(names.contains(&"adesh.store_work_episode"));
        assert!(names.contains(&"adesh.recall_relevant_memory"));
        assert!(names.contains(&"adesh.connector_event"));
    }

    #[tokio::test]
    async fn tools_call_prepare_returns_structured_content() {
        let storage = SqliteStorage::connect("sqlite::memory:").await.unwrap();
        storage.migrate().await.unwrap();

        let store = StoreWorkEpisodeRequest {
            workspace: WorkspaceDescriptor {
                kind: "git".to_string(),
                locator: Some("/tmp/example".to_string()),
                cwd: Some("/tmp/example".to_string()),
                branch: Some("main".to_string()),
                external_ref: Some("git@github.com:example/repo.git".to_string()),
            },
            task_prompt: "Investigate flaky retry tests".to_string(),
            summary: "Retry tests were flaky due to shared fixtures".to_string(),
            files_touched: vec!["tests/retry.rs".to_string()],
            tests: vec![],
            decisions: vec![],
            unresolved_items: vec!["Need fixture isolation".to_string()],
            observed_preferences: vec![],
            risk_signals: vec![],
            issue_refs: vec![],
            artifact_refs: vec![],
            task_hint: Some("retry".to_string()),
            started_at: None,
            ended_at: None,
        };
        crate::cognition::store_work_episode(&storage, store)
            .await
            .unwrap();

        let request = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "adesh.prepare_task_context",
                "arguments": {
                    "workspace": {
                        "kind": "git",
                        "locator": "/tmp/example",
                        "cwd": "/tmp/example",
                        "branch": "main",
                        "external_ref": "git@github.com:example/repo.git"
                    },
                    "task_prompt": "How should I continue on flaky retry tests?",
                    "files_in_focus": ["tests/retry.rs"],
                    "task_hint": "retry"
                }
            }
        });
        let response = handle_mcp_request(&storage, None, &request)
            .await
            .unwrap()
            .unwrap();
        assert!(response["result"]["structuredContent"].is_object());
        assert!(response["result"]["content"].is_array());
    }

    #[tokio::test]
    async fn tools_call_connector_event_maps_lifecycle() {
        let storage = SqliteStorage::connect("sqlite::memory:").await.unwrap();
        storage.migrate().await.unwrap();

        let request = json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "adesh.connector_event",
                "arguments": {
                    "connector_id": "codex-vscode",
                    "connector_kind": "chat_extension",
                    "connector_version": "0.1.0",
                    "session_id": "sess-connector-1",
                    "event_kind": "task_end",
                    "workspace": {
                        "kind": "task_space",
                        "locator": "workspace://mcp-connector-smoke",
                        "cwd": null,
                        "branch": null,
                        "external_ref": null
                    },
                    "task_prompt": "Finalize retry hardening",
                    "task_hint": "retry-hardening",
                    "summary": "Stored via connector_event MCP tool",
                    "files_touched": ["src/retry.rs"],
                    "decisions": [
                        {"decision": "Keep retry state explicit", "rationale": "Auditability"}
                    ],
                    "unresolved_items": ["Need timeout benchmark"],
                    "risk_signals": ["Without benchmark, confidence is weak"]
                }
            }
        });
        let response = handle_mcp_request(&storage, None, &request)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            response["result"]["structuredContent"]["handled_as"],
            "store_work_episode"
        );
        assert!(
            response["result"]["structuredContent"]["stored_episode"]["episode_id"].is_string()
        );
    }

    #[tokio::test]
    async fn tools_call_rejects_unknown_tool() {
        let storage = SqliteStorage::connect("sqlite::memory:").await.unwrap();
        storage.migrate().await.unwrap();

        let request = json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "adesh.unknown_tool",
                "arguments": {}
            }
        });
        let response = handle_mcp_request(&storage, None, &request)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response["error"]["code"], -32602);
        assert!(
            response["error"]["message"]
                .as_str()
                .unwrap()
                .contains("unsupported MCP tool")
        );
    }
}
