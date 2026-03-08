# API Batch 3: Execution and Audit Contracts v0.1
Adesh OS

This document defines the Batch 3 interface contracts for syscall execution, denials, IPC artifacts, and audit traces. These are interface contracts, not database tables.

---

```json
{
  "$id": "https://adesh.os/schemas/v0.1/SyscallEnvelope.json",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "SyscallEnvelope",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "syscall_id",
    "operation_id",
    "isolation_id",
    "issued_at",
    "pinned",
    "caller",
    "target",
    "intent",
    "gate",
    "taint_in",
    "status"
  ],
  "properties": {
    "syscall_id": { "type": "string", "description": "UUID for this syscall attempt." },
    "operation_id": { "type": "string" },
    "isolation_id": { "type": "string" },
    "issued_at": { "type": "string", "format": "date-time" },

    "pinned": {
      "type": "object",
      "additionalProperties": false,
      "required": ["active_state_version", "capability_snapshot_version", "audience_graph_version"],
      "properties": {
        "active_state_version": { "type": "string" },
        "capability_snapshot_version": { "type": "string" },
        "audience_graph_version": { "type": "string" }
      }
    },

    "caller": {
      "type": "object",
      "additionalProperties": false,
      "required": ["component"],
      "properties": {
        "component": { "type": "string", "enum": ["reasoning_core", "verification_core", "scheduler", "gateway"] },
        "model_id": { "type": "string", "description": "If caller is reasoning_core, identifies model backend." }
      }
    },

    "target": {
      "type": "object",
      "additionalProperties": false,
      "required": ["kind", "name"],
      "properties": {
        "kind": { "type": "string", "enum": ["sensor", "actuator", "ipc", "sanitizer", "memory_read"] },
        "name": { "type": "string", "description": "Registered sensor/actuator/syscall name." },
        "provider": { "type": "string", "description": "mcp|adapter|internal" },
        "endpoint_ref": { "type": "string", "description": "Optional pointer to MCP server/adapter instance." }
      }
    },

    "intent": {
      "type": "object",
      "additionalProperties": false,
      "required": ["action", "args"],
      "properties": {
        "action": { "type": "string", "description": "Tool action or method name." },
        "args": {
          "type": "object",
          "additionalProperties": true,
          "description": "Arguments payload (validated against target schema by Verification Core)."
        },
        "declared_effect": {
          "type": "string",
          "enum": ["read", "write", "external_side_effect", "self_modification"],
          "description": "Declared effect category used in risk predicates."
        },
        "declared_audience_id": {
          "type": "string",
          "description": "If action sends data to a party, the intended audience node id."
        },
        "data_handles": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Refs to inputs being used (experience refs, attachments, IPC artifacts)."
        }
      }
    },

    "gate": {
      "type": "object",
      "additionalProperties": false,
      "required": ["risk_r", "sensitivity_s", "max_gate", "approval_mode", "audience_ceiling_s"],
      "properties": {
        "risk_r": { "type": "integer", "minimum": 0, "maximum": 4 },
        "sensitivity_s": { "type": "integer", "minimum": 0, "maximum": 4 },
        "max_gate": { "type": "integer", "minimum": 0, "maximum": 4 },
        "approval_mode": { "type": "string", "enum": ["none", "confirm", "diff", "oob_required", "refuse"] },
        "audience_ceiling_s": { "type": "integer", "minimum": 0, "maximum": 4 }
      }
    },

    "taint_in": {
      "type": "object",
      "additionalProperties": false,
      "required": ["max_taint_s", "sources"],
      "properties": {
        "max_taint_s": { "type": "integer", "minimum": 0, "maximum": 4 },
        "sources": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["kind", "ref_id", "taint_s"],
            "properties": {
              "kind": { "type": "string", "enum": ["block", "evidence", "ipc_artifact", "tool_result", "inferred"] },
              "ref_id": { "type": "string" },
              "taint_s": { "type": "integer", "minimum": 0, "maximum": 4 }
            }
          }
        }
      }
    },

    "status": {
      "type": "string",
      "enum": ["proposed", "permitted", "denied", "awaiting_approval", "executed", "failed"],
      "description": "Lifecycle of syscall through governance and execution."
    },

    "result": {
      "type": "object",
      "additionalProperties": false,
      "description": "Present when executed or failed.",
      "properties": {
        "ok": { "type": "boolean" },
        "started_at": { "type": "string", "format": "date-time" },
        "finished_at": { "type": "string", "format": "date-time" },
        "output_ref": { "type": "string", "description": "Ref to tool output stored in Experience Log (not raw content here)." },
        "output_sensitivity_s": { "type": "integer", "minimum": 0, "maximum": 4 },
        "output_taint_s": { "type": "integer", "minimum": 0, "maximum": 4 },
        "error_code": { "type": "string" },
        "error_message": { "type": "string" },
        "retryable": { "type": "boolean" }
      }
    },

    "audit_trace_id": { "type": "string" }
  }
}
```

