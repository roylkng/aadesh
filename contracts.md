Goal understood: lock the Gateway decision (HTTP primary, MCP secondary) and then produce the first three kernel interface contracts as concrete JSON Schemas.

## 1) Gateway decision

Agree with the review.

* **Primary interface**: Localhost HTTP API (REST + WebSockets) for:

  * Owner UI CRUD (Audience Graph, policies, review queue)
  * approvals and diffs
  * async notifications (state changes, pending approvals, reflection results)
  * diagnostics (audit trace browsing, replay)

* **Secondary bridge**: MCP Host for agent-to-agent integration:

  * OpenClaw, Claude Desktop, other agents can delegate to the OS
  * they are treated as an `audience_id` and fully constrained by default deny

This is the correct split: MCP is optimal for tool semantics, not for a control plane.

---

## 2) Kernel API contracts v0.1 (JSON Schema)

Below are strict schemas for:

1. `OperationSpec` and `GateDecision`
2. `CompiledSlice`
3. `SyscallEnvelope` (with policy-aware rejections)

These are written to be language-agnostic and implementable on Pi and workstation.

---

### 2.1 OperationSpec and GateDecision

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://agentos.local/schemas/operation_spec.json",
  "title": "OperationSpec",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "operation_id",
    "request_id",
    "isolation_id",
    "requesting_audience_id",
    "intent_anchor",
    "operation_intent",
    "created_at"
  ],
  "properties": {
    "operation_id": { "type": "string", "minLength": 8 },
    "request_id": { "type": "string", "minLength": 8 },
    "parent_operation_id": { "type": ["string", "null"] },
    "isolation_id": { "type": "string", "minLength": 8 },
    "requesting_audience_id": { "type": "string", "minLength": 1 },

    "created_at": { "type": "string", "format": "date-time" },

    "operation_intent": {
      "type": "object",
      "additionalProperties": false,
      "required": ["goal_text"],
      "properties": {
        "goal_text": { "type": "string", "minLength": 1 },
        "constraints_text": { "type": ["string", "null"] },

        "proposed_actions": {
          "type": "array",
          "items": { "$ref": "https://agentos.local/schemas/action_descriptor.json" },
          "default": []
        },

        "data_handles": {
          "description": "References to inputs the caller believes are relevant (doc ids, file paths, message ids). Not direct content.",
          "type": "array",
          "items": { "$ref": "https://agentos.local/schemas/data_handle.json" },
          "default": []
        },

        "requested_outputs": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["type"],
            "properties": {
              "type": { "type": "string", "minLength": 1 },
              "format": { "type": ["string", "null"] }
            }
          },
          "default": []
        }
      }
    },

    "intent_anchor": {
      "description": "Structured anchor for plan-trajectory alignment. Derived from Root Owner intent.",
      "type": "object",
      "additionalProperties": false,
      "required": ["objective", "success_criteria", "forbidden_outcomes"],
      "properties": {
        "objective": { "type": "string", "minLength": 1 },
        "success_criteria": { "type": "array", "items": { "type": "string" }, "default": [] },
        "forbidden_outcomes": { "type": "array", "items": { "type": "string" }, "default": [] },
        "scope_limits": { "type": "array", "items": { "type": "string" }, "default": [] }
      }
    },

    "pinned": {
      "description": "Filled after compilation.",
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "active_state_version": { "type": "string" },
        "capability_snapshot_version": { "type": "string" }
      }
    },

    "budgets": {
      "description": "Per-operation budgets; may be defaults derived by policy if omitted.",
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "token_budget_total": { "type": "integer", "minimum": 256 },
        "latency_budget_ms": { "type": "integer", "minimum": 1 },
        "cost_budget_cents": { "type": "integer", "minimum": 0 },
        "max_syscalls": { "type": "integer", "minimum": 0 }
      }
    },

    "context_predicates_hint": {
      "description": "Caller hints only. OS determines truth.",
      "type": "object",
      "additionalProperties": true
    }
  }
}
```

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://agentos.local/schemas/gate_decision.json",
  "title": "GateDecision",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "operation_id",
    "action_risk",
    "data_sensitivity",
    "max_gate",
    "approval_mode",
    "allowed_scopes",
    "denied_scopes",
    "taint_ceiling",
    "policy_hits",
    "audit_trace_id",
    "computed_at"
  ],
  "properties": {
    "operation_id": { "type": "string" },

    "action_risk": { "type": "string", "enum": ["R0", "R1", "R2", "R3", "R4"] },
    "data_sensitivity": { "type": "string", "enum": ["S0", "S1", "S2", "S3", "S4"] },

    "max_gate": { "type": "integer", "minimum": 0, "maximum": 4 },

    "approval_mode": {
      "type": "string",
      "enum": ["none", "confirm", "diff_approve", "oob_required", "refuse"]
    },

    "approval_prompt": { "type": ["string", "null"] },

    "allowed_scopes": { "type": "array", "items": { "type": "string" }, "default": [] },
    "denied_scopes": { "type": "array", "items": { "type": "string" }, "default": [] },

    "taint_ceiling": {
      "description": "Maximum sensitivity permitted in working memory/output for this operation under audience constraints.",
      "type": "string",
      "enum": ["S0", "S1", "S2", "S3", "S4"]
    },

    "policy_hits": {
      "description": "Deterministic reasons used to compute the gate and constraints.",
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["policy_id", "kind", "message"],
        "properties": {
          "policy_id": { "type": "string" },
          "kind": {
            "type": "string",
            "enum": [
              "audience_scope_deny",
              "negative_memory_never_act",
              "negative_memory_do_not_assume",
              "sensitivity_ceiling",
              "risk_floor",
              "budget_limit",
              "self_mod_block"
            ]
          },
          "message": { "type": "string" },
          "trigger": { "type": ["object", "null"], "additionalProperties": true }
        }
      },
      "default": []
    },

    "audit_trace_id": { "type": "string" },
    "computed_at": { "type": "string", "format": "date-time" }
  }
}
```

