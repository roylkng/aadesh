use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::Utc;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    transport::smtp::authentication::Credentials,
};
use reqwest::{
    Client,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde_json::{Value, json};

use adesh_core::{
    AppConfig, StorageError,
    action_schemas::{ActionDescriptor, email_send_descriptor, webhook_post_json_descriptor},
    ports::tool::{ToolExecutionResult, ToolProvider},
};

#[async_trait]
trait ActionExecutor: Send + Sync {
    fn descriptor(&self) -> ActionDescriptor;
    async fn health(&self) -> Result<(), StorageError>;
    async fn execute(
        &self,
        syscall_id: &str,
        args: &Value,
    ) -> Result<ToolExecutionResult, StorageError>;
}

pub struct RoutedToolProvider {
    actions: HashMap<(String, String), Arc<dyn ActionExecutor>>,
}

impl RoutedToolProvider {
    fn new(actions: HashMap<(String, String), Arc<dyn ActionExecutor>>) -> Self {
        Self { actions }
    }
}

#[async_trait]
impl ToolProvider for RoutedToolProvider {
    async fn health(&self) -> Result<(), StorageError> {
        for executor in self.actions.values() {
            executor.health().await?;
        }
        Ok(())
    }

    async fn execute_syscall(
        &self,
        syscall_id: &str,
        tool_name: &str,
        action_name: &str,
        args_schema_ref: &str,
        result_schema_ref: Option<&str>,
        args: &Value,
    ) -> Result<ToolExecutionResult, StorageError> {
        let executor = self
            .actions
            .get(&(tool_name.to_string(), action_name.to_string()))
            .ok_or_else(|| {
                StorageError::InvalidInput(format!(
                    "unsupported tool/action {tool_name}/{action_name}"
                ))
            })?;
        let descriptor = executor.descriptor();
        if descriptor.args_schema_ref != args_schema_ref {
            return Err(StorageError::InvalidInput(format!(
                "args schema ref mismatch for {tool_name}/{action_name}: expected {}, got {args_schema_ref}",
                descriptor.args_schema_ref
            )));
        }
        if descriptor.result_schema_ref.as_deref() != result_schema_ref {
            return Err(StorageError::InvalidInput(format!(
                "result schema ref mismatch for {tool_name}/{action_name}"
            )));
        }

        executor.execute(syscall_id, args).await
    }
}

struct FakeEmailSendExecutor;

#[async_trait]
impl ActionExecutor for FakeEmailSendExecutor {
    fn descriptor(&self) -> ActionDescriptor {
        email_send_descriptor()
    }

    async fn health(&self) -> Result<(), StorageError> {
        Ok(())
    }

    async fn execute(
        &self,
        syscall_id: &str,
        args: &Value,
    ) -> Result<ToolExecutionResult, StorageError> {
        let started_at = Utc::now();
        let recipients = args
            .get("to")
            .and_then(Value::as_array)
            .map(|values| values.len())
            .unwrap_or(0);
        let ended_at = Utc::now();

        Ok(ToolExecutionResult {
            ok: true,
            output_kind: "json".to_string(),
            output_json: json!({
                "provider": "fake_email",
                "delivery_state": "accepted",
                "from_address": "adesh@example.invalid",
                "recipient_count": recipients,
                "idempotency_supported": true,
                "idempotency_token": syscall_id,
            }),
            content_ref: None,
            sensitivity_s: 1,
            taint_s: 1,
            started_at,
            ended_at,
            attempts_used: 1,
        })
    }
}

struct FakeWebhookPostJsonExecutor;

#[async_trait]
impl ActionExecutor for FakeWebhookPostJsonExecutor {
    fn descriptor(&self) -> ActionDescriptor {
        webhook_post_json_descriptor()
    }

    async fn health(&self) -> Result<(), StorageError> {
        Ok(())
    }

    async fn execute(
        &self,
        syscall_id: &str,
        args: &Value,
    ) -> Result<ToolExecutionResult, StorageError> {
        let url = args
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                StorageError::InvalidInput("webhook post_json requires non-empty url".to_string())
            })?;
        let started_at = Utc::now();
        let ended_at = Utc::now();

        Ok(ToolExecutionResult {
            ok: true,
            output_kind: "json".to_string(),
            output_json: json!({
                "provider": "fake_webhook",
                "delivery_state": "accepted",
                "status_code": 202,
                "url": url,
                "idempotency_supported": true,
                "syscall_id": syscall_id,
            }),
            content_ref: None,
            sensitivity_s: 1,
            taint_s: 1,
            started_at,
            ended_at,
            attempts_used: 1,
        })
    }
}

