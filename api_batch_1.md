Goal understood: lock the LLD “physics” via schemas, starting with Batch 1. Yes, batching is the right way to avoid a schema wall and keep invariants crisp.

Below is **Batch 1: Boot & Route Phase** schemas. These are **interface contracts**, not database tables. Field names are chosen to directly support v2.5 semantics: ownership, OOB for R4, audience binding, operation isolation, and deterministic routing.

```json
{
  "$id": "https://adesh.os/schemas/v0.1/OwnerSession.json",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "OwnerSession",
  "type": "object",
  "additionalProperties": false,
  "required": ["owner_id", "session_id", "auth_level", "issued_at", "expires_at", "scopes"],
  "properties": {
    "owner_id": {
      "type": "string",
      "description": "Stable Root Owner principal id."
    },
    "session_id": {
      "type": "string",
      "description": "Opaque session identifier."
    },
    "auth_level": {
      "type": "string",
      "enum": ["local", "strong", "oob_verified"],
      "description": "Authentication strength metadata for session UX. OOB verification must remain approval-bound and must not grant global elevated execution privileges."
    },
    "issued_at": { "type": "string", "format": "date-time" },
    "expires_at": { "type": "string", "format": "date-time" },
    "scopes": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Administrative scopes granted to this session (e.g., audience_graph:write, policy:write, approvals:approve)."
    },
    "client": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "client_type": { "type": "string", "enum": ["ui", "cli", "api", "mcp"] },
        "device_id": { "type": "string" },
        "ip": { "type": "string" },
        "user_agent": { "type": "string" }
      }
    },
    "oob": {
      "type": "object",
      "additionalProperties": false,
      "description": "Optional OOB challenge state for R4 operations.",
      "properties": {
        "challenge_id": { "type": "string" },
        "nonce": { "type": "string", "description": "Server-generated nonce to be signed or verified out-of-band." },
        "challenge_type": {
          "type": "string",
          "enum": ["totp", "webauthn", "device_signature", "hardware_key", "other"]
        },
        "status": { "type": "string", "enum": ["pending", "verified", "expired", "failed"] },
        "requested_at": { "type": "string", "format": "date-time" },
        "verified_at": { "type": "string", "format": "date-time" }
      }
    }
  }
}
```

```json
{
  "$id": "https://adesh.os/schemas/v0.1/RequestEnvelope.json",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "RequestEnvelope",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "request_id",
    "source",
    "received_at",
    "requesting_principal",
    "requesting_audience_id",
    "input",
    "constraints"
  ],
  "properties": {
    "request_id": { "type": "string", "description": "UUID for the inbound request." },
    "source": {
      "type": "object",
      "additionalProperties": false,
      "required": ["channel", "transport"],
      "properties": {
        "channel": { "type": "string", "enum": ["http", "mcp"] },
        "transport": { "type": "string", "enum": ["rest", "websocket", "sse", "mcp_stdio", "mcp_http"] },
        "client_id": { "type": "string", "description": "Optional caller identity (e.g., OpenClaw instance id)." }
      }
    },
    "received_at": { "type": "string", "format": "date-time" },

    "requesting_principal": {
      "type": "object",
      "additionalProperties": false,
      "required": ["principal_type", "principal_id"],
      "properties": {
        "principal_type": { "type": "string", "enum": ["root_owner", "agent_client", "external_user"] },
        "principal_id": { "type": "string" },
        "owner_session_id": {
          "type": "string",
          "description": "If principal_type is root_owner, an owner session may be attached/validated by gateway."
        }
      }
    },

    "requesting_audience_id": {
      "type": "string",
      "description": "Audience Graph node id representing the caller/audience context. Default to Root Owner for localhost UI/CLI."
    },

    "conversation": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "thread_id": { "type": "string" },
        "turn_id": { "type": "string" },
        "history_refs": {
          "type": "array",
          "items": { "type": "string" },
          "description": "References to prior events in the Experience Log (not raw text).",
          "maxItems": 100
        }
      }
    },

    "input": {
      "type": "object",
      "additionalProperties": false,
      "required": ["kind", "content"],
      "properties": {
        "kind": { "type": "string", "enum": ["text", "structured"] },
        "content": { "type": "string", "description": "Raw user request text or serialized structured request." },
        "attachments": {
          "type": "array",
          "description": "References to blobs/docs already ingested or newly uploaded.",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["ref_id", "ref_type"],
            "properties": {
              "ref_id": { "type": "string" },
              "ref_type": { "type": "string", "enum": ["file", "doc", "email", "image", "audio", "url", "artifact"] },
              "sensitivity_hint": { "type": "integer", "minimum": 0, "maximum": 4 }
            }
          }
        }
      }
    },

    "constraints": {
      "type": "object",
      "additionalProperties": false,
      "required": ["budgets", "policy_mode"],
      "properties": {
        "policy_mode": {
          "type": "string",
          "enum": ["default", "strict", "lenient"],
          "description": "Root Owner may choose stricter stance; never bypasses core invariants."
        },
        "budgets": {
          "type": "object",
          "additionalProperties": false,
          "required": ["token_budget"],
          "properties": {
            "token_budget": { "type": "integer", "minimum": 256, "description": "Total per-operation token cap for compiled slice injection." },
            "latency_ms": { "type": "integer", "minimum": 0 },
            "cost_cents": { "type": "integer", "minimum": 0 },
            "compute_units": { "type": "number", "minimum": 0 }
          }
        },
        "preferred_model": {
          "type": "string",
          "description": "Optional hint; actual selection may be overridden by policy."
        },
        "allow_multi_operation": {
          "type": "boolean",
          "default": true,
          "description": "Whether scheduler may decompose into multiple operations."
        }
      }
    },

    "intent_anchor": {
      "type": "object",
      "additionalProperties": false,
      "description": "Optional pre-parsed intent anchor. If absent, OS derives one during scheduling/verification.",
      "properties": {
        "goal": { "type": "string" },
        "success_criteria": { "type": "array", "items": { "type": "string" } },
        "forbidden_outcomes": { "type": "array", "items": { "type": "string" } },
        "scope_limits": { "type": "array", "items": { "type": "string" } }
      }
    }
  }
}
```