Supporting schemas referenced above:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://agentos.local/schemas/data_handle.json",
  "title": "DataHandle",
  "type": "object",
  "additionalProperties": false,
  "required": ["kind", "ref"],
  "properties": {
    "kind": { "type": "string", "enum": ["file", "doc", "message", "url", "artifact", "db_record"] },
    "ref": { "type": "string", "minLength": 1 },
    "sensitivity_hint": { "type": ["string", "null"], "enum": ["S0", "S1", "S2", "S3", "S4", null] }
  }
}
```

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://agentos.local/schemas/action_descriptor.json",
  "title": "ActionDescriptor",
  "type": "object",
  "additionalProperties": false,
  "required": ["type"],
  "properties": {
    "type": { "type": "string", "minLength": 1 },
    "target": { "type": ["string", "null"] },
    "details": { "type": ["object", "null"], "additionalProperties": true }
  }
}
```

---

### 2.2 CompiledSlice (what JIT feeds Reasoning Core)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://agentos.local/schemas/compiled_slice.json",
  "title": "CompiledSlice",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "operation_id",
    "isolation_id",
    "pinned_state",
    "gate_decision",
    "blocks",
    "token_budgets",
    "omissions",
    "taint",
    "audit_trace_id",
    "compiled_at"
  ],
  "properties": {
    "operation_id": { "type": "string" },
    "isolation_id": { "type": "string" },

    "pinned_state": {
      "type": "object",
      "additionalProperties": false,
      "required": ["active_state_version", "capability_snapshot_version"],
      "properties": {
        "active_state_version": { "type": "string" },
        "capability_snapshot_version": { "type": "string" }
      }
    },

    "gate_decision": { "$ref": "https://agentos.local/schemas/gate_decision.json" },

    "token_budgets": {
      "type": "object",
      "additionalProperties": false,
      "required": ["total", "policy", "capability", "op_context", "evidence", "scratch"],
      "properties": {
        "total": { "type": "integer", "minimum": 256 },
        "policy": { "type": "integer", "minimum": 64 },
        "capability": { "type": "integer", "minimum": 32 },
        "op_context": { "type": "integer", "minimum": 32 },
        "evidence": { "type": "integer", "minimum": 0 },
        "scratch": { "type": "integer", "minimum": 0 }
      }
    },

    "blocks": {
      "type": "object",
      "additionalProperties": false,
      "required": ["policy", "capability", "op_context", "evidence", "scratch"],
      "properties": {
        "policy": { "$ref": "https://agentos.local/schemas/memory_block.json" },
        "capability": { "$ref": "https://agentos.local/schemas/memory_block.json" },
        "op_context": { "$ref": "https://agentos.local/schemas/memory_block.json" },
        "evidence": { "$ref": "https://agentos.local/schemas/memory_block.json" },
        "scratch": { "$ref": "https://agentos.local/schemas/memory_block.json" }
      }
    },

    "omissions": {
      "description": "What was omitted due to budget, scope, confidence, or taint constraints.",
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["reason", "count"],
        "properties": {
          "reason": {
            "type": "string",
            "enum": ["token_budget", "audience_scope", "low_confidence", "taint_ceiling", "policy_deny"]
          },
          "count": { "type": "integer", "minimum": 1 },
          "examples": { "type": "array", "items": { "type": "string" }, "default": [] }
        }
      },
      "default": []
    },

    "taint": {
      "description": "Taint tracking for cognitive integrity.",
      "type": "object",
      "additionalProperties": false,
      "required": ["operation_taint", "block_taint"],
      "properties": {
        "operation_taint": { "type": "string", "enum": ["S0", "S1", "S2", "S3", "S4"] },
        "block_taint": {
          "type": "object",
          "additionalProperties": false,
          "required": ["policy", "capability", "op_context", "evidence", "scratch"],
          "properties": {
            "policy": { "type": "string", "enum": ["S0", "S1", "S2", "S3", "S4"] },
            "capability": { "type": "string", "enum": ["S0", "S1", "S2", "S3", "S4"] },
            "op_context": { "type": "string", "enum": ["S0", "S1", "S2", "S3", "S4"] },
            "evidence": { "type": "string", "enum": ["S0", "S1", "S2", "S3", "S4"] },
            "scratch": { "type": "string", "enum": ["S0", "S1", "S2", "S3", "S4"] }
          }
        }
      }
    },

    "audit_trace_id": { "type": "string" },
    "compiled_at": { "type": "string", "format": "date-time" }
  }
}
```

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://agentos.local/schemas/memory_block.json",
  "title": "MemoryBlock",
  "type": "object",
  "additionalProperties": false,
  "required": ["block_type", "content", "provenance_refs", "sensitivity", "token_count_estimate"],
  "properties": {
    "block_type": {
      "type": "string",
      "enum": ["policy", "capability", "op_context", "evidence", "scratch"]
    },
    "content": {
      "description": "The textual or structured content injected to the reasoning core. Keep small and deterministic.",
      "type": ["string", "object"],
      "additionalProperties": true
    },
    "provenance_refs": {
      "description": "References into the Experience Log or Active State that justify content.",
      "type": "array",
      "items": { "type": "string" },
      "default": []
    },
    "sensitivity": { "type": "string", "enum": ["S0", "S1", "S2", "S3", "S4"] },
    "token_count_estimate": { "type": "integer", "minimum": 0 }
  }
}
```

