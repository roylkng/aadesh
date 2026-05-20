# Aadesh Memory Contract vNext (Design Only)

Status: design document.
Authority: defines memory semantics and boundaries for the next Aadesh evolution.

## 1) Purpose

Define memory semantics independent of storage backend choice.

This contract answers:
- what memory classes exist
- what writes each class
- what reads each class
- how confidence/promotion/supersession works
- what stays inside Aadesh vs external artifacts

## 2) Memory classes

## 2.1 Episodic memory

Represents bounded work events.

Examples:
- task prompt
- summary
- files/artifacts touched
- tests observed
- unresolved items reported at episode close

Primary write source:
- `store_work_episode`
- connector `task_checkpoint` / `task_end`

Primary consumers:
- `prepare_task_context`
- `recall_relevant_memory`
- ranking features

## 2.2 Semantic memory

Represents stable guidance claims inferred or asserted over episodes.

Examples:
- decisions and rationale
- workspace preferences
- recurring risk patterns

Primary write source:
- extraction from episodic summaries/tests/artifacts
- explicit decision/preference input

Primary consumers:
- `prepare_task_context` decisions/preferences/risks

## 2.3 Procedural memory

Represents reusable process patterns.

Examples:
- preferred validation sequence
- recurring checklist for specific change classes

Primary write source:
- repeated aligned episode patterns

Primary consumers:
- likely next directions
- risk review/ranking heuristics

## 2.4 Intervention/outcome memory

Represents surfaced suggestions and what happened next.

Core fields:
- surfaced suggestion identity
- context identity
- adoption outcome (`accepted|ignored|modified`)
- later outcome and correction signals

Primary write source:
- connector trace payloads
- post-task store payloads

Primary consumers:
- ranking policy calibration
- supervisory observability reports

## 2.5 Evaluation memory

Represents benchmark evidence.

Core fields:
- run metadata
- baseline/treatment summary
- judge summary
- failure tags
- promotion/non-promotion decision

Primary write source:
- evaluation harness ingestion

Primary consumers:
- longitudinal quality tracking
- advisory learning inputs
- external Design Lab consumption via exported summaries

## 2.6 Optional future policy-state memory

Represents active/candidate policy state and mutation history.

Only activate if traces + eval persistence cannot express needed policy semantics.

## 3) Write triggers

- Host before-task guidance call:
  - read-only for memory, may emit context id for trace linkage
- Host checkpoint/end event:
  - writes episodic observations
  - writes intervention outcome signals if provided
- Evaluation run completion:
  - writes evaluation memory summary
  - stores bulky judge transcript/logs as artifact refs

## 4) Retrieval consumers and read paths

`prepare_task_context` reads:
- episodic (recent relevant events)
- semantic (decisions/preferences/risks)
- procedural (repeated practice patterns)
- intervention/outcome (what suggestions worked here)
- evaluation (only compact confidence calibration features)

`recall_relevant_memory` reads:
- episodic + semantic primarily
- intervention/evaluation on explicit request or relevance match

ranking consumes:
- semantic confidence/evidence
- unresolved episodic loops
- intervention outcome patterns
- evaluation failure clusters

## 5) Promotion rules

Base states:
- observation
- candidate
- confirmed
- superseded

Rules:
- explicit user/host decision with evidence -> candidate immediately, confirm with corroboration or explicit reaffirmation
- repeated aligned signals in same scope -> candidate to confirmed
- inferred one-off signal remains candidate
- conflicting newer confirmed memory may supersede older confirmed memory with evidence link

Alignment key in v0:
- same scope key
- same memory class
- same normalized subject key
- non-contradictory normalized statement key

## 6) Confidence and provenance requirements

Every surfaced memory item must include:
- confidence
- evidence refs
- basis (`explicit|inferred|artifact-backed|repeated-pattern`)

No evidence ref means item is advisory-low confidence and must not outrank evidence-backed confirmed memory.

## 7) Contradiction and supersession

When contradictory guidance appears in same scope/subject:
- keep both records with conflict linkage
- prefer newer confirmed memory when evidence is stronger or equally strong but more recent
- mark prior memory superseded when resolution is strong
- if unresolved, surface uncertainty instead of hiding conflict

## 8) Decay and retention

Durable by default:
- normalized episodic summaries
- semantic/procedural memory
- intervention/outcome records
- evaluation summaries

Prunable:
- bulky raw transcripts/logs/large artifacts

Decay policy:
- confidence decay can reduce ranking weight over time
- records remain queryable for audit and longitudinal analysis

## 9) Aadesh boundary vs external artifacts

Inside Aadesh hot path:
- normalized structured memory records
- confidence/evidence links
- ranking features

Outside hot path (artifact store refs):
- full judge transcripts
- long raw tool logs
- bulky diff outputs

## 10) Backend choice separation (MemPalace / MemPlus)

Aadesh memory contract is backend-agnostic.

Separate concerns:
- Aadesh semantics: memory classes, confidence, promotion, supersession
- storage/index backend: SQLite + FTS, MemPalace, MemPlus, or hybrid
- retrieval/ranking policy: scoring and selection logic in cognition core

MemPalace/MemPlus can be adopted as indexing/storage aids only if they preserve Aadesh semantics and evidence/provenance guarantees.