```json
{
  "$id": "https://adesh.os/schemas/v0.1/OperationSpec.json",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "OperationSpec",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "operation_id",
    "parent_request_id",
    "isolation_id",
    "created_at",
    "requesting_audience_id",
    "operation_goal",
    "lifecycle",
    "budgets",
    "pinned_state"
  ],
  "properties": {
    "operation_id": { "type": "string", "description": "UUID per operation (process)." },
    "parent_request_id": { "type": "string" },
    "isolation_id": {
      "type": "string",
      "description": "Hard isolation boundary id; must be unique per operation to prevent cross-contamination."
    },
    "created_at": { "type": "string", "format": "date-time" },

    "requesting_audience_id": { "type": "string" },

    "operation_goal": {
      "type": "object",
      "additionalProperties": false,
      "required": ["summary"],
      "properties": {
        "summary": { "type": "string", "description": "Human-readable operation intent." },
        "input_refs": {
          "type": "array",
          "items": { "type": "string" },
          "description": "References to Experience Log entries or attachments relevant to this operation."
        },
        "requested_outputs": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Desired output artifacts (draft_email, summary, schedule_event, etc.). Not a taxonomy; descriptive only."
        }
      }
    },

    "lifecycle": {
      "type": "object",
      "additionalProperties": false,
      "required": ["state"],
      "properties": {
        "state": {
          "type": "string",
          "enum": ["created", "compiled", "awaiting_approval", "running", "blocked", "completed", "failed", "cancelled"]
        },
        "state_reason": { "type": "string" },
        "updated_at": { "type": "string", "format": "date-time" }
      }
    },

    "budgets": {
      "type": "object",
      "additionalProperties": false,
      "required": ["token_budget", "block_budgets"],
      "properties": {
        "token_budget": { "type": "integer", "minimum": 256 },
        "block_budgets": {
          "type": "object",
          "additionalProperties": false,
          "required": ["policy", "capability", "operation_context", "evidence", "scratch"],
          "properties": {
            "policy": { "type": "integer", "minimum": 64 },
            "capability": { "type": "integer", "minimum": 32 },
            "operation_context": { "type": "integer", "minimum": 32 },
            "evidence": { "type": "integer", "minimum": 0 },
            "scratch": { "type": "integer", "minimum": 0 }
          }
        },
        "latency_ms": { "type": "integer", "minimum": 0 },
        "cost_cents": { "type": "integer", "minimum": 0 }
      }
    },

    "pinned_state": {
      "type": "object",
      "additionalProperties": false,
      "required": ["active_state_version", "capability_snapshot_version", "audience_graph_version"],
      "properties": {
        "active_state_version": { "type": "string", "description": "Pinned version used for compilation and replay." },
        "capability_snapshot_version": { "type": "string", "description": "Pinned capability registry snapshot." },
        "audience_graph_version": { "type": "string", "description": "Pinned audience graph version for disclosure checks and replay." }
      }
    },

    "governance_hints": {
      "type": "object",
      "additionalProperties": false,
      "description": "Non-binding hints. Governance Kernel computes authoritative R/S.",
      "properties": {
        "sensitivity_hint": { "type": "integer", "minimum": 0, "maximum": 4 },
        "risk_hint": { "type": "integer", "minimum": 0, "maximum": 4 },
        "requires_owner_session": { "type": "boolean" }
      }
    },

    "ipc": {
      "type": "object",
      "additionalProperties": false,
      "description": "Explicit IPC dependencies for this operation.",
      "properties": {
        "consumes_artifacts": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Artifact ids piped into this operation."
        },
        "inherits_sensitivity": { "type": "integer", "minimum": 0, "maximum": 4 }
      }
    }
  }
}
```

### Notes on Batch 1 design choices (so you don’t get surprised later)

* **OwnerSession** includes an explicit OOB challenge structure but does not mandate a specific method. That lets you run TOTP/WebAuthn/device signature later.
* **RequestEnvelope** carries budgets and an optional intent anchor, but the OS can derive intent anchors if absent.
* **OperationSpec** contains `isolation_id`, pinned versions, and block budgets. That makes operation isolation and token packing enforceable at runtime.