---

### 2.3 SyscallEnvelope (request/response + policy-aware denial)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://agentos.local/schemas/syscall_envelope.json",
  "title": "SyscallEnvelope",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "operation_id",
    "isolation_id",
    "syscall_id",
    "kind",
    "target",
    "request",
    "context"
  ],
  "properties": {
    "operation_id": { "type": "string" },
    "isolation_id": { "type": "string" },
    "syscall_id": { "type": "string" },

    "kind": {
      "type": "string",
      "enum": ["sensor_read", "memory_read", "ipc_send", "ipc_recv", "actuator_call", "sanitize"]
    },

    "target": {
      "description": "Tool identifier or internal service name.",
      "type": "string",
      "minLength": 1
    },

    "request": {
      "description": "Tool-specific payload, must validate against registered schema for the target.",
      "type": "object",
      "additionalProperties": true
    },

    "context": {
      "type": "object",
      "additionalProperties": false,
      "required": ["gate", "audience", "pinned_state_version", "taint"],
      "properties": {
        "gate": {
          "type": "object",
          "additionalProperties": false,
          "required": ["action_risk", "data_sensitivity", "max_gate"],
          "properties": {
            "action_risk": { "type": "string", "enum": ["R0", "R1", "R2", "R3", "R4"] },
            "data_sensitivity": { "type": "string", "enum": ["S0", "S1", "S2", "S3", "S4"] },
            "max_gate": { "type": "integer", "minimum": 0, "maximum": 4 }
          }
        },
        "audience": {
          "type": "object",
          "additionalProperties": false,
          "required": ["requesting_audience_id", "allowed_scopes"],
          "properties": {
            "requesting_audience_id": { "type": "string" },
            "allowed_scopes": { "type": "array", "items": { "type": "string" }, "default": [] }
          }
        },
        "pinned_state_version": { "type": "string" },
        "taint": { "type": "string", "enum": ["S0", "S1", "S2", "S3", "S4"] }
      }
    }
  }
}
```

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://agentos.local/schemas/syscall_result.json",
  "title": "SyscallResult",
  "type": "object",
  "additionalProperties": false,
  "required": ["syscall_id", "status", "audit_trace_id", "completed_at"],
  "properties": {
    "syscall_id": { "type": "string" },
    "status": { "type": "string", "enum": ["allowed", "denied", "executed", "failed"] },

    "response": {
      "description": "Present when executed and successful.",
      "type": ["object", "null"],
      "additionalProperties": true
    },

    "error": {
      "description": "Present when failed.",
      "type": ["object", "null"],
      "additionalProperties": false,
      "properties": {
        "code": { "type": "string" },
        "message": { "type": "string" },
        "retryable": { "type": "boolean" }
      }
    },

    "denial": {
      "description": "Policy-aware denial payload to prevent infinite retry loops.",
      "type": ["object", "null"],
      "additionalProperties": false,
      "properties": {
        "violations": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["constraint_id", "kind", "message"],
            "properties": {
              "constraint_id": { "type": "string" },
              "kind": {
                "type": "string",
                "enum": [
                  "negative_memory_never_act",
                  "negative_memory_never_store",
                  "negative_memory_do_not_assume",
                  "audience_scope_deny",
                  "gate_requires_approval",
                  "taint_ceiling_violation",
                  "risk_floor_violation",
                  "budget_violation",
                  "self_mod_block"
                ]
              },
              "message": { "type": "string" },
              "trigger_fields": { "type": "array", "items": { "type": "string" }, "default": [] },
              "trigger_data_classes": { "type": "array", "items": { "type": "string" }, "default": [] },
              "trigger_sensitivity": { "type": ["string", "null"], "enum": ["S0", "S1", "S2", "S3", "S4", null] }
            }
          }
        },

        "retry": {
          "type": "object",
          "additionalProperties": false,
          "required": ["allowed"],
          "properties": {
            "allowed": { "type": "boolean" },
            "conditions": { "type": "array", "items": { "type": "string" }, "default": [] }
          }
        },

        "remediation": {
          "description": "Permitted remediation paths the reasoning core can take.",
          "type": "array",
          "items": {
            "type": "string",
            "enum": ["ask_user", "request_owner_approval", "show_diff", "sanitize", "use_alternate_actuator", "refuse"]
          },
          "default": []
        }
      }
    },

    "audit_trace_id": { "type": "string" },
    "completed_at": { "type": "string", "format": "date-time" }
  }
}
```