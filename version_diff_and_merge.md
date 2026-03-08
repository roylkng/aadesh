# Version Diff, Merge, and Conflict Resolution Spec v0.1
Adesh OS

This document specifies how Adesh OS computes diffs between immutable versions and how it resolves merge conflicts for:
- Active State versions (persona/operational profile primitives)
- Audience Graph versions (nodes/edges/scopes)
- Capability Snapshot versions (tools and schemas)

It defines:
- canonical diff representation
- deterministic merge rules
- conflict detection and surfacing (review queue / approvals)
- constraints for safe merges (no silent disclosure expansion)

This is algorithmic logic. Not implementation code.

---

## 0) Core principles

1. **Versions are immutable**
A “merge” produces a new version. No in-place edits.

2. **Diffs are canonical**
A diff must be stable and deterministic across runs.

3. **No silent safety regression**
Any merge that expands disclosure, lowers safety gates, or enables new actuators must be explicitly approved (often R3/R4).

4. **Conflicts are first-class**
If merge is ambiguous, do not guess. Surface conflict as a review item.

---

## 1) Canonical Diff format

A diff is a list of patch operations:
- `op`: `add|remove|replace|move` (move optional)
- `path`: canonical JSON pointer-like path
- `before`: optional (for replace/remove)
- `after`: optional (for add/replace)
- `meta`:
  - `change_class`: `safe|sensitive|dangerous`
  - `requires_review`: bool
  - `requires_oob`: bool
  - `reason_codes[]`
  - `provenance_refs[]`

Paths must be stable and deterministic.

Example:
```json
[
  {
    "op": "replace",
    "path": "/active_state/primitives/preferences/meeting_time",
    "before": { "value": "after_10am", "context": {} },
    "after": { "value": "9am_tues_q4", "context": { "quarter": "Q4" } },
    "meta": {
      "change_class": "sensitive",
      "requires_review": true,
      "requires_oob": false,
      "reason_codes": ["preference.affects_actuator"],
      "provenance_refs": ["event:..."]
    }
  }
]
````

---

## 2) Active State diff and merge

### 2.1 Primitive identity

Each primitive has:

* stable `primitive_id`
* `type`, `key`
* time bounds and context predicates

Preferred diff base:

* compare by `primitive_id`
  Fallback:
* compare by `(type,key,time_bounds_start,context_predicates_hash)`

### 2.2 Diff rules

* Add: new primitive_id appears
* Remove: primitive marked deprecated (preferred) or removed (rare)
* Replace: same primitive_id but value/metadata changes

### 2.3 Merge policy (deterministic)

Given base version `B`, and two branches `L` and `R`:

* For primitives with different keys: auto-merge (no conflict)
* For same primitive_id:

  * if both made identical change: accept
  * if one changed and other did not: accept changed
  * if both changed differently: conflict

### 2.4 Conflict resolution

Conflicts become review items:

* include both candidate states
* include evidence refs and time/context
* never auto-resolve conflicts that affect:

  * boundaries
  * actuator-affecting preferences
  * disclosure rules

---

## 3) Audience Graph diff and merge

### 3.1 Node/edge identity

Nodes keyed by `node_id`.
Edges keyed by `(src_id,dst_id,edge_type)` or edge_id if stable.

### 3.2 Diff rules

* Adding nodes/edges: add ops
* Changing policy scopes/ceilings: replace ops on policy path

### 3.3 Safety constraints (no silent expansion)

Any diff that:

* adds a new edge from root_owner to non-root node
* increases `sensitivity_ceiling_s`
* adds wildcard scope `*`
* adds new allowed scopes
  must be marked:
* `change_class=dangerous`
* `requires_review=true`
* `requires_oob=true` when ceiling reaches S3/S4 or wildcard is involved

### 3.4 Merge policy

* Non-overlapping changes merge
* Conflicts when both branches edit same edge policy:

  * different ceilings
  * different allowed scopes sets
    Conflict => review queue, never auto-resolve.

---

## 4) Capability Snapshot diff and merge

### 4.1 Identity

Tool keyed by `(kind,name)`.

### 4.2 Diff rules

* enable/disable tool: replace tool status
* schema_ref change: replace schema_ref
* risk floor change: replace risk metadata

### 4.3 Safety constraints

Any diff that:

* enables an actuator with external side effects
* raises risk floors downward (less strict)
* adds new actuators
  must be `dangerous` and reviewed. OOB for critical actuators.

### 4.4 Merge policy

* If both enable same tool but with different schema_ref => conflict
* If one disables and other enables => conflict
  Conflicts => review queue.

---

## 5) Conflict surfacing and UX requirements

Conflicts must be represented as ReviewItems with:

* base version id
* left and right candidate changes
* computed diff for each branch
* recommended safe action (often “deny expansion”)
* impact analysis

---

## 6) Deterministic hashing and comparison

To keep diffs stable:

* canonicalize JSON before hashing
* stable sorting of keys and arrays (when order is not meaningful)
* store `content_hash` for version snapshots

---

## 7) Minimum test cases (must pass)

1. Active State conflicting edit:

* two branches modify same boundary differently -> conflict item generated.

2. Audience Graph scope expansion:

* add new scope -> requires review and possibly OOB.

3. Capability enablement:

* enable external actuator -> requires review and approval; merge conflicts not auto-resolved.

4. Deterministic diff:

* same inputs produce identical diff ordering and paths.

```