struct HttpWebhookPostJsonExecutor {
    client: Client,
}

impl HttpWebhookPostJsonExecutor {
    fn new() -> Self {
        Self {
            client: Client::builder()
                .build()
                .expect("http webhook client should build"),
        }
    }
}

#[async_trait]
impl ActionExecutor for HttpWebhookPostJsonExecutor {
    fn descriptor(&self) -> ActionDescriptor {
        webhook_post_json_descriptor()
    }

    async fn health(&self) -> Result<(), StorageError> {
        Ok(())
    }

    async fn execute(
        &self,
        syscall_id: &str,
        args: &Value,
    ) -> Result<ToolExecutionResult, StorageError> {
        let url = args
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                StorageError::InvalidInput("webhook post_json requires non-empty url".to_string())
            })?;
        let payload = args.get("payload").cloned().ok_or_else(|| {
            StorageError::InvalidInput("webhook post_json missing payload".to_string())
        })?;
        let headers = parse_webhook_headers(args.get("headers"))?;

        let started_at = Utc::now();
        let response = self
            .client
            .post(url)
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .map_err(|err| StorageError::Unavailable(format!("webhook post failed: {err}")))?;
        let status_code = i64::from(response.status().as_u16());
        let ended_at = Utc::now();

        Ok(ToolExecutionResult {
            ok: response.status().is_success(),
            output_kind: "json".to_string(),
            output_json: json!({
                "provider": "http_webhook",
                "delivery_state": if response.status().is_success() { "accepted" } else { "failed" },
                "status_code": status_code,
                "url": url,
                "idempotency_supported": false,
                "syscall_id": syscall_id,
            }),
            content_ref: None,
            sensitivity_s: 1,
            taint_s: 1,
            started_at,
            ended_at,
            attempts_used: 1,
        })
    }
}

#[async_trait]
trait EmailSender: Send + Sync {
    async fn health(&self) -> Result<(), StorageError>;
    async fn send(
        &self,
        from_address: &str,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: &str,
        body: &str,
        syscall_id: &str,
    ) -> Result<EmailSendOutcome, StorageError>;
}

#[derive(Debug, Clone)]
struct EmailSendOutcome {
    provider: String,
    delivery_state: String,
    provider_message_id: Option<String>,
    from_address: String,
    recipient_count: usize,
    idempotency_supported: bool,
}

struct SmtpEmailSendExecutor {
    from_address: String,
    sender: Arc<dyn EmailSender>,
}

impl SmtpEmailSendExecutor {
    fn new(from_address: String, sender: Arc<dyn EmailSender>) -> Result<Self, StorageError> {
        validate_email_address(&from_address)?;
        Ok(Self {
            from_address,
            sender,
        })
    }
}

#[async_trait]
impl ActionExecutor for SmtpEmailSendExecutor {
    fn descriptor(&self) -> ActionDescriptor {
        email_send_descriptor()
    }

    async fn health(&self) -> Result<(), StorageError> {
        self.sender.health().await
    }

    async fn execute(
        &self,
        syscall_id: &str,
        args: &Value,
    ) -> Result<ToolExecutionResult, StorageError> {
        let to = parse_recipient_field(args, "to")?;
        let cc = parse_recipient_field(args, "cc")?;
        let bcc = parse_recipient_field(args, "bcc")?;
        let subject = parse_required_string(args, "subject")?;
        let body = parse_required_string(args, "body")?;
        if to.is_empty() {
            return Err(StorageError::InvalidInput(
                "email send requires at least one recipient in to".to_string(),
            ));
        }

        let started_at = Utc::now();
        let outcome = self
            .sender
            .send(
                &self.from_address,
                &to,
                &cc,
                &bcc,
                &subject,
                &body,
                syscall_id,
            )
            .await?;
        let output_json = json!({
            "provider": outcome.provider,
            "delivery_state": outcome.delivery_state,
            "provider_message_id": outcome.provider_message_id,
            "from_address": outcome.from_address,
            "recipient_count": outcome.recipient_count,
            "idempotency_supported": outcome.idempotency_supported,
            "syscall_id": syscall_id,
        });
        let ended_at = Utc::now();

        Ok(ToolExecutionResult {
            ok: true,
            output_kind: "json".to_string(),
            output_json,
            content_ref: None,
            sensitivity_s: 1,
            taint_s: 1,
            started_at,
            ended_at,
            attempts_used: 1,
        })
    }
}