---

```json
{
  "$id": "https://adesh.os/schemas/v0.1/SyscallDeny.json",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "SyscallDeny",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "syscall_id",
    "operation_id",
    "isolation_id",
    "denied_at",
    "deny_class",
    "violations",
    "retry_policy",
    "remediation",
    "audit_trace_id"
  ],
  "properties": {
    "syscall_id": { "type": "string" },
    "operation_id": { "type": "string" },
    "isolation_id": { "type": "string" },
    "denied_at": { "type": "string", "format": "date-time" },

    "deny_class": {
      "type": "string",
      "enum": [
        "audience_scope_denied",
        "sensitivity_ceiling_exceeded",
        "negative_memory_violation",
        "gate_requires_approval",
        "taint_laundering_risk",
        "self_modification_forbidden",
        "schema_requires_forbidden_field",
        "budget_exceeded",
        "verification_failed"
      ]
    },

    "violations": {
      "type": "array",
      "minItems": 1,
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["constraint_id", "constraint_type", "message"],
        "properties": {
          "constraint_id": { "type": "string", "description": "Stable id for policy/scope/gate rule." },
          "constraint_type": {
            "type": "string",
            "enum": ["policy", "audience_scope", "gate", "taint", "budget", "schema", "verification"]
          },
          "message": { "type": "string" },

          "triggering_fields": {
            "type": "array",
            "items": { "type": "string" },
            "description": "Fields/data classes that triggered the denial (e.g., passport_number, ssn)."
          },
          "triggering_refs": {
            "type": "array",
            "items": { "type": "string" },
            "description": "Refs to artifacts/blocks involved in the violation."
          },
          "computed": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
              "risk_r": { "type": "integer", "minimum": 0, "maximum": 4 },
              "sensitivity_s": { "type": "integer", "minimum": 0, "maximum": 4 },
              "max_gate": { "type": "integer", "minimum": 0, "maximum": 4 },
              "audience_ceiling_s": { "type": "integer", "minimum": 0, "maximum": 4 },
              "taint_s": { "type": "integer", "minimum": 0, "maximum": 4 }
            }
          }
        }
      }
    },

    "retry_policy": {
      "type": "object",
      "additionalProperties": false,
      "required": ["allowed", "conditions"],
      "properties": {
        "allowed": { "type": "boolean" },
        "conditions": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Human-readable retry conditions (e.g., remove passport_number, use manual booking workflow)."
        },
        "cooldown_ms": { "type": "integer", "minimum": 0, "default": 0 },
        "max_attempts": { "type": "integer", "minimum": 0, "default": 0 }
      }
    },

    "remediation": {
      "type": "object",
      "additionalProperties": false,
      "required": ["options"],
      "properties": {
        "options": {
          "type": "array",
          "minItems": 1,
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["type", "description"],
            "properties": {
              "type": {
                "type": "string",
                "enum": ["ask_user", "sanitize", "alternate_actuator", "require_approval", "require_oob", "refuse", "reduce_scope"]
              },
              "description": { "type": "string" },
              "payload": {
                "type": "object",
                "additionalProperties": true,
                "description": "Optional structured payload for UI (e.g., approval prompt, sanitizer parameters)."
              }
            }
          }
        }
      }
    },

    "audit_trace_id": { "type": "string" }
  }
}
```

---

```json
{
  "$id": "https://adesh.os/schemas/v0.1/IPCArtifact.json",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "IPCArtifact",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "artifact_id",
    "produced_by_operation_id",
    "produced_at",
    "kind",
    "content_ref",
    "sensitivity_s",
    "taint_s",
    "provenance_refs",
    "audience_scope_tag"
  ],
  "properties": {
    "artifact_id": { "type": "string", "description": "Stable id for piped artifact." },
    "produced_by_operation_id": { "type": "string" },
    "produced_at": { "type": "string", "format": "date-time" },

    "kind": {
      "type": "string",
      "enum": ["summary", "draft", "table", "extracted_fields", "sanitized_view", "other"]
    },

    "content_ref": {
      "type": "string",
      "description": "Pointer to stored artifact content in Experience Log/blob store (not inline)."
    },

    "sensitivity_s": { "type": "integer", "minimum": 0, "maximum": 4 },
    "taint_s": { "type": "integer", "minimum": 0, "maximum": 4 },

    "provenance_refs": {
      "type": "array",
      "minItems": 1,
      "items": { "type": "string" },
      "description": "Refs to evidence/events used to create this artifact."
    },

    "audience_scope_tag": {
      "type": "object",
      "additionalProperties": false,
      "required": ["allowed_scopes", "max_disclosure_s"],
      "properties": {
        "allowed_scopes": { "type": "array", "items": { "type": "string" } },
        "max_disclosure_s": { "type": "integer", "minimum": 0, "maximum": 4 }
      }
    },

    "ipc_rules": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "receiver_inherits_s": { "type": "integer", "minimum": 0, "maximum": 4 },
        "requires_recompile": { "type": "boolean", "default": true }
      }
    },

    "audit_trace_id": { "type": "string" }
  }
}
```

