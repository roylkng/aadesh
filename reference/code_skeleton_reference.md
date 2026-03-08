# Code Skeleton Reference

Non-authoritative implementation sketch. Canonical structure and behavior are defined by the root-level specs and `docs/IMPLEMENTATION_PLAN.md`.

Goal understood: next step is a concrete, production-grade codebase skeleton: **crate/module layout** plus **SQLiteStorageProvider + FSBlobStore skeleton implementations** wired to the DDL and the Batch contracts.

Below is a clean starting point for a Rust workspace that matches the HLD and keeps swapping possible.

---

## 1) Repo layout (Rust workspace)

```text
adesh-os/
  Cargo.toml
  crates/
    agentos-contracts/         # Batch 1–3 Rust structs + Validate
      Cargo.toml
      src/
        lib.rs
        contracts/
          mod.rs
          common.rs
          batch1.rs
          batch2.rs
          batch3.rs

    agentos-core/              # Pure kernel logic: scheduler, governance, compiler, verification
      Cargo.toml
      src/
        lib.rs
        ports/                 # provider traits
          mod.rs
          storage.rs
          queue.rs
          blob.rs
          model.rs
          tools.rs
          auth.rs
          observability.rs
        kernel/
          mod.rs
          scheduler.rs
          governance.rs
          compiler.rs
          verification.rs

    agentos-storage-sqlite/    # SQLiteStorageProvider implementation
      Cargo.toml
      src/
        lib.rs
        sqlite.rs              # connection and pragmas
        storage.rs             # implements StorageProvider
        migrations/            # embedded SQL migrations
          0001_init.sql

    agentos-blob-fs/           # FSBlobStore implementation
      Cargo.toml
      src/
        lib.rs
        fs.rs

    agentos-daemon/            # axum REST + WS; wires core + providers
      Cargo.toml
      src/
        main.rs
        http/
          mod.rs
          routes.rs
          ws.rs
        wiring.rs              # builds provider graph
        config.rs

  ui/                          # React/Vite later (served by daemon)
    ...
```

### Workspace `Cargo.toml`

```toml
[workspace]
resolver = "2"
members = [
  "crates/agentos-contracts",
  "crates/agentos-core",
  "crates/agentos-storage-sqlite",
  "crates/agentos-blob-fs",
  "crates/agentos-daemon",
]
```

---

## 2) Provider traits (ports) in `agentos-core`

### `crates/agentos-core/src/ports/storage.rs`

```rust
use async_trait::async_trait;
use agentos_contracts::contracts::{AuditTrace, OperationSpec};
use agentos_contracts::contracts::ContractError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
  #[error("not found: {0}")]
  NotFound(String),
  #[error("conflict: {0}")]
  Conflict(String),
  #[error("unauthorized: {0}")]
  Unauthorized(String),
  #[error("invalid input: {0}")]
  InvalidInput(String),
  #[error("contract invalid: {0}")]
  ContractInvalid(#[from] ContractError),
  #[error("io: {0}")]
  Io(String),
  #[error("db: {0}")]
  Db(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

#[async_trait]
pub trait StorageProvider: Send + Sync {
  // Experience events (append-only)
  async fn append_event(
    &self,
    event_ref: &str,
    created_at_rfc3339: &str,
    kind: &str,
    source_class: &str,
    audience_id: Option<&str>,
    sensitivity_s: u8,
    taint_s: u8,
    content_ref: Option<&str>,
    json_payload: &serde_json::Value,
    idempotency_key: Option<&str>,
  ) -> Result<()>;

  async fn get_event(&self, event_ref: &str) -> Result<serde_json::Value>;

  // Requests idempotency: store & reuse the response payload
  async fn get_idempotent_response(&self, idempotency_key: &str)
    -> Result<Option<serde_json::Value>>;

  async fn put_idempotent_response(
    &self,
    idempotency_key: &str,
    request_id: &str,
    response_json: &serde_json::Value,
  ) -> Result<()>;

  // Operations
  async fn create_operation(&self, op: &OperationSpec, idempotency_key: Option<&str>) -> Result<()>;
  async fn update_operation_state(&self, operation_id: &str, new_state: &str, reason: Option<&str>) -> Result<()>;
  async fn get_operation(&self, operation_id: &str) -> Result<OperationSpec>;

  // Audit
  async fn store_audit_trace(&self, trace: &AuditTrace, idempotency_key: Option<&str>) -> Result<()>;
  async fn get_audit_trace(&self, audit_trace_id: &str) -> Result<AuditTrace>;
}
```

This is the minimal slice you need to get `/v1/requests`, `/v1/operations/{id}`, `/v1/audit/{id}` working. You can expand later to include gate decisions, compiled slices, syscalls, graphs, jobs, etc.

---

## 3) SQLite provider skeleton (`agentos-storage-sqlite`)

### `crates/agentos-storage-sqlite/Cargo.toml`

```toml
[package]
name = "agentos-storage-sqlite"
version = "0.1.0"
edition = "2021"

[dependencies]
agentos-core = { path = "../agentos-core" }
agentos-contracts = { path = "../agentos-contracts" }
async-trait = "0.1"
serde_json = "1"
thiserror = "1"
chrono = { version = "0.4", features = ["serde"] }

# SQLite driver (pick one)
rusqlite = { version = "0.31", features = ["bundled"] }
tokio = { version = "1", features = ["rt-multi-thread"] }
```