struct SmtpEmailSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

#[async_trait]
impl EmailSender for SmtpEmailSender {
    async fn health(&self) -> Result<(), StorageError> {
        self.transport
            .test_connection()
            .await
            .map_err(|err| StorageError::Unavailable(format!("smtp health probe failed: {err}")))?;
        Ok(())
    }

    async fn send(
        &self,
        from_address: &str,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: &str,
        body: &str,
        _syscall_id: &str,
    ) -> Result<EmailSendOutcome, StorageError> {
        let mut builder = Message::builder().from(parse_mailbox(from_address)?);

        for address in to {
            builder = builder.to(parse_mailbox(address)?);
        }
        for address in cc {
            builder = builder.cc(parse_mailbox(address)?);
        }
        for address in bcc {
            builder = builder.bcc(parse_mailbox(address)?);
        }

        let message = builder
            .subject(subject)
            .body(body.to_string())
            .map_err(|err| {
                StorageError::InvalidInput(format!("failed to build smtp message: {err}"))
            })?;

        let send_result = self
            .transport
            .send(message)
            .await
            .map_err(|err| StorageError::Unavailable(format!("smtp send failed: {err}")))?;

        Ok(EmailSendOutcome {
            provider: "smtp".to_string(),
            delivery_state: "accepted".to_string(),
            provider_message_id: Some(send_result.message().collect::<Vec<_>>().join(" ")),
            from_address: from_address.to_string(),
            recipient_count: to.len() + cc.len() + bcc.len(),
            idempotency_supported: false,
        })
    }
}

pub fn build_tool_provider(config: &AppConfig) -> Result<Arc<dyn ToolProvider>, StorageError> {
    let email_executor: Arc<dyn ActionExecutor> = match config.email_provider_backend.as_str() {
        "fake" => Arc::new(FakeEmailSendExecutor),
        "smtp" => {
            let sender = Arc::new(build_smtp_sender(config));
            Arc::new(SmtpEmailSendExecutor::new(
                config.email_from_address.clone(),
                sender,
            )?)
        }
        other => {
            return Err(StorageError::InvalidInput(format!(
                "unknown email provider backend: {other}"
            )));
        }
    };

    let mut actions = HashMap::new();
    let email_descriptor = email_executor.descriptor();
    actions.insert(
        (
            email_descriptor.tool_name.clone(),
            email_descriptor.action_name.clone(),
        ),
        email_executor,
    );
    let webhook_executor: Arc<dyn ActionExecutor> = match config.webhook_provider_backend.as_str() {
        "fake" => Arc::new(FakeWebhookPostJsonExecutor),
        "http" => Arc::new(HttpWebhookPostJsonExecutor::new()),
        other => {
            return Err(StorageError::InvalidInput(format!(
                "unknown webhook provider backend: {other}"
            )));
        }
    };
    let webhook_descriptor = webhook_executor.descriptor();
    actions.insert(
        (
            webhook_descriptor.tool_name.clone(),
            webhook_descriptor.action_name.clone(),
        ),
        webhook_executor,
    );

    Ok(Arc::new(RoutedToolProvider::new(actions)))
}

fn build_smtp_sender(config: &AppConfig) -> SmtpEmailSender {
    let mut builder =
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.email_smtp_host)
            .port(config.email_smtp_port);

    if let (Some(username), Some(password)) = (
        config.email_smtp_username.clone(),
        config.email_smtp_password.clone(),
    ) {
        builder = builder.credentials(Credentials::new(username, password));
    }

    SmtpEmailSender {
        transport: builder.build(),
    }
}

fn parse_recipient_field(args: &Value, field: &str) -> Result<Vec<String>, StorageError> {
    let values = args.get(field).and_then(Value::as_array).ok_or_else(|| {
        StorageError::InvalidInput(format!("email send payload missing {field} array"))
    })?;

    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        let address = value.as_str().ok_or_else(|| {
            StorageError::InvalidInput(format!("{field} entries must be strings"))
        })?;
        validate_email_address(address)?;
        parsed.push(address.to_string());
    }
    Ok(parsed)
}