---

```json
{
  "$id": "https://adesh.os/schemas/v0.1/AuditTrace.json",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "AuditTrace",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "audit_trace_id",
    "created_at",
    "request_id",
    "operation_id",
    "isolation_id",
    "pinned",
    "timeline",
    "summary"
  ],
  "properties": {
    "audit_trace_id": { "type": "string" },
    "created_at": { "type": "string", "format": "date-time" },

    "request_id": { "type": "string" },
    "operation_id": { "type": "string" },
    "isolation_id": { "type": "string" },

    "pinned": {
      "type": "object",
      "additionalProperties": false,
      "required": ["active_state_version", "capability_snapshot_version", "audience_graph_version"],
      "properties": {
        "active_state_version": { "type": "string" },
        "capability_snapshot_version": { "type": "string" },
        "audience_graph_version": { "type": "string" }
      }
    },

    "summary": {
      "type": "object",
      "additionalProperties": false,
      "required": ["gate", "audience", "result"],
      "properties": {
        "gate": {
          "type": "object",
          "additionalProperties": false,
          "required": ["risk_r", "sensitivity_s", "max_gate", "approval_mode"],
          "properties": {
            "risk_r": { "type": "integer", "minimum": 0, "maximum": 4 },
            "sensitivity_s": { "type": "integer", "minimum": 0, "maximum": 4 },
            "max_gate": { "type": "integer", "minimum": 0, "maximum": 4 },
            "approval_mode": { "type": "string", "enum": ["none", "confirm", "diff", "oob_required", "refuse"] }
          }
        },
        "audience": {
          "type": "object",
          "additionalProperties": false,
          "required": ["requesting_audience_id", "sensitivity_ceiling_s"],
          "properties": {
            "requesting_audience_id": { "type": "string" },
            "sensitivity_ceiling_s": { "type": "integer", "minimum": 0, "maximum": 4 }
          }
        },
        "result": {
          "type": "string",
          "enum": ["completed", "failed", "cancelled", "blocked"],
          "description": "Final operation outcome."
        }
      }
    },

    "timeline": {
      "type": "array",
      "description": "Ordered event timeline. Each item references concrete artifacts stored elsewhere when needed.",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["ts", "event_type"],
        "properties": {
          "ts": { "type": "string", "format": "date-time" },
          "event_type": {
            "type": "string",
            "enum": [
              "operation_state_change",
              "gate_decision",
              "compiled_slice",
              "reasoning_output",
              "verification_pass",
              "verification_fail",
              "syscall_proposed",
              "syscall_permitted",
              "syscall_denied",
              "syscall_executed",
              "ipc_emit",
              "ipc_receive",
              "approval_requested",
              "approval_granted",
              "approval_denied",
              "oob_challenge_requested",
              "oob_challenge_verified",
              "sanitization_applied",
              "omissions_recorded"
            ]
          },
          "ref_id": { "type": "string", "description": "Optional link to a stored object (GateDecision, CompiledSlice, SyscallEnvelope, SyscallDeny, IPCArtifact)." },
          "note": { "type": "string" }
        }
      }
    },

    "attachments": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "gate_decision_ref": { "type": "string" },
        "compiled_slice_ref": { "type": "string" },
        "syscall_refs": { "type": "array", "items": { "type": "string" } },
        "ipc_artifact_refs": { "type": "array", "items": { "type": "string" } },
        "experience_log_refs": { "type": "array", "items": { "type": "string" } }
      }
    }
  }
}
```

---

### What Batch 3 now guarantees

* **Syscalls are fully traceable**: caller, target, intent, pinned versions, gate, taint-in, and results.
* **Denials are actionable**: constraint ids, triggering fields/refs, retry policy, remediation options (anti-retry trap).
* **IPC is safe and explicit**: artifacts carry sensitivity + taint + provenance; receiver inherits sensitivity.
* **Audit is replay-friendly**: pinned versions + timeline refs create deterministic debugging.