### `crates/agentos-storage-sqlite/src/sqlite.rs`

```rust
use rusqlite::{Connection, OpenFlags};
use std::{path::Path, sync::{Arc, Mutex}};

#[derive(Clone)]
pub struct SqlitePool {
  // Simple v0: single connection guarded by a mutex.
  // Upgrade later: r2d2 or deadpool.
  conn: Arc<Mutex<Connection>>,
}

impl SqlitePool {
  pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
    let conn = Connection::open_with_flags(
      path,
      OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )?;

    // Pragmas for production-ish defaults (tune per device)
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;

    Ok(Self { conn: Arc::new(Mutex::new(conn)) })
  }

  pub fn conn(&self) -> Arc<Mutex<Connection>> {
    self.conn.clone()
  }
}
```

### `crates/agentos-storage-sqlite/src/lib.rs`

```rust
pub mod sqlite;
pub mod storage;

pub use sqlite::SqlitePool;
pub use storage::SqliteStorageProvider;
```

### `crates/agentos-storage-sqlite/src/storage.rs`

```rust
use agentos_core::ports::storage::{Result, StorageError, StorageProvider};
use agentos_contracts::contracts::{AuditTrace, OperationSpec, Validate};
use async_trait::async_trait;
use rusqlite::{params};
use serde_json::Value;
use crate::sqlite::SqlitePool;

#[derive(Clone)]
pub struct SqliteStorageProvider {
  pool: SqlitePool,
}

impl SqliteStorageProvider {
  pub fn new(pool: SqlitePool) -> Self {
    Self { pool }
  }

  fn map_db_err(e: rusqlite::Error) -> StorageError {
    StorageError::Db(e.to_string())
  }
}

#[async_trait]
impl StorageProvider for SqliteStorageProvider {
  async fn append_event(
    &self,
    event_ref: &str,
    created_at: &str,
    kind: &str,
    source_class: &str,
    audience_id: Option<&str>,
    sensitivity_s: u8,
    taint_s: u8,
    content_ref: Option<&str>,
    json_payload: &Value,
    _idempotency_key: Option<&str>,
  ) -> Result<()> {
    let payload_str = serde_json::to_string(json_payload)
      .map_err(|e| StorageError::InvalidInput(e.to_string()))?;

    let conn = self.pool.conn();
    let mut conn = conn.lock().map_err(|_| StorageError::Io("sqlite mutex poisoned".into()))?;

    conn.execute(
      r#"
      INSERT INTO experience_events
        (event_ref, created_at, source_class, author, audience_id, sensitivity_s, taint_s, kind, content_ref, json_payload)
      VALUES
        (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9)
      "#,
      params![
        event_ref,
        created_at,
        source_class,
        audience_id,
        sensitivity_s as i64,
        taint_s as i64,
        kind,
        content_ref,
        payload_str
      ],
    ).map_err(Self::map_db_err)?;

    Ok(())
  }

  async fn get_event(&self, event_ref: &str) -> Result<Value> {
    let conn = self.pool.conn();
    let conn = conn.lock().map_err(|_| StorageError::Io("sqlite mutex poisoned".into()))?;

    let mut stmt = conn.prepare(
      "SELECT json_payload FROM experience_events WHERE event_ref = ?1"
    ).map_err(Self::map_db_err)?;

    let payload: String = stmt.query_row(params![event_ref], |row| row.get(0))
      .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound(event_ref.into()),
        _ => Self::map_db_err(e),
      })?;

    serde_json::from_str(&payload).map_err(|e| StorageError::InvalidInput(e.to_string()))
  }

  async fn get_idempotent_response(&self, idempotency_key: &str) -> Result<Option<Value>> {
    let conn = self.pool.conn();
    let conn = conn.lock().map_err(|_| StorageError::Io("sqlite mutex poisoned".into()))?;

    let mut stmt = conn.prepare(
      "SELECT response_json FROM idempotency_keys WHERE idempotency_key = ?1"
    ).map_err(Self::map_db_err)?;

    let res: Result<String> = stmt.query_row(params![idempotency_key], |row| row.get(0))
      .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound("no row".into()),
        _ => Self::map_db_err(e),
      });

    match res {
      Ok(s) => {
        let v: Value = serde_json::from_str(&s).map_err(|e| StorageError::InvalidInput(e.to_string()))?;
        Ok(Some(v))
      }
      Err(StorageError::NotFound(_)) => Ok(None),
      Err(e) => Err(e),
    }
  }

  async fn put_idempotent_response(
    &self,
    idempotency_key: &str,
    request_id: &str,
    response_json: &Value,
  ) -> Result<()> {
    let payload_str = serde_json::to_string(response_json)
      .map_err(|e| StorageError::InvalidInput(e.to_string()))?;

    let conn = self.pool.conn();
    let mut conn = conn.lock().map_err(|_| StorageError::Io("sqlite mutex poisoned".into()))?;

    conn.execute(
      r#"
      INSERT INTO idempotency_keys
        (idempotency_key, request_id, response_json, created_at)
      VALUES
        (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
      "#,
      params![idempotency_key, request_id, payload_str],
    ).map_err(Self::map_db_err)?;

    Ok(())
  }

  async fn create_operation(&self, op: &OperationSpec, _idempotency_key: Option<&str>) -> Result<()> {
    op.validate()?; // contract validation

    let conn = self.pool.conn();
    let mut conn = conn.lock().map_err(|_| StorageError::Io("sqlite mutex poisoned".into()))?;

    let budgets_json = serde_json::to_string(&op.budgets)
      .map_err(|e| StorageError::InvalidInput(e.to_string()))?;
    let goal_json = serde_json::to_string(&op.operation_goal)
      .map_err(|e| StorageError::InvalidInput(e.to_string()))?;
    let ipc_json = serde_json::to_string(&op.ipc)
      .map_err(|e| StorageError::InvalidInput(e.to_string()))?;

    let state = format!("{:?}", op.lifecycle.state).to_lowercase();
    let updated_at = op.lifecycle.updated_at.unwrap_or(op.created_at).to_rfc3339();

    conn.execute(
      r#"
      INSERT INTO operations
        (operation_id, parent_request_id, isolation_id, created_at, updated_at, state, state_reason,
         requesting_audience_id,
         pinned_active_state_version, pinned_capability_snapshot_version, pinned_audience_graph_version,
         budgets_json, operation_goal_json, ipc_json)
      VALUES
        (?1, ?2, ?3, ?4, ?5, ?6, ?7,
         ?8,
         ?9, ?10, ?11,
         ?12, ?13, ?14)
      "#,
      params![
        op.operation_id,
        op.parent_request_id,
        op.isolation_id,
        op.created_at.to_rfc3339(),
        updated_at,
        state,
        op.lifecycle.state_reason.as_deref(),
        op.requesting_audience_id,
        op.pinned_state.active_state_version,
        op.pinned_state.capability_snapshot_version,
        "", // pinned_audience_graph_version (fill later)
        budgets_json,
        goal_json,
        ipc_json
      ]
    ).map_err(Self::map_db_err)?;

    Ok(())
  }

  async fn update_operation_state(&self, operation_id: &str, new_state: &str, reason: Option<&str>) -> Result<()> {
    let conn = self.pool.conn();
    let mut conn = conn.lock().map_err(|_| StorageError::Io("sqlite mutex poisoned".into()))?;

    conn.execute(
      "UPDATE operations SET state = ?1, state_reason = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE operation_id = ?3",
      params![new_state, reason, operation_id]
    ).map_err(Self::map_db_err)?;

    Ok(())
  }

  async fn get_operation(&self, operation_id: &str) -> Result<OperationSpec> {
    let conn = self.pool.conn();
    let conn = conn.lock().map_err(|_| StorageError::Io("sqlite mutex poisoned".into()))?;

    let mut stmt = conn.prepare(
      r#"
      SELECT parent_request_id, isolation_id, created_at, updated_at, state, state_reason,
             requesting_audience_id,
             pinned_active_state_version, pinned_capability_snapshot_version,
             budgets_json, operation_goal_json, ipc_json
      FROM operations WHERE operation_id = ?1
      "#
    ).map_err(Self::map_db_err)?;

    let row = stmt.query_row(params![operation_id], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, String>(3)?,
        row.get::<_, String>(4)?,
        row.get::<_, Option<String>>(5)?,
        row.get::<_, String>(6)?,
        row.get::<_, Option<String>>(7)?,
        row.get::<_, Option<String>>(8)?,
        row.get::<_, String>(9)?,
        row.get::<_, String>(10)?,
        row.get::<_, String>(11)?,
      ))
    }).map_err(|e| match e {
      rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound(operation_id.into()),
      _ => Self::map_db_err(e),
    })?;

    let (parent_request_id, isolation_id, created_at, updated_at, state, state_reason,
      requesting_audience_id, pinned_active, pinned_cap, budgets_json, goal_json, ipc_json) = row;

    let budgets = serde_json::from_str(&budgets_json).map_err(|e| StorageError::InvalidInput(e.to_string()))?;
    let operation_goal = serde_json::from_str(&goal_json).map_err(|e| StorageError::InvalidInput(e.to_string()))?;
    let ipc = serde_json::from_str(&ipc_json).map_err(|e| StorageError::InvalidInput(e.to_string()))?;

    let state_enum = match state.as_str() {
      "created" => agentos_contracts::contracts::OperationState::Created,
      "compiled" => agentos_contracts::contracts::OperationState::Compiled,
      "awaitingapproval" | "awaiting_approval" => agentos_contracts::contracts::OperationState::AwaitingApproval,
      "running" => agentos_contracts::contracts::OperationState::Running,
      "blocked" => agentos_contracts::contracts::OperationState::Blocked,
      "completed" => agentos_contracts::contracts::OperationState::Completed,
      "failed" => agentos_contracts::contracts::OperationState::Failed,
      "cancelled" => agentos_contracts::contracts::OperationState::Cancelled,
      _ => agentos_contracts::contracts::OperationState::Failed,
    };

    Ok(OperationSpec {
      operation_id: operation_id.into(),
      parent_request_id,
      isolation_id,
      created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
        .map_err(|e| StorageError::InvalidInput(e.to_string()))?
        .with_timezone(&chrono::Utc),
      requesting_audience_id,
      operation_goal,
      lifecycle: agentos_contracts::contracts::OperationLifecycle {
        state: state_enum,
        state_reason,
        updated_at: Some(chrono::DateTime::parse_from_rfc3339(&updated_at)
          .map_err(|e| StorageError::InvalidInput(e.to_string()))?
          .with_timezone(&chrono::Utc)),
      },
      budgets,
      pinned_state: agentos_contracts::contracts::PinnedState {
        active_state_version: pinned_active.unwrap_or_default(),
        capability_snapshot_version: pinned_cap.unwrap_or_default(),
      },
      governance_hints: None,
      ipc,
    })
  }

  async fn store_audit_trace(&self, trace: &AuditTrace, _idempotency_key: Option<&str>) -> Result<()> {
    let payload_pinned = serde_json::to_string(&trace.pinned).map_err(|e| StorageError::InvalidInput(e.to_string()))?;
    let payload_summary = serde_json::to_string(&trace.summary).map_err(|e| StorageError::InvalidInput(e.to_string()))?;
    let payload_timeline = serde_json::to_string(&trace.timeline).map_err(|e| StorageError::InvalidInput(e.to_string()))?;
    let payload_attach = serde_json::to_string(&trace.attachments).map_err(|e| StorageError::InvalidInput(e.to_string()))?;

    let conn = self.pool.conn();
    let mut conn = conn.lock().map_err(|_| StorageError::Io("sqlite mutex poisoned".into()))?;

    conn.execute(
      r#"
      INSERT INTO audit_traces
        (audit_trace_id, created_at, request_id, operation_id, isolation_id,
         pinned_json, summary_json, timeline_json, attachments_json)
      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
      "#,
      params![
        trace.audit_trace_id,
        trace.created_at.to_rfc3339(),
        trace.request_id,
        trace.operation_id,
        trace.isolation_id,
        payload_pinned,
        payload_summary,
        payload_timeline,
        payload_attach
      ]
    ).map_err(Self::map_db_err)?;

    Ok(())
  }

  async fn get_audit_trace(&self, audit_trace_id: &str) -> Result<AuditTrace> {
    let conn = self.pool.conn();
    let conn = conn.lock().map_err(|_| StorageError::Io("sqlite mutex poisoned".into()))?;

    let mut stmt = conn.prepare(
      r#"
      SELECT created_at, request_id, operation_id, isolation_id, pinned_json, summary_json, timeline_json, attachments_json
      FROM audit_traces WHERE audit_trace_id = ?1
      "#
    ).map_err(Self::map_db_err)?;

    let row = stmt.query_row(params![audit_trace_id], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, String>(3)?,
        row.get::<_, String>(4)?,
        row.get::<_, String>(5)?,
        row.get::<_, String>(6)?,
        row.get::<_, Option<String>>(7)?,
      ))
    }).map_err(|e| match e {
      rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound(audit_trace_id.into()),
      _ => Self::map_db_err(e),
    })?;

    let (created_at, request_id, operation_id, isolation_id, pinned_json, summary_json, timeline_json, attachments_json) = row;

    Ok(AuditTrace {
      audit_trace_id: audit_trace_id.into(),
      created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
        .map_err(|e| StorageError::InvalidInput(e.to_string()))?
        .with_timezone(&chrono::Utc),
      request_id,
      operation_id,
      isolation_id,
      pinned: serde_json::from_str(&pinned_json).map_err(|e| StorageError::InvalidInput(e.to_string()))?,
      summary: serde_json::from_str(&summary_json).map_err(|e| StorageError::InvalidInput(e.to_string()))?,
      timeline: serde_json::from_str(&timeline_json).map_err(|e| StorageError::InvalidInput(e.to_string()))?,
      attachments: match attachments_json {
        Some(s) => serde_json::from_str(&s).map_err(|e| StorageError::InvalidInput(e.to_string()))?,
        None => None,
      },
    })
  }
}
```

