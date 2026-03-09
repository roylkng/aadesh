use serde_json::{Value, json};

use crate::StorageError;

const CAP_BOOTSTRAP_JSON: &str =
    include_str!("../../../registry/bootstrap/capability_snapshots/cap_bootstrap.json");
const EMAIL_SEND_ARGS_SCHEMA_ENTRY_JSON: &str =
    include_str!("../../../registry/bootstrap/schema_registry/email_send_args.json");
const EMAIL_SEND_RESULT_SCHEMA_ENTRY_JSON: &str =
    include_str!("../../../registry/bootstrap/schema_registry/email_send_result.json");
const WEBHOOK_POST_JSON_ARGS_SCHEMA_ENTRY_JSON: &str =
    include_str!("../../../registry/bootstrap/schema_registry/webhook_post_json_args.json");
const WEBHOOK_POST_JSON_RESULT_SCHEMA_ENTRY_JSON: &str =
    include_str!("../../../registry/bootstrap/schema_registry/webhook_post_json_result.json");

#[derive(Debug, Clone)]
pub struct ActionDescriptor {
    pub tool_name: String,
    pub action_name: String,
    pub args_schema_ref: String,
    pub result_schema_ref: Option<String>,
    pub diff_supported: bool,
    pub execution_class: String,
    pub default_approval_mode: String,
    pub diff_kind: String,
    pub editable_payload_schema: Value,
}

pub const EMAIL_SEND_ARGS_SCHEMA_REF: &str = "schema:sha256:adesh-email-send-payload-v0_1";
pub const EMAIL_SEND_RESULT_SCHEMA_REF: &str = "schema:sha256:adesh-email-send-result-v0_1";
pub const WEBHOOK_POST_JSON_ARGS_SCHEMA_REF: &str =
    "schema:sha256:adesh-webhook-post-json-args-v0_1";
pub const WEBHOOK_POST_JSON_RESULT_SCHEMA_REF: &str =
    "schema:sha256:adesh-webhook-post-json-result-v0_1";

pub fn email_send_args_schema() -> Value {
    parse_bootstrap_schema_entry(EMAIL_SEND_ARGS_SCHEMA_ENTRY_JSON)
        .and_then(|entry| entry.get("payload_json").cloned())
        .unwrap_or_else(|| panic!("bootstrap email send args schema entry is invalid"))
}

pub fn email_send_result_schema() -> Value {
    parse_bootstrap_schema_entry(EMAIL_SEND_RESULT_SCHEMA_ENTRY_JSON)
        .and_then(|entry| entry.get("payload_json").cloned())
        .unwrap_or_else(|| panic!("bootstrap email send result schema entry is invalid"))
}

pub fn bootstrap_schema_registry_entries() -> Vec<(String, Value)> {
    [
        EMAIL_SEND_ARGS_SCHEMA_ENTRY_JSON,
        EMAIL_SEND_RESULT_SCHEMA_ENTRY_JSON,
        WEBHOOK_POST_JSON_ARGS_SCHEMA_ENTRY_JSON,
        WEBHOOK_POST_JSON_RESULT_SCHEMA_ENTRY_JSON,
    ]
    .into_iter()
    .map(|raw| {
        let entry = parse_bootstrap_schema_entry(raw)
            .unwrap_or_else(|| panic!("bootstrap schema registry entry is invalid"));
        let schema_ref = entry
            .get("schema_ref")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("bootstrap schema registry entry missing schema_ref"))
            .to_string();
        (schema_ref, entry)
    })
    .collect()
}

pub fn bootstrap_capability_snapshot(version: &str) -> Value {
    let mut snapshot = serde_json::from_str::<Value>(CAP_BOOTSTRAP_JSON)
        .unwrap_or_else(|_| panic!("bootstrap capability snapshot JSON is invalid"));
    if let Some(object) = snapshot.as_object_mut() {
        object.insert(
            "capability_snapshot_version".to_string(),
            Value::String(version.to_string()),
        );
    }
    snapshot
}

pub fn webhook_post_json_descriptor() -> ActionDescriptor {
    resolve_action_descriptor_from_snapshot(
        &bootstrap_capability_snapshot("cap:bootstrap"),
        "webhook",
        "post_json",
    )
    .unwrap_or_else(|_| panic!("bootstrap webhook.post_json descriptor is invalid"))
}

fn parse_bootstrap_schema_entry(raw: &str) -> Option<Value> {
    serde_json::from_str(raw).ok()
}