fn parse_required_string(args: &Value, field: &str) -> Result<String, StorageError> {
    let value = args
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| StorageError::InvalidInput(format!("email send payload missing {field}")))?;
    Ok(value.to_string())
}

fn parse_webhook_headers(value: Option<&Value>) -> Result<HeaderMap, StorageError> {
    let Some(value) = value else {
        return Ok(HeaderMap::new());
    };
    let object = value.as_object().ok_or_else(|| {
        StorageError::InvalidInput("webhook headers must be an object".to_string())
    })?;

    let mut headers = HeaderMap::new();
    for (key, value) in object {
        let value = value.as_str().ok_or_else(|| {
            StorageError::InvalidInput("webhook header values must be strings".to_string())
        })?;
        let name = HeaderName::from_bytes(key.as_bytes()).map_err(|err| {
            StorageError::InvalidInput(format!("invalid webhook header name {key}: {err}"))
        })?;
        let value = HeaderValue::from_str(value).map_err(|err| {
            StorageError::InvalidInput(format!("invalid webhook header value for {key}: {err}"))
        })?;
        headers.insert(name, value);
    }
    Ok(headers)
}

fn validate_email_address(value: &str) -> Result<(), StorageError> {
    let _ = parse_mailbox(value)?;
    Ok(())
}

fn parse_mailbox(value: &str) -> Result<lettre::message::Mailbox, StorageError> {
    value
        .parse()
        .map_err(|err| StorageError::InvalidInput(format!("invalid email address {value}: {err}")))
}

#[cfg(test)]
mod tests {
    use super::{
        ActionExecutor, EmailSendOutcome, EmailSender, RoutedToolProvider, SmtpEmailSendExecutor,
        build_tool_provider, parse_mailbox,
    };
    use adesh_core::{
        AppConfig, StorageError,
        action_schemas::{
            EMAIL_SEND_ARGS_SCHEMA_REF, EMAIL_SEND_RESULT_SCHEMA_REF,
            WEBHOOK_POST_JSON_ARGS_SCHEMA_REF, WEBHOOK_POST_JSON_RESULT_SCHEMA_REF,
        },
        ports::tool::ToolProvider,
    };
    use async_trait::async_trait;
    use axum::{Json, Router, routing::post};
    use http::StatusCode;
    use serde_json::json;
    use std::{collections::HashMap, sync::Arc};
    use tokio::net::TcpListener;

    #[derive(Default)]
    struct StubEmailSender {
        health_ok: bool,
    }

    #[async_trait]
    impl EmailSender for StubEmailSender {
        async fn health(&self) -> Result<(), StorageError> {
            if self.health_ok {
                Ok(())
            } else {
                Err(StorageError::Unavailable(
                    "stub smtp unavailable".to_string(),
                ))
            }
        }

        async fn send(
            &self,
            from_address: &str,
            to: &[String],
            cc: &[String],
            bcc: &[String],
            _subject: &str,
            _body: &str,
            _syscall_id: &str,
        ) -> Result<EmailSendOutcome, StorageError> {
            Ok(EmailSendOutcome {
                provider: "smtp".to_string(),
                delivery_state: "accepted".to_string(),
                provider_message_id: Some("msg-1".to_string()),
                from_address: from_address.to_string(),
                recipient_count: to.len() + cc.len() + bcc.len(),
                idempotency_supported: false,
            })
        }
    }

