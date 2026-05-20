# Evaluation Persistence Design (Aadesh Core)

Status: design document.

## 1) Purpose

Persist evaluation evidence in Aadesh so learning and future external analysis are grounded in durable structured data.

Design Lab is a separate consumer, not implemented here.

## 2) Required stored fields

Per evaluation run:
- run metadata (run id, timestamp, workspace scope, scenario id, harness version)
- baseline summary
- treatment summary
- judge summary
- failure cluster tags
- promotion/non-promotion decision

Recommended metadata:
- model identifiers used in baseline/treatment
- prompt template version
- scorer/judge method id

## 3) Storage shape recommendation

Hot-path relational payload:
- structured compact summaries only

Artifact references:
- raw judge transcripts
- long logs
- bulky outputs

Reason:
- keeps query surface fast and stable
- preserves raw material without bloating relational hot path

## 4) Write path

Primary writer:
- evaluation harness script/process

Write sequence:
1. persist structured run summary
2. persist artifact refs for bulky payloads
3. persist promotion decision linked to run id

## 5) Read path consumers

Inside Aadesh:
- ranking calibration features
- quality trend analysis and regression checks

Outside Aadesh (future Design Lab):
- pull structured summaries
- dereference raw artifacts when deep analysis is required

## 6) Data quality requirements

Each stored run must include:
- deterministic run id
- scope linkage
- baseline/treatment pair completeness
- explicit judge summary (no implicit null)
- explicit promotion decision enum

If raw artifacts are missing, run can still be valid if summary quality gates pass.

## 7) Retention

Durable:
- structured run summaries
- promotion decisions

Prunable:
- raw transcripts/logs referenced as artifacts

## 8) Non-goals

- no Design Lab analysis workflows in this repo
- no policy gating decisions in evaluation ingestion path
- no new public tool surface required for v0 persistence