pub fn resolve_action_descriptor_from_snapshot(
    snapshot: &Value,
    tool_name: &str,
    action_name: &str,
) -> Result<ActionDescriptor, StorageError> {
    let capabilities = snapshot
        .get("capabilities")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            StorageError::Corruption("capability snapshot missing capabilities array".to_string())
        })?;

    for capability in capabilities {
        let snapshot_tool_name = capability
            .get("tool_name")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                StorageError::Corruption(
                    "capability snapshot capability missing tool_name".to_string(),
                )
            })?;
        if snapshot_tool_name != tool_name {
            continue;
        }

        let actions = capability
            .get("actions")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                StorageError::Corruption(
                    "capability snapshot capability missing actions array".to_string(),
                )
            })?;
        for action in actions {
            let snapshot_action_name = action
                .get("action_name")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    StorageError::Corruption(
                        "capability snapshot action missing action_name".to_string(),
                    )
                })?;
            if snapshot_action_name != action_name {
                continue;
            }

            return Ok(ActionDescriptor {
                tool_name: snapshot_tool_name.to_string(),
                action_name: snapshot_action_name.to_string(),
                args_schema_ref: action
                    .get("args_schema_ref")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        StorageError::Corruption(
                            "capability snapshot action missing args_schema_ref".to_string(),
                        )
                    })?
                    .to_string(),
                result_schema_ref: action
                    .get("result_schema_ref")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                diff_supported: action
                    .get("diff_supported")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| {
                        StorageError::Corruption(
                            "capability snapshot action missing diff_supported".to_string(),
                        )
                    })?,
                execution_class: action
                    .get("execution_class")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        StorageError::Corruption(
                            "capability snapshot action missing execution_class".to_string(),
                        )
                    })?
                    .to_string(),
                default_approval_mode: action
                    .get("default_approval_mode")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        StorageError::Corruption(
                            "capability snapshot action missing default_approval_mode".to_string(),
                        )
                    })?
                    .to_string(),
                diff_kind: action
                    .get("diff_kind")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        StorageError::Corruption(
                            "capability snapshot action missing diff_kind".to_string(),
                        )
                    })?
                    .to_string(),
                editable_payload_schema: action
                    .get("editable_payload_schema")
                    .cloned()
                    .ok_or_else(|| {
                        StorageError::Corruption(
                            "capability snapshot action missing editable_payload_schema"
                                .to_string(),
                        )
                    })?,
            });
        }
    }

    Err(StorageError::InvalidInput(format!(
        "unknown tool/action {tool_name}/{action_name}"
    )))
}

pub fn email_send_descriptor() -> ActionDescriptor {
    ActionDescriptor {
        tool_name: "email".to_string(),
        action_name: "send".to_string(),
        args_schema_ref: EMAIL_SEND_ARGS_SCHEMA_REF.to_string(),
        result_schema_ref: Some(EMAIL_SEND_RESULT_SCHEMA_REF.to_string()),
        diff_supported: true,
        execution_class: "external_api".to_string(),
        default_approval_mode: "diff".to_string(),
        diff_kind: "email_send_payload".to_string(),
        editable_payload_schema: email_send_args_schema(),
    }
}

pub fn default_email_send_payload(body: &str) -> Value {
    json!({
        "to": ["user@example.com"],
        "cc": [],
        "bcc": [],
        "subject": "Drafted subject",
        "body": body.trim(),
    })
}

pub fn approval_diff_payload_for_action(
    descriptor: &ActionDescriptor,
    proposal_args: &Value,
) -> Value {
    json!({
        "kind": descriptor.diff_kind,
        "tool_id": descriptor.tool_name,
        "action": descriptor.action_name,
        "args_schema_ref": descriptor.args_schema_ref,
        "result_schema_ref": descriptor.result_schema_ref,
        "before": null,
        "after": proposal_args,
        "current_args": proposal_args,
        "editable_payload_schema": descriptor.editable_payload_schema,
    })
}

pub fn normalize_args_for_action(
    tool_name: &str,
    action_name: &str,
    payload: &Value,
) -> Result<Value, StorageError> {
    match (tool_name, action_name) {
        ("email", "send") => normalize_email_send_payload(payload),
        ("webhook", "post_json") => normalize_webhook_post_json_payload(payload),
        _ => Err(StorageError::InvalidInput(format!(
            "unknown tool/action {tool_name}/{action_name}"
        ))),
    }
}

