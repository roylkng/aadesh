# Aadesh vs Design Lab Boundary

Status: boundary note.

## Aadesh scope (this repo)

Aadesh owns:
- continuity and memory substrate
- intervention/outcome traces
- evaluation persistence primitives
- retrieval/ranking context preparation for host agents

Aadesh does not own in this phase:
- policy experimentation workflows
- decision lab orchestration
- comparative policy program execution
- broad controller behavior

## Design Lab scope (separate repo/folder)

Design Lab owns:
- policy experimentation and comparative analysis workflows
- deeper benchmark analysis across runs/variants
- promotion policy studies and decision support

Design Lab consumes Aadesh outputs:
- structured evaluation summaries
- intervention/outcome traces
- linked raw artifacts via refs

Conceptual relationship:
- Design Lab remains a separate evaluator and experimentation environment.
- Aadesh is not merely a data exhaust layer for Design Lab.
- Aadesh remains the durable substrate that Design Lab can consume and later build upon.

## Interface contract between them

Recommended handoff contract:
- Aadesh exports structured summaries with stable ids and scope links
- raw bulky evidence remains artifact-addressable
- Design Lab reads and analyzes; it does not mutate Aadesh core semantics directly

This boundary keeps Aadesh focused on substrate reliability while Design Lab evolves independently.
