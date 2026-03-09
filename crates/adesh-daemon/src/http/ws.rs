use axum::extract::ws::{Message, WebSocket};
use axum::{
    extract::{State, WebSocketUpgrade},
    response::Response,
};
use chrono::Utc;
use tokio::sync::broadcast;
use uuid::Uuid;

use adesh_contracts::{WsEnvelope, WsHelloData};

use super::AppState;

pub async fn events(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| send_hello(socket, state))
}

async fn send_hello(mut socket: WebSocket, state: AppState) {
    let event = WsEnvelope {
        event_id: Uuid::new_v4().to_string(),
        ts: Utc::now(),
        r#type: "hello".to_string(),
        request_id: None,
        operation_id: None,
        isolation_id: None,
        audit_trace_id: None,
        data: WsHelloData {
            message: "connected".to_string(),
            server_version: state.config.server_version,
            capability_snapshot_version: state.config.capability_snapshot_version,
        },
    };

    if let Ok(json) = serde_json::to_string(&event) {
        let _ = socket.send(Message::Text(json.into())).await;
    }

    let mut rx = state.events.subscribe();
    loop {
        match rx.recv().await {
            Ok(payload) => {
                if socket.send(Message::Text(payload.into())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}