pub fn validate_result_for_action(
    tool_name: &str,
    action_name: &str,
    result: &Value,
) -> Result<(), StorageError> {
    match (tool_name, action_name) {
        ("email", "send") => validate_email_send_result(result),
        ("webhook", "post_json") => validate_webhook_post_json_result(result),
        _ => Err(StorageError::InvalidInput(format!(
            "unknown tool/action {tool_name}/{action_name}"
        ))),
    }
}

pub fn validate_instance_against_schema(
    schema: &Value,
    instance: &Value,
    error_kind: ValidationErrorKind,
) -> Result<(), StorageError> {
    validate_schema_node("$", schema, instance, error_kind)
}

#[derive(Debug, Clone, Copy)]
pub enum ValidationErrorKind {
    InvalidInput,
    Corruption,
}

fn normalize_email_send_payload(payload: &Value) -> Result<Value, StorageError> {
    let object = payload.as_object().ok_or_else(|| {
        StorageError::InvalidInput("modified_payload must be an object".to_string())
    })?;

    let mut normalized = serde_json::Map::new();
    for field in ["to", "cc", "bcc"] {
        let values = object.get(field).ok_or_else(|| {
            StorageError::InvalidInput(format!("modified_payload missing field `{field}`"))
        })?;
        let array = values.as_array().ok_or_else(|| {
            StorageError::InvalidInput(format!("modified_payload field `{field}` must be an array"))
        })?;
        let mut deduped = Vec::new();
        for item in array {
            let normalized_value = item
                .as_str()
                .map(|value| value.trim().to_lowercase())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    StorageError::InvalidInput(format!(
                        "modified_payload field `{field}` must contain non-empty strings"
                    ))
                })?;
            if !deduped
                .iter()
                .any(|existing: &Value| existing == &json!(normalized_value))
            {
                deduped.push(json!(normalized_value));
            }
        }
        if field == "to" && deduped.is_empty() {
            return Err(StorageError::InvalidInput(
                "modified_payload field `to` must contain at least one recipient".to_string(),
            ));
        }
        normalized.insert(field.to_string(), Value::Array(deduped));
    }

    for field in ["subject", "body"] {
        let normalized_value = object
            .get(field)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                StorageError::InvalidInput(format!(
                    "modified_payload field `{field}` must be a non-empty string"
                ))
            })?;
        normalized.insert(field.to_string(), json!(normalized_value));
    }

    Ok(Value::Object(normalized))
}

fn normalize_webhook_post_json_payload(payload: &Value) -> Result<Value, StorageError> {
    let object = payload.as_object().ok_or_else(|| {
        StorageError::InvalidInput("modified_payload must be an object".to_string())
    })?;
    let url = object
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            StorageError::InvalidInput(
                "modified_payload field `url` must be a non-empty string".to_string(),
            )
        })?;
    let payload_json = object
        .get("payload")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| {
            StorageError::InvalidInput(
                "modified_payload field `payload` must be an object".to_string(),
            )
        })?;

    let mut normalized = serde_json::Map::new();
    normalized.insert("url".to_string(), Value::String(url.to_string()));
    normalized.insert("payload".to_string(), Value::Object(payload_json));

    if let Some(headers) = object.get("headers") {
        let headers = headers.as_object().cloned().ok_or_else(|| {
            StorageError::InvalidInput(
                "modified_payload field `headers` must be an object when present".to_string(),
            )
        })?;
        normalized.insert("headers".to_string(), Value::Object(headers));
    }

    Ok(Value::Object(normalized))
}

fn validate_email_send_result(result: &Value) -> Result<(), StorageError> {
    let object = result.as_object().ok_or_else(|| {
        StorageError::Corruption("email send result must be a JSON object".to_string())
    })?;

    require_string_field(object, "provider", true)?;
    require_string_field(object, "delivery_state", true)?;
    require_string_field(object, "from_address", true)?;
    object
        .get("recipient_count")
        .and_then(Value::as_i64)
        .ok_or_else(|| {
            StorageError::Corruption(
                "email send result missing integer recipient_count".to_string(),
            )
        })?;
    object
        .get("idempotency_supported")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            StorageError::Corruption(
                "email send result missing boolean idempotency_supported".to_string(),
            )
        })?;

    if let Some(value) = object.get("provider_message_id") {
        if !value.is_null() && value.as_str().is_none() {
            return Err(StorageError::Corruption(
                "email send result provider_message_id must be string or null".to_string(),
            ));
        }
    }

    Ok(())
}