This skeleton is enough to persist and retrieve core objects and start wiring the daemon endpoints.

---

## 4) FSBlobStore skeleton (`agentos-blob-fs`)

### `crates/agentos-blob-fs/Cargo.toml`

```toml
[package]
name = "agentos-blob-fs"
version = "0.1.0"
edition = "2021"

[dependencies]
async-trait = "0.1"
bytes = "1"
sha2 = "0.10"
hex = "0.4"
tokio = { version = "1", features = ["fs", "rt-multi-thread"] }
thiserror = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### `crates/agentos-blob-fs/src/fs.rs`

```rust
use async_trait::async_trait;
use bytes::Bytes;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BlobMeta {
  pub content_type: Option<String>,
  pub size_bytes: u64,
  pub checksum_sha256: String,
  pub sensitivity_s: u8,
  pub taint_s: u8,
  pub provenance_refs: Vec<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum BlobError {
  #[error("io: {0}")]
  Io(String),
  #[error("not found: {0}")]
  NotFound(String),
  #[error("invalid: {0}")]
  Invalid(String),
}

#[async_trait]
pub trait BlobStore: Send + Sync {
  async fn put_bytes(&self, bytes: Bytes, meta: BlobMeta) -> Result<String, BlobError>;
  async fn get_bytes(&self, content_ref: &str) -> Result<Bytes, BlobError>;
  async fn head(&self, content_ref: &str) -> Result<BlobMeta, BlobError>;
}

#[derive(Clone)]
pub struct FsBlobStore {
  root: PathBuf,
}

impl FsBlobStore {
  pub fn new(root: impl AsRef<Path>) -> Self {
    Self { root: root.as_ref().to_path_buf() }
  }

  fn content_path(&self, content_ref: &str) -> PathBuf {
    self.root.join(content_ref).join("blob")
  }
  fn meta_path(&self, content_ref: &str) -> PathBuf {
    self.root.join(content_ref).join("meta.json")
  }
}

#[async_trait]
impl BlobStore for FsBlobStore {
  async fn put_bytes(&self, bytes: Bytes, mut meta: BlobMeta) -> Result<String, BlobError> {
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let hexsum = hex::encode(digest);

    meta.size_bytes = bytes.len() as u64;
    meta.checksum_sha256 = hexsum.clone();

    // content_ref could be content-addressed
    let content_ref = format!("sha256:{}", hexsum);
    let dir = self.root.join(&content_ref);
    fs::create_dir_all(&dir).await.map_err(|e| BlobError::Io(e.to_string()))?;

    fs::write(self.content_path(&content_ref), &bytes).await.map_err(|e| BlobError::Io(e.to_string()))?;
    let meta_json = serde_json::to_vec_pretty(&meta).map_err(|e| BlobError::Invalid(e.to_string()))?;
    fs::write(self.meta_path(&content_ref), meta_json).await.map_err(|e| BlobError::Io(e.to_string()))?;

    Ok(content_ref)
  }

  async fn get_bytes(&self, content_ref: &str) -> Result<Bytes, BlobError> {
    let p = self.content_path(content_ref);
    let data = fs::read(p).await.map_err(|e| {
      if e.kind() == std::io::ErrorKind::NotFound { BlobError::NotFound(content_ref.into()) }
      else { BlobError::Io(e.to_string()) }
    })?;
    Ok(Bytes::from(data))
  }

  async fn head(&self, content_ref: &str) -> Result<BlobMeta, BlobError> {
    let p = self.meta_path(content_ref);
    let data = fs::read(p).await.map_err(|e| {
      if e.kind() == std::io::ErrorKind::NotFound { BlobError::NotFound(content_ref.into()) }
      else { BlobError::Io(e.to_string()) }
    })?;
    serde_json::from_slice(&data).map_err(|e| BlobError::Invalid(e.to_string()))
  }
}
```

Below is a production-grade **agentos-daemon skeleton** that wires:

* axum REST API (subset needed to unblock UI):

  * `POST /v1/requests`
  * `GET /v1/operations/{id}`
  * `GET /v1/audit/{id}`
  * `GET /v1/health`
* WebSocket `/v1/events` with:

  * `operation_state`
  * `audit_update`
* Uses `SqliteStorageProvider` and persists:

  * `OperationSpec` in `operations`
  * basic `AuditTrace` in `audit_traces`
  * optional Experience Log append for the inbound request

The scheduler/governance/LLM are stubbed, but the plumbing, IDs, persistence, and event stream are real.

---

## 1) `crates/agentos-daemon/Cargo.toml`

```toml
[package]
name = "agentos-daemon"
version = "0.1.0"
edition = "2021"

[dependencies]
agentos-contracts = { path = "../agentos-contracts" }
agentos-core = { path = "../agentos-core" }
agentos-storage-sqlite = { path = "../agentos-storage-sqlite" }

axum = { version = "0.7", features = ["ws", "macros"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "signal"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }

serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
thiserror = "1"

tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "json"] }
```

---

## 2) `crates/agentos-daemon/src/main.rs`

```rust
use agentos_storage_sqlite::{SqlitePool, SqliteStorageProvider};
use axum::{routing::get, Router};
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

