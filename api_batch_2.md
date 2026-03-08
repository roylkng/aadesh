Goal understood: deliver Batch 2 schemas: `GateDecision` and `CompiledSlice`, encoding max(R,S), scopes, approvals, taint, block budgets, omissions, and pinned versions. These must be strict enough that governance is enforceable and audit-friendly.

Below are the JSON Schemas (Draft 2020-12). These remain interface contracts, not DB tables.

---

```json
{
  "$id": "https://adesh.os/schemas/v0.1/GateDecision.json",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "GateDecision",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "operation_id",
    "isolation_id",
    "evaluated_at",
    "pinned",
    "risk",
    "sensitivity",
    "max_gate",
    "audience",
    "scopes",
    "constraints",
    "approval"
  ],
  "properties": {
    "operation_id": { "type": "string" },
    "isolation_id": { "type": "string" },
    "evaluated_at": { "type": "string", "format": "date-time" },

    "pinned": {
      "type": "object",
      "additionalProperties": false,
      "required": ["active_state_version", "capability_snapshot_version", "audience_graph_version"],
      "properties": {
        "active_state_version": {
          "type": "string",
          "description": "Pinned active state version used for this decision."
        },
        "capability_snapshot_version": {
          "type": "string",
          "description": "Pinned capability snapshot version used for this decision."
        },
        "audience_graph_version": {
          "type": "string",
          "description": "Pinned audience graph version used for this decision."
        }
      }
    },

    "risk": {
      "type": "object",
      "additionalProperties": false,
      "required": ["level", "predicates"],
      "properties": {
        "level": { "type": "integer", "minimum": 0, "maximum": 4, "description": "R0-R4." },
        "predicates": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Universal risk predicates that fired (e.g., sends_information_to_third_party)."
        }
      }
    },

    "sensitivity": {
      "type": "object",
      "additionalProperties": false,
      "required": ["level", "sources"],
      "properties": {
        "level": { "type": "integer", "minimum": 0, "maximum": 4, "description": "S0-S4." },
        "sources": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["kind", "ref_id"],
            "properties": {
              "kind": {
                "type": "string",
                "enum": ["attachment", "event_ref", "ipc_artifact", "tool_result", "inferred"]
              },
              "ref_id": { "type": "string" },
              "sensitivity_hint": { "type": "integer", "minimum": 0, "maximum": 4 }
            }
          },
          "description": "Evidence for sensitivity classification."
        }
      }
    },

    "max_gate": {
      "type": "integer",
      "minimum": 0,
      "maximum": 4,
      "description": "max(R,S) used for enforcement."
    },

    "audience": {
      "type": "object",
      "additionalProperties": false,
      "required": ["requesting_audience_id", "is_root_owner"],
      "properties": {
        "requesting_audience_id": { "type": "string" },
        "is_root_owner": { "type": "boolean" },
        "graph_version": {
          "type": "string",
          "description": "Audience Graph version used."
        }
      }
    },

    "scopes": {
      "type": "object",
      "additionalProperties": false,
      "required": ["allowed", "denied", "sensitivity_ceiling"],
      "properties": {
        "allowed": { "type": "array", "items": { "type": "string" } },
        "denied": { "type": "array", "items": { "type": "string" } },
        "sensitivity_ceiling": {
          "type": "integer",
          "minimum": 0,
          "maximum": 4,
          "description": "Max sensitivity allowed to disclose to this audience."
        }
      }
    },

    "constraints": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "negative_memory",
        "token_budgets",
        "taint_policy",
        "intent_anchor_required"
      ],
      "properties": {
        "negative_memory": {
          "type": "object",
          "additionalProperties": false,
          "required": ["never_store", "never_act", "do_not_assume"],
          "properties": {
            "never_store": { "type": "array", "items": { "type": "string" } },
            "never_act": { "type": "array", "items": { "type": "string" } },
            "do_not_assume": { "type": "array", "items": { "type": "string" } },
            "forget_expire": { "type": "array", "items": { "type": "string" } }
          }
        },

        "token_budgets": {
          "type": "object",
          "additionalProperties": false,
          "required": ["total", "blocks"],
          "properties": {
            "total": { "type": "integer", "minimum": 256 },
            "blocks": {
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
            }
          }
        },

        "taint_policy": {
          "type": "object",
          "additionalProperties": false,
          "required": ["propagate_max_sensitivity", "requires_sanitization_syscall"],
          "properties": {
            "propagate_max_sensitivity": { "type": "boolean", "const": true },
            "requires_sanitization_syscall": { "type": "boolean", "const": true }
          }
        },

        "intent_anchor_required": {
          "type": "boolean",
          "description": "Whether verification must enforce plan-trajectory alignment for this operation."
        }
      }
    },

    "approval": {
      "type": "object",
      "additionalProperties": false,
      "required": ["mode"],
      "properties": {
        "mode": {
          "type": "string",
          "enum": ["none", "confirm", "diff", "oob_required", "refuse"]
        },
        "reason": { "type": "string" },
        "confirm_prompt": { "type": "string" },
        "diff_template": {
          "type": "object",
          "additionalProperties": true,
          "description": "Template describing what a diff must contain for R3-class actions."
        },
        "oob": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "challenge_id": { "type": "string" },
            "required": { "type": "boolean" }
          }
        }
      }
    },

    "audit_trace_id": {
      "type": "string",
      "description": "Trace id linking decision, compilation, syscalls, and outcomes."
    }
  }
}
```