    fn smtp_config() -> AppConfig {
        AppConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            root_owner_token: "test-token".to_string(),
            database_url: "sqlite::memory:".to_string(),
            server_version: "test".to_string(),
            capability_snapshot_version: "cap:bootstrap".to_string(),
            model_provider_backend: "fake".to_string(),
            model_provider_base_url: "http://127.0.0.1:1234".to_string(),
            model_provider_model: "qwen/qwen3.5-35b-a3b".to_string(),
            email_provider_backend: "smtp".to_string(),
            email_from_address: "sender@example.com".to_string(),
            email_smtp_host: "127.0.0.1".to_string(),
            email_smtp_port: 1025,
            email_smtp_username: None,
            email_smtp_password: None,
            webhook_provider_backend: "fake".to_string(),
        }
    }

    fn http_webhook_config() -> AppConfig {
        let mut config = smtp_config();
        config.email_provider_backend = "fake".to_string();
        config.webhook_provider_backend = "http".to_string();
        config
    }

    #[tokio::test]
    async fn smtp_tool_provider_uses_configured_sender_identity() {
        let executor: Arc<dyn ActionExecutor> = Arc::new(
            SmtpEmailSendExecutor::new(
                "sender@example.com".to_string(),
                Arc::new(StubEmailSender { health_ok: true }),
            )
            .unwrap(),
        );
        let mut routes = HashMap::new();
        routes.insert(("email".to_string(), "send".to_string()), executor);
        let provider = RoutedToolProvider::new(routes);

        let result = provider
            .execute_syscall(
                "syscall-1",
                "email",
                "send",
                EMAIL_SEND_ARGS_SCHEMA_REF,
                Some(EMAIL_SEND_RESULT_SCHEMA_REF),
                &json!({
                    "to": ["to@example.com"],
                    "cc": [],
                    "bcc": [],
                    "subject": "Hello",
                    "body": "World"
                }),
            )
            .await
            .unwrap();

        assert_eq!(result.output_json["provider"], "smtp");
        assert_eq!(result.output_json["from_address"], "sender@example.com");
        assert_eq!(result.output_json["recipient_count"], 1);
        assert_eq!(result.output_json["idempotency_supported"], false);
    }

    #[test]
    fn build_tool_provider_rejects_unknown_backend() {
        let mut config = smtp_config();
        config.email_provider_backend = "bogus".to_string();
        let err = match build_tool_provider(&config) {
            Ok(_) => panic!("expected invalid backend to fail"),
            Err(err) => err.to_string(),
        };
        assert!(err.contains("unknown email provider backend"));
    }

    #[test]
    fn build_tool_provider_rejects_unknown_webhook_backend() {
        let mut config = smtp_config();
        config.webhook_provider_backend = "bogus".to_string();
        let err = match build_tool_provider(&config) {
            Ok(_) => panic!("expected invalid backend to fail"),
            Err(err) => err.to_string(),
        };
        assert!(err.contains("unknown webhook provider backend"));
    }

    #[test]
    fn smtp_tool_provider_rejects_invalid_from_address() {
        let err = match SmtpEmailSendExecutor::new(
            "not-an-address".to_string(),
            Arc::new(StubEmailSender { health_ok: true }),
        ) {
            Ok(_) => panic!("expected invalid sender identity to fail"),
            Err(err) => err.to_string(),
        };
        assert!(err.contains("invalid email address"));
    }

    #[test]
    fn mailbox_parser_rejects_invalid_address() {
        let err = parse_mailbox("not-an-address").unwrap_err().to_string();
        assert!(err.contains("invalid email address"));
    }

    #[tokio::test]
    async fn routed_provider_executes_second_action() {
        let provider = build_tool_provider(&smtp_config()).unwrap();
        let result = provider
            .execute_syscall(
                "syscall-webhook-1",
                "webhook",
                "post_json",
                WEBHOOK_POST_JSON_ARGS_SCHEMA_REF,
                Some(WEBHOOK_POST_JSON_RESULT_SCHEMA_REF),
                &json!({
                    "url": "https://example.invalid/hooks/demo",
                    "payload": {"hello": "world"}
                }),
            )
            .await
            .unwrap();

        assert_eq!(result.output_json["provider"], "fake_webhook");
        assert_eq!(result.output_json["status_code"], 202);
    }

    #[tokio::test]
    async fn http_webhook_provider_posts_json_to_loopback_server() {
        async fn handler(
            Json(payload): Json<serde_json::Value>,
        ) -> (StatusCode, Json<serde_json::Value>) {
            assert_eq!(payload["hello"], "world");
            (StatusCode::ACCEPTED, Json(json!({"ok": true})))
        }

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, Router::new().route("/hook", post(handler)))
                .await
                .unwrap();
        });

        let provider = build_tool_provider(&http_webhook_config()).unwrap();
        let result = provider
            .execute_syscall(
                "syscall-webhook-http-1",
                "webhook",
                "post_json",
                WEBHOOK_POST_JSON_ARGS_SCHEMA_REF,
                Some(WEBHOOK_POST_JSON_RESULT_SCHEMA_REF),
                &json!({
                    "url": format!("http://{addr}/hook"),
                    "payload": {"hello": "world"},
                    "headers": {"x-adesh-test": "1"}
                }),
            )
            .await
            .unwrap();

        assert_eq!(result.output_json["provider"], "http_webhook");
        assert_eq!(result.output_json["status_code"], 202);

        server.abort();
    }
}