mod config;
mod wiring;
mod http;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
    .json()
    .init();

  let cfg = config::Config::from_env();

  // SQLite storage
  let pool = SqlitePool::open(&cfg.sqlite_path)?;
  let storage = SqliteStorageProvider::new(pool);

  // Event bus
  let event_bus = wiring::EventBus::new();

  let state = wiring::AppState {
    storage,
    event_bus: event_bus.clone(),
  };

  let app = Router::new()
    .merge(http::router(state))
    .route("/v1/health", get(http::health::health));

  let addr: SocketAddr = cfg.bind_addr.parse()?;
  tracing::info!(%addr, "Starting agentos-daemon");

  axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
  Ok(())
}
```

Add `anyhow` dependency if you want convenience; otherwise replace with explicit errors.

---

## 3) `crates/agentos-daemon/src/config.rs`

```rust
#[derive(Clone, Debug)]
pub struct Config {
  pub bind_addr: String,
  pub sqlite_path: String,
}

impl Config {
  pub fn from_env() -> Self {
    let bind_addr = std::env::var("ADESH_BIND").unwrap_or_else(|_| "127.0.0.1:7777".to_string());
    let sqlite_path = std::env::var("ADESH_SQLITE").unwrap_or_else(|_| "./adesh.db".to_string());
    Self { bind_addr, sqlite_path }
  }
}
```

---

## 4) `crates/agentos-daemon/src/wiring.rs`

A minimal in-memory event bus for WS fanout. Persisted audit/logging still goes to SQLite.

```rust
use axum::extract::ws::Message;
use std::sync::Arc;
use tokio::sync::broadcast;