---

```json
{
  "$id": "https://adesh.os/schemas/v0.1/CompiledSlice.json",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "CompiledSlice",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "operation_id",
    "isolation_id",
    "compiled_at",
    "pinned",
    "gate",
    "intent_anchor",
    "blocks",
    "taint",
    "omissions",
    "provenance_summary",
    "audit_trace_id"
  ],
  "properties": {
    "operation_id": { "type": "string" },
    "isolation_id": { "type": "string" },
    "compiled_at": { "type": "string", "format": "date-time" },

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

    "gate": {
      "description": "Embedded gate decision summary required by the reasoning core.",
      "type": "object",
      "additionalProperties": false,
      "required": ["risk_level", "sensitivity_level", "max_gate", "approval_mode", "sensitivity_ceiling"],
      "properties": {
        "risk_level": { "type": "integer", "minimum": 0, "maximum": 4 },
        "sensitivity_level": { "type": "integer", "minimum": 0, "maximum": 4 },
        "max_gate": { "type": "integer", "minimum": 0, "maximum": 4 },
        "approval_mode": { "type": "string", "enum": ["none", "confirm", "diff", "oob_required", "refuse"] },
        "sensitivity_ceiling": { "type": "integer", "minimum": 0, "maximum": 4 }
      }
    },

    "intent_anchor": {
      "type": "object",
      "additionalProperties": false,
      "required": ["goal"],
      "properties": {
        "goal": { "type": "string" },
        "success_criteria": { "type": "array", "items": { "type": "string" } },
        "forbidden_outcomes": { "type": "array", "items": { "type": "string" } },
        "scope_limits": { "type": "array", "items": { "type": "string" } }
      }
    },

    "blocks": {
      "type": "object",
      "additionalProperties": false,
      "required": ["policy", "capability", "operation_context", "evidence", "scratch"],
      "properties": {
        "policy": {
          "type": "object",
          "additionalProperties": false,
          "required": ["token_budget", "content", "taint_s"],
          "properties": {
            "token_budget": { "type": "integer" },
            "content": { "type": "string", "description": "Non-truncatable governance and negative memory constraints." },
            "taint_s": { "type": "integer", "minimum": 0, "maximum": 4, "description": "Taint level for this block." }
          }
        },

        "capability": {
          "type": "object",
          "additionalProperties": false,
          "required": ["token_budget", "content", "taint_s"],
          "properties": {
            "token_budget": { "type": "integer" },
            "content": { "type": "string", "description": "Available sensors/actuators, budgets, limitations." },
            "taint_s": { "type": "integer", "minimum": 0, "maximum": 4 }
          }
        },

        "operation_context": {
          "type": "object",
          "additionalProperties": false,
          "required": ["token_budget", "content", "taint_s"],
          "properties": {
            "token_budget": { "type": "integer" },
            "content": { "type": "string", "description": "Minimal context relevant to this operation, filtered by audience and gate." },
            "taint_s": { "type": "integer", "minimum": 0, "maximum": 4 }
          }
        },

        "evidence": {
          "type": "object",
          "additionalProperties": false,
          "required": ["token_budget", "snippets", "taint_s"],
          "properties": {
            "token_budget": { "type": "integer" },
            "snippets": {
              "type": "array",
              "description": "Evidence snippets with provenance refs. May be empty if budget is 0.",
              "items": {
                "type": "object",
                "additionalProperties": false,
                "required": ["ref_id", "text", "sensitivity_s"],
                "properties": {
                  "ref_id": { "type": "string" },
                  "text": { "type": "string" },
                  "sensitivity_s": { "type": "integer", "minimum": 0, "maximum": 4 },
                  "provenance": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                      "source_class": { "type": "string" },
                      "artifact_ids": { "type": "array", "items": { "type": "string" } }
                    }
                  }
                }
              }
            },
            "taint_s": { "type": "integer", "minimum": 0, "maximum": 4 }
          }
        },

        "scratch": {
          "type": "object",
          "additionalProperties": false,
          "required": ["token_budget", "content", "taint_s", "expires_at"],
          "properties": {
            "token_budget": { "type": "integer" },
            "content": {
              "type": "string",
              "description": "Ephemeral working space. Never promoted directly."
            },
            "taint_s": { "type": "integer", "minimum": 0, "maximum": 4 },
            "expires_at": { "type": "string", "format": "date-time" }
          }
        }
      }
    },

    "taint": {
      "type": "object",
      "additionalProperties": false,
      "required": ["operation_max_taint_s", "sanitization_required_for_output"],
      "properties": {
        "operation_max_taint_s": {
          "type": "integer",
          "minimum": 0,
          "maximum": 4,
          "description": "Max taint across blocks; derived artifacts inherit this unless sanitized."
        },
        "sanitization_required_for_output": {
          "type": "boolean",
          "description": "Whether any output <= ceiling requires explicit sanitization syscall due to taint and audience ceilings."
        }
      }
    },

    "omissions": {
      "type": "object",
      "additionalProperties": false,
      "required": ["did_omit", "omitted_items"],
      "properties": {
        "did_omit": { "type": "boolean" },
        "omitted_items": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["block", "reason"],
            "properties": {
              "block": { "type": "string", "enum": ["policy", "capability", "operation_context", "evidence", "scratch"] },
              "reason": {
                "type": "string",
                "enum": ["token_budget_exceeded", "audience_scope_denied", "gate_confidence_threshold", "sensitivity_ceiling", "taint_policy"]
              },
              "details": { "type": "string" }
            }
          }
        }
      }
    },

    "provenance_summary": {
      "type": "object",
      "additionalProperties": false,
      "required": ["primitive_refs", "evidence_refs"],
      "properties": {
        "primitive_refs": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Ids of active-state primitives included in the slice."
        },
        "evidence_refs": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Refs to experience log entries/snippets used."
        }
      }
    },

    "audit_trace_id": { "type": "string" }
  }
}
```

### Batch 2 design notes (what these schemas enforce)

* `GateDecision` is authoritative and contains both **what fired** (predicates) and **what is permitted** (scopes + ceiling + approval mode).
* `CompiledSlice` embeds a minimal gate summary so the model cannot claim ignorance of constraints.
* Working memory blocks are explicit, token-bounded, and taint-labeled.
* `omissions` is first-class to make truncation auditable and prevent silent loss of governance.
* Evidence snippets carry sensitivity and provenance, enabling verification and taint propagation.