fn validate_webhook_post_json_result(result: &Value) -> Result<(), StorageError> {
    let object = result.as_object().ok_or_else(|| {
        StorageError::Corruption("webhook post_json result must be a JSON object".to_string())
    })?;
    require_string_field(object, "provider", true)?;
    require_string_field(object, "delivery_state", true)?;
    require_string_field(object, "url", true)?;
    object
        .get("status_code")
        .and_then(Value::as_i64)
        .ok_or_else(|| {
            StorageError::Corruption(
                "webhook post_json result missing integer status_code".to_string(),
            )
        })?;
    object
        .get("idempotency_supported")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            StorageError::Corruption(
                "webhook post_json result missing boolean idempotency_supported".to_string(),
            )
        })?;

    Ok(())
}

fn validate_schema_node(
    path: &str,
    schema: &Value,
    instance: &Value,
    error_kind: ValidationErrorKind,
) -> Result<(), StorageError> {
    if let Some(schema_type) = schema.get("type") {
        validate_type(path, schema_type, instance, error_kind)?;
    }

    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        let object = instance.as_object().ok_or_else(|| {
            validation_error(
                error_kind,
                format!("{path} must be an object for required-field validation"),
            )
        })?;
        for field in required {
            let field_name = field.as_str().ok_or_else(|| {
                validation_error(
                    ValidationErrorKind::Corruption,
                    format!("{path} schema has non-string required entry"),
                )
            })?;
            if !object.contains_key(field_name) {
                return Err(validation_error(
                    error_kind,
                    format!("{path} missing required field `{field_name}`"),
                ));
            }
        }
    }

    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        let object = instance.as_object().ok_or_else(|| {
            validation_error(
                error_kind,
                format!("{path} must be an object for property validation"),
            )
        })?;

        let additional_properties_allowed = schema
            .get("additionalProperties")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        if !additional_properties_allowed {
            for field_name in object.keys() {
                if !properties.contains_key(field_name) {
                    return Err(validation_error(
                        error_kind,
                        format!("{path} contains unknown field `{field_name}`"),
                    ));
                }
            }
        }

        for (field_name, field_schema) in properties {
            if let Some(field_value) = object.get(field_name) {
                validate_schema_node(
                    &format!("{path}.{field_name}"),
                    field_schema,
                    field_value,
                    error_kind,
                )?;
            }
        }
    }

    if let Some(items_schema) = schema.get("items") {
        let items = instance.as_array().ok_or_else(|| {
            validation_error(
                error_kind,
                format!("{path} must be an array for item validation"),
            )
        })?;
        for (index, item) in items.iter().enumerate() {
            validate_schema_node(&format!("{path}[{index}]"), items_schema, item, error_kind)?;
        }
    }

    Ok(())
}