use agentos_storage_sqlite::SqliteStorageProvider;

#[derive(Clone)]
pub struct EventBus {
  tx: broadcast::Sender<Message>,
}

impl EventBus {
  pub fn new() -> Self {
    let (tx, _) = broadcast::channel(1024);
    Self { tx }
  }

  pub fn subscribe(&self) -> broadcast::Receiver<Message> {
    self.tx.subscribe()
  }

  pub fn publish_json(&self, value: &serde_json::Value) {
    if let Ok(text) = serde_json::to_string(value) {
      let _ = self.tx.send(Message::Text(text));
    }
  }
}

#[derive(Clone)]
pub struct AppState {
  pub storage: SqliteStorageProvider,
  pub event_bus: EventBus,
}

pub type SharedState = Arc<AppState>;
```

---

## 5) `crates/agentos-daemon/src/http/mod.rs`

```rust
use axum::Router;
use std::sync::Arc;

use crate::wiring::AppState;

pub mod health;
pub mod routes;
pub mod ws;

pub fn router(state: AppState) -> Router {
  let shared = Arc::new(state);

  Router::new()
    .merge(routes::router(shared.clone()))
    .merge(ws::router(shared))
}
```

---

## 6) `crates/agentos-daemon/src/http/health.rs`

```rust
use axum::Json;
use serde_json::json;

pub async fn health() -> Json<serde_json::Value> {
  Json(json!({
    "status": "ok",
    "version": "0.2",
    "storage": "ok",
    "model_provider": "degraded",
    "tool_provider": "degraded",
    "queue": "degraded"
  }))
}
```

---

## 7) `crates/agentos-daemon/src/http/ws.rs`

WebSocket `/v1/events`. Root-owner auth is not implemented here yet. You will add it as middleware.

```rust
use axum::{
  extract::{State, ws::{WebSocketUpgrade, WebSocket, Message}},
  response::IntoResponse,
  routing::get,
  Router,
};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::wiring::AppState;

pub fn router(state: Arc<AppState>) -> Router {
  Router::new()
    .route("/v1/events", get(events_ws))
    .with_state(state)
}

async fn events_ws(
  State(state): State<Arc<AppState>>,
  ws: WebSocketUpgrade
) -> impl IntoResponse {
  ws.on_upgrade(move |socket| handle_socket(state, socket))
}

async fn handle_socket(state: Arc<AppState>, mut socket: WebSocket) {
  let mut rx: broadcast::Receiver<Message> = state.event_bus.subscribe();

  // Send a hello event
  state.event_bus.publish_json(&serde_json::json!({
    "event_id": uuid::Uuid::new_v4().to_string(),
    "ts": chrono::Utc::now().to_rfc3339(),
    "type": "hello",
    "data": { "message": "connected" }
  }));

  loop {
    tokio::select! {
      // Fanout bus -> client
      msg = rx.recv() => {
        match msg {
          Ok(m) => {
            if socket.send(m).await.is_err() { break; }
          }
          Err(_) => break,
        }
      }
      // Client -> server (optional)
      incoming = socket.recv() => {
        match incoming {
          Some(Ok(Message::Close(_))) | None => break,
          Some(Ok(_)) => { /* ignore for now */ }
          Some(Err(_)) => break,
        }
      }
    }
  }
}
```

---

## 8) `crates/agentos-daemon/src/http/routes.rs`

Implements:

* `POST /v1/requests` (stores request event, creates op, stores audit, emits WS transitions)
* `GET /v1/operations/{id}`
* `GET /v1/audit/{id}`

```rust
use axum::{
  extract::{Path, State},
  routing::{get, post},
  http::HeaderMap,
  Json, Router,
};
use serde_json::json;
use std::sync::Arc;

use agentos_contracts::contracts::{
  AuditAttachments, AuditPinned, AuditSummary, AuditSummaryAudience, AuditSummaryGate, AuditTimelineItem,
  OperationGoal, OperationLifecycle, OperationResult, OperationSpec, OperationState, RequestEnvelope, Validate,
};

use crate::wiring::AppState;

pub fn router(state: Arc<AppState>) -> Router {
  Router::new()
    .route("/v1/requests", post(post_requests))
    .route("/v1/operations/:operation_id", get(get_operation))
    .route("/v1/audit/:audit_trace_id", get(get_audit))
    .with_state(state)
}