fn validate_type(
    path: &str,
    schema_type: &Value,
    instance: &Value,
    error_kind: ValidationErrorKind,
) -> Result<(), StorageError> {
    let allowed_types = if let Some(single) = schema_type.as_str() {
        vec![single.to_string()]
    } else if let Some(many) = schema_type.as_array() {
        many.iter()
            .map(|value| {
                value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                    validation_error(
                        ValidationErrorKind::Corruption,
                        format!("{path} schema contains non-string type entry"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        return Err(validation_error(
            ValidationErrorKind::Corruption,
            format!("{path} schema type must be string or array of strings"),
        ));
    };

    let matches = allowed_types
        .iter()
        .any(|expected| instance_matches_type(expected, instance));
    if matches {
        Ok(())
    } else {
        Err(validation_error(
            error_kind,
            format!(
                "{path} does not match allowed types {}",
                allowed_types.join("|")
            ),
        ))
    }
}

fn instance_matches_type(expected: &str, instance: &Value) -> bool {
    match expected {
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "integer" => instance.as_i64().is_some() || instance.as_u64().is_some(),
        "boolean" => instance.is_boolean(),
        "null" => instance.is_null(),
        _ => false,
    }
}

fn validation_error(kind: ValidationErrorKind, message: String) -> StorageError {
    match kind {
        ValidationErrorKind::InvalidInput => StorageError::InvalidInput(message),
        ValidationErrorKind::Corruption => StorageError::Corruption(message),
    }
}

fn require_string_field(
    object: &serde_json::Map<String, Value>,
    field: &str,
    non_empty: bool,
) -> Result<(), StorageError> {
    let value = object.get(field).and_then(Value::as_str).ok_or_else(|| {
        StorageError::Corruption(format!("email send result missing string {field}"))
    })?;
    if non_empty && value.trim().is_empty() {
        return Err(StorageError::Corruption(format!(
            "email send result field {field} must be non-empty"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        EMAIL_SEND_ARGS_SCHEMA_REF, EMAIL_SEND_RESULT_SCHEMA_REF, ValidationErrorKind,
        WEBHOOK_POST_JSON_ARGS_SCHEMA_REF, approval_diff_payload_for_action,
        bootstrap_capability_snapshot, default_email_send_payload, email_send_args_schema,
        email_send_descriptor, email_send_result_schema, normalize_args_for_action,
        resolve_action_descriptor_from_snapshot, validate_instance_against_schema,
        validate_result_for_action, webhook_post_json_descriptor,
    };
    use serde_json::json;

    #[test]
    fn email_descriptor_exposes_schema_refs() {
        let descriptor = email_send_descriptor();
        assert_eq!(descriptor.args_schema_ref, EMAIL_SEND_ARGS_SCHEMA_REF);
        assert_eq!(
            descriptor.result_schema_ref,
            Some(EMAIL_SEND_RESULT_SCHEMA_REF.to_string())
        );
    }

    #[test]
    fn bootstrap_snapshot_resolves_email_send_descriptor() {
        let snapshot = bootstrap_capability_snapshot("cap:bootstrap");
        let descriptor =
            resolve_action_descriptor_from_snapshot(&snapshot, "email", "send").unwrap();
        assert_eq!(descriptor.tool_name, "email");
        assert_eq!(descriptor.action_name, "send");
        assert_eq!(descriptor.args_schema_ref, EMAIL_SEND_ARGS_SCHEMA_REF);
    }

    #[test]
    fn bootstrap_snapshot_resolves_webhook_descriptor() {
        let descriptor = webhook_post_json_descriptor();
        assert_eq!(descriptor.tool_name, "webhook");
        assert_eq!(descriptor.action_name, "post_json");
        assert_eq!(
            descriptor.args_schema_ref,
            WEBHOOK_POST_JSON_ARGS_SCHEMA_REF
        );
    }

    #[test]
    fn diff_payload_carries_action_metadata() {
        let descriptor = email_send_descriptor();
        let diff = approval_diff_payload_for_action(&descriptor, &default_email_send_payload("hi"));
        assert_eq!(diff["tool_id"], "email");
        assert_eq!(diff["action"], "send");
        assert_eq!(diff["args_schema_ref"], EMAIL_SEND_ARGS_SCHEMA_REF);
        assert_eq!(diff["result_schema_ref"], EMAIL_SEND_RESULT_SCHEMA_REF);
    }

    #[test]
    fn normalized_email_payload_dedupes_and_trims() {
        let normalized = normalize_args_for_action(
            "email",
            "send",
            &json!({
                "to": [" A@example.com ", "a@example.com"],
                "cc": [],
                "bcc": [],
                "subject": " Hi ",
                "body": " Body "
            }),
        )
        .unwrap();

        assert_eq!(normalized["to"], json!(["a@example.com"]));
        assert_eq!(normalized["subject"], "Hi");
        assert_eq!(normalized["body"], "Body");
    }

    #[test]
    fn email_result_validation_requires_schema_shape() {
        let ok = json!({
            "provider": "smtp",
            "delivery_state": "accepted",
            "provider_message_id": "abc",
            "from_address": "sender@example.com",
            "recipient_count": 1,
            "idempotency_supported": false
        });
        validate_result_for_action("email", "send", &ok).unwrap();

        let err = validate_result_for_action("email", "send", &json!({"provider": "smtp"}))
            .unwrap_err()
            .to_string();
        assert!(err.contains("recipient_count") || err.contains("delivery_state"));
    }

    #[test]
    fn stored_args_schema_rejects_unknown_fields() {
        let err = validate_instance_against_schema(
            &email_send_args_schema(),
            &json!({
                "to": ["a@example.com"],
                "cc": [],
                "bcc": [],
                "subject": "Hello",
                "body": "World",
                "unexpected": true
            }),
            ValidationErrorKind::InvalidInput,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("unknown field"));
    }

    #[test]
    fn stored_result_schema_allows_nullable_provider_message_id() {
        validate_instance_against_schema(
            &email_send_result_schema(),
            &json!({
                "provider": "smtp",
                "delivery_state": "accepted",
                "provider_message_id": null,
                "from_address": "sender@example.com",
                "recipient_count": 1,
                "idempotency_supported": false
            }),
            ValidationErrorKind::Corruption,
        )
        .unwrap();
    }
}