async fn post_requests(
  State(state): State<Arc<AppState>>,
  headers: HeaderMap,
  Json(req): Json<RequestEnvelope>,
) -> Json<serde_json::Value> {
  // Validate contract
  if let Err(e) = req.validate() {
    return Json(json!({
      "ok": false,
      "error": { "code": "INVALID_INPUT", "message": e.to_string(), "details": {}},
      "meta": { "request_id": req.request_id, "ts": chrono::Utc::now().to_rfc3339() }
    }));
  }

  // Extract Idempotency-Key (optional)
  let idem = headers.get("Idempotency-Key")
    .and_then(|v| v.to_str().ok())
    .map(|s| s.to_string());

  // If idempotency key exists, return previously stored response
  if let Some(key) = idem.as_deref() {
    match state.storage.get_idempotent_response(key).await {
      Ok(Some(resp)) => return Json(resp),
      Ok(None) => {}
      Err(e) => {
        return Json(json!({
          "ok": false,
          "error": { "code": "PERMANENT", "message": format!("idempotency lookup failed: {e}"), "details": {}},
          "meta": { "request_id": req.request_id, "ts": chrono::Utc::now().to_rfc3339() }
        }));
      }
    }
  }

  // Serialize request payload for Experience Log (no fail-open)
  let payload = match serde_json::to_value(&req) {
    Ok(v) => v,
    Err(e) => {
      return Json(json!({
        "ok": false,
        "error": { "code": "PERMANENT", "message": format!("request serialization failed: {e}"), "details": {}},
        "meta": { "request_id": req.request_id, "ts": chrono::Utc::now().to_rfc3339() }
      }));
    }
  };

  // Persist inbound request as an experience event
  let event_ref = format!("event:{}", uuid::Uuid::new_v4());
  let created_at = chrono::Utc::now().to_rfc3339();
  if let Err(e) = state.storage.append_event(
    &event_ref,
    &created_at,
    "request",
    "http",
    Some(&req.requesting_audience_id),
    1,
    1,
    None,
    &payload,
    idem.as_deref(),
  ).await {
    return Json(json!({
      "ok": false,
      "error": { "code": "PERMANENT", "message": format!("append_event failed: {e}"), "details": {}},
      "meta": { "request_id": req.request_id, "ts": chrono::Utc::now().to_rfc3339() }
    }));
  }

  // Stub scheduler: create single operation
  let operation_id = uuid::Uuid::new_v4().to_string();
  let isolation_id = uuid::Uuid::new_v4().to_string();

  // Pinned versions: stub for now
  let pinned_active = "state:0".to_string();
  let pinned_cap = "cap:0".to_string();

  let op = OperationSpec {
    operation_id: operation_id.clone(),
    parent_request_id: req.request_id.clone(),
    isolation_id: isolation_id.clone(),
    created_at: chrono::Utc::now(),
    requesting_audience_id: req.requesting_audience_id.clone(),
    operation_goal: OperationGoal {
      summary: req.input.content.clone(),
      input_refs: Some(vec![event_ref.clone()]),
      requested_outputs: None,
    },
    lifecycle: OperationLifecycle {
      state: OperationState::Created,
      state_reason: None,
      updated_at: Some(chrono::Utc::now()),
    },
    budgets: agentos_contracts::contracts::OperationBudgets {
      token_budget: req.constraints.budgets.token_budget,
      block_budgets: agentos_contracts::contracts::BlockBudgets {
        policy: 512,
        capability: 512,
        operation_context: 1024,
        evidence: 1536,
        scratch: 512,
      },
      latency_ms: req.constraints.budgets.latency_ms,
      cost_cents: req.constraints.budgets.cost_cents,
    },
    pinned_state: agentos_contracts::contracts::PinnedState {
      active_state_version: pinned_active.clone(),
      capability_snapshot_version: pinned_cap.clone(),
    },
    governance_hints: None,
    ipc: None,
  };

  if let Err(e) = op.validate() {
    return Json(json!({
      "ok": false,
      "error": { "code": "INVALID_INPUT", "message": e.to_string(), "details": {}},
      "meta": { "request_id": req.request_id, "ts": chrono::Utc::now().to_rfc3339() }
    }));
  }

  if let Err(e) = state.storage.create_operation(&op, idem.as_deref()).await {
    return Json(json!({
      "ok": false,
      "error": { "code": "PERMANENT", "message": format!("create_operation failed: {e}"), "details": {}},
      "meta": { "request_id": req.request_id, "ts": chrono::Utc::now().to_rfc3339() }
    }));
  }

  // WS: created
  state.event_bus.publish_json(&json!({
    "event_id": uuid::Uuid::new_v4().to_string(),
    "ts": chrono::Utc::now().to_rfc3339(),
    "type": "operation_state",
    "operation_id": op.operation_id,
    "isolation_id": op.isolation_id,
    "audit_trace_id": null,
    "data": { "state": "created", "reason": null }
  }));

  // Transition to running (stub)
  let _ = state.storage.update_operation_state(&op.operation_id, "running", None).await;
  state.event_bus.publish_json(&json!({
    "event_id": uuid::Uuid::new_v4().to_string(),
    "ts": chrono::Utc::now().to_rfc3339(),
    "type": "operation_state",
    "operation_id": op.operation_id,
    "isolation_id": op.isolation_id,
    "audit_trace_id": null,
    "data": { "state": "running", "reason": "stub scheduler" }
  }));

  // Minimal AuditTrace
  let audit_trace_id = format!("audit:{}", uuid::Uuid::new_v4());
  let audit = agentos_contracts::contracts::AuditTrace {
    audit_trace_id: audit_trace_id.clone(),
    created_at: chrono::Utc::now(),
    request_id: req.request_id.clone(),
    operation_id: op.operation_id.clone(),
    isolation_id: op.isolation_id.clone(),
    pinned: AuditPinned {
      active_state_version: pinned_active,
      capability_snapshot_version: pinned_cap,
      audience_graph_version: "aud:0".to_string(),
    },
    timeline: vec![
      AuditTimelineItem {
        ts: chrono::Utc::now(),
        event_type: agentos_contracts::contracts::TimelineEventType::OperationStateChange,
        ref_id: None,
        note: Some("created -> running (stub)".into()),
      }
    ],
    summary: AuditSummary {
      gate: AuditSummaryGate { risk_r: 1, sensitivity_s: 1, max_gate: 1, approval_mode: agentos_contracts::contracts::ApprovalMode::None },
      audience: AuditSummaryAudience { requesting_audience_id: req.requesting_audience_id.clone(), sensitivity_ceiling_s: 4 },
      result: OperationResult::Completed,
    },
    attachments: Some(AuditAttachments {
      gate_decision_ref: None,
      compiled_slice_ref: None,
      syscall_refs: None,
      ipc_artifact_refs: None,
      experience_log_refs: Some(vec![event_ref]),
    }),
  };

  if let Err(e) = state.storage.store_audit_trace(&audit, idem.as_deref()).await {
    return Json(json!({
      "ok": false,
      "error": { "code": "PERMANENT", "message": format!("store_audit_trace failed: {e}"), "details": {}},
      "meta": { "request_id": req.request_id, "ts": chrono::Utc::now().to_rfc3339() }
    }));
  }

  state.event_bus.publish_json(&json!({
    "event_id": uuid::Uuid::new_v4().to_string(),
    "ts": chrono::Utc::now().to_rfc3339(),
    "type": "audit_update",
    "operation_id": op.operation_id,
    "isolation_id": op.isolation_id,
    "audit_trace_id": audit_trace_id,
    "data": { "ref_id": "audit_trace" }
  }));

  // Complete operation (stub)
  let _ = state.storage.update_operation_state(&op.operation_id, "completed", Some("stub completed")).await;
  state.event_bus.publish_json(&json!({
    "event_id": uuid::Uuid::new_v4().to_string(),
    "ts": chrono::Utc::now().to_rfc3339(),
    "type": "operation_state",
    "operation_id": op.operation_id,
    "isolation_id": op.isolation_id,
    "audit_trace_id": audit_trace_id,
    "data": { "state": "completed", "reason": "stub completed" }
  }));

  let response = json!({
    "ok": true,
    "data": {
      "request_id": req.request_id,
      "operation_ids": [ op.operation_id ],
      "primary_operation_id": op.operation_id,
      "audit_trace_ids": [ audit_trace_id ]
    },
    "meta": { "request_id": req.request_id, "ts": chrono::Utc::now().to_rfc3339() }
  });

  // Store idempotent response (if key present)
  if let Some(key) = idem.as_deref() {
    if let Err(e) = state.storage.put_idempotent_response(key, &req.request_id, &response).await {
      return Json(json!({
        "ok": false,
        "error": { "code": "PERMANENT", "message": format!("idempotency store failed: {e}"), "details": {}},
        "meta": { "request_id": req.request_id, "ts": chrono::Utc::now().to_rfc3339() }
      }));
    }
  }

  Json(response)
}

async fn get_operation(
  State(state): State<Arc<AppState>>,
  Path(operation_id): Path<String>,
) -> Json<serde_json::Value> {
  match state.storage.get_operation(&operation_id).await {
    Ok(op) => Json(json!({
      "ok": true,
      "data": op,
      "meta": { "request_id": uuid::Uuid::new_v4().to_string(), "ts": chrono::Utc::now().to_rfc3339() }
    })),
    Err(e) => Json(json!({
      "ok": false,
      "error": { "code": "NOT_FOUND", "message": e.to_string(), "details": {} },
      "meta": { "request_id": uuid::Uuid::new_v4().to_string(), "ts": chrono::Utc::now().to_rfc3339() }
    }))
  }
}

async fn get_audit(
  State(state): State<Arc<AppState>>,
  Path(audit_trace_id): Path<String>,
) -> Json<serde_json::Value> {
  match state.storage.get_audit_trace(&audit_trace_id).await {
    Ok(a) => Json(json!({
      "ok": true,
      "data": a,
      "meta": { "request_id": uuid::Uuid::new_v4().to_string(), "ts": chrono::Utc::now().to_rfc3339(), "audit_trace_id": audit_trace_id }
    })),
    Err(e) => Json(json!({
      "ok": false,
      "error": { "code": "NOT_FOUND", "message": e.to_string(), "details": {} },
      "meta": { "request_id": uuid::Uuid::new_v4().to_string(), "ts": chrono::Utc::now().to_rfc3339() }
    }))
  }
}
```

---

## 9) Migration file

Put your SQLite DDL into:

`crates/agentos-storage-sqlite/src/migrations/0001_init.sql`

Then add a tiny migration runner later (or run manually for now). If you want it automated, I’ll add a simple `run_migrations()` call in `SqlitePool::open()` that executes embedded SQL.

---

## 10) What you can test immediately

1. Run daemon:

```bash
ADESH_SQLITE=./adesh.db cargo run -p agentos-daemon
```

2. Connect WS:

* `ws://127.0.0.1:7777/v1/events`

3. POST request:

```bash
curl -X POST http://127.0.0.1:7777/v1/requests \
  -H 'Content-Type: application/json' \
  -d '{
    "request_id":"req-1",
    "source":{"channel":"http","transport":"rest"},
    "received_at":"2026-03-07T00:00:00Z",
    "requesting_principal":{"principal_type":"root_owner","principal_id":"root"},
    "requesting_audience_id":"root_owner",
    "input":{"kind":"text","content":"hello"},
    "constraints":{"policy_mode":"default","budgets":{"token_budget":1024}}
  }'
```

4. GET operation and audit.

---
