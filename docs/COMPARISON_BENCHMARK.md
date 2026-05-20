# External Memory Comparison Benchmark

Status: active validation aid.
Authority: comparison protocol for Aadesh vs adjacent memory/context systems.

## Purpose

Aadesh should not be accepted as useful just because it remembers facts. Existing tools already do that. This benchmark tests the narrower wedge:

Aadesh is useful when its continuity and supervisory traces produce better task guidance than baseline memory recall, and when that advantage remains visible against memory-oriented systems such as memd, Knowns, OpenMemory, and Hermes.

The comparison must keep five dimensions separate:
- memory recall quality
- next-direction quality
- setup friction
- cross-host portability
- outcome-trace learning

Aadesh only wins strategically if it shows value beyond generic memory recall. If another system matches recall but lacks intervention/outcome learning, Aadesh should narrow further around supervisory traces, evaluation persistence, and advisory ranking rather than expanding into a generic memory server or Hermes-like runtime.

## Systems To Compare

Minimum comparison set:
- baseline agent with empty Aadesh DB
- Aadesh treatment with seeded continuity and outcome memory
- Hermes Agent as a host/runtime comparator
- Knowns or OpenMemory as a memory-layer comparator, when safely available

Optional:
- memd
- OpenMemory/Mem0
- project-specific static docs or AGENTS.md baseline

## Current Script

```bash
./scripts/external_memory_comparison_harness.sh \
  --include-external-stubs \
  --run-hermes-probe \
  --run-memory-layer-probe
```

The script always runs:
- `baseline`
- `aadesh`

It can also:
- run a real local Hermes runtime probe in an isolated `HERMES_HOME`
- run a scored Hermes benchmark against the same tasks with `--run-hermes-benchmark`
- probe local availability of Knowns/OpenMemory/memd without installing or fetching them
- run a scored direct mem0/OpenMemory comparator with `--run-openmemory-direct-benchmark`
- run the harder multi-week judge layer with `--run-hard-supervisory-benchmark`
- create not-run slots for memory-layer systems when adapters are not available
- import scored external reports using `--external-result NAME=PATH`

Stub slots are intentionally not treated as evidence until real exported results are provided.

Current memory-layer safety rule:
- do not execute unverified package-manager commands such as `npx knowns` inside the repo
- run Knowns/OpenMemory only when installed locally or isolated in an explicitly approved environment
- if OpenMemory requires Docker and Docker is unavailable, record it as blocked rather than weakening sandbox boundaries

Direct OpenMemory comparator:

```bash
OPENMEMORY_LMSTUDIO_BASE_URL=http://127.0.0.1:1234/v1 \
OPENMEMORY_LMSTUDIO_CHAT_URL=http://127.0.0.1:1234/api/v1/chat \
OPENMEMORY_CONTAINER_LMSTUDIO_BASE_URL=http://host.docker.internal:1234/v1 \
OPENMEMORY_LLM_MODEL=qwen/qwen3.6-27b \
OPENMEMORY_EMBED_MODEL=text-embedding-nomic-embed-text-v1.5 \
OPENMEMORY_EMBED_DIMS=768 \
./scripts/external_memory_comparison_harness.sh \
  --run-openmemory-direct-benchmark
```

This mode uses local Docker images only:
- `mem0/openmemory-mcp:latest`
- `qdrant/qdrant:latest`

It seeds mem0/OpenMemory with the same comparison memories using `infer=false`, retrieves memories through Qdrant, then asks the local LM Studio chat model to produce guidance from only the retrieved memories. It does not exercise OpenMemory as an intervention/outcome trace system.

`OPENMEMORY_LMSTUDIO_BASE_URL` remains the OpenAI-compatible endpoint used for model/embedding availability and mem0 configuration. `OPENMEMORY_LMSTUDIO_CHAT_URL` is the host chat endpoint used to generate scored comparator answers.

## Real Trace Validation

After the synthetic/hard comparator passes, validate against captured host traces instead of adding more generic memory features:

```bash
./scripts/real_trace_validation_harness.sh \
  --db-path .aadesh/session.db \
  --output-dir /tmp/adesh-real-trace-validation
```

For a deterministic local proof of the validator itself:

```bash
./scripts/real_trace_validation_harness.sh \
  --seed-fixture \
  --strict \
  --output-dir /tmp/adesh-real-trace-validation-fixture
```

For a connector-path smoke that writes linked `accepted`, `ignored`, and `modified` outcomes before validating:

```bash
./scripts/real_trace_validation_smoke.sh \
  --output-dir /tmp/adesh-real-trace-smoke
```

For a broader simulated multi-host run across Codex, Qwen Code, and OpenCode-style hosts:

```bash
./scripts/real_trace_multihost_simulation.sh \
  --output-dir /tmp/adesh-real-trace-multihost
```

This simulation intentionally includes one degraded unlinked trace. The run should still pass only if linked traces remain learnable, degraded traces remain unlearnable, and real-trace guidance stays grounded.

For an installed-CLI host-friction check that invokes Qwen, OpenCode, Gemini, and Codex when available:

```bash
./scripts/live_cli_trace_validation.sh \
  --output-dir /tmp/adesh-live-cli-trace
```

This live check is intentionally non-destructive. It records completed CLI turns as connector traces, stores blocked/failed CLI attempts as report evidence, and still relies on the deterministic real-trace validator for pass/fail.

By default, the Qwen CLI path targets a local OpenAI-compatible LM Studio endpoint at `http://127.0.0.1:1234/v1` with model `qwen/qwen3.6-27b`. Override with `LMSTUDIO_BASE_URL`, `LIVE_CLI_MODEL`, `QWEN_OPENAI_BASE_URL`, or `QWEN_MODEL` when needed.

To include this as an explicit optional gate inside the comparison workflow:

```bash
./scripts/external_memory_comparison_harness.sh \
  --run-live-cli-trace \
  --live-cli-timeout 180
```

This harness reads recent stored episodes, derives case expectations from captured decisions/open loops/preferences, reruns `prepare_task_context`, and reports whether Aadesh surfaces grounded guidance from that captured data. It also summarizes linked intervention outcomes when the DB contains Phase B traces.

Use this as the bridge from synthetic benchmark confidence to real host-session evidence. A strong result here supports the supervisory-continuity wedge; a weak result should drive trace ingestion or ranking fixes, not a broad memory-server expansion.

## Hard Supervisory Benchmark

Use this after at least one real comparator run:

```bash
./scripts/hard_supervisory_comparison_benchmark.sh \
  --comparison-report /tmp/adesh-external-comparison-.../comparison_report.json \
  --days 14 \
  --sessions 12 \
  --stress-events 36 \
  --data-profile production
```

Or run it as an opt-in follow-up from the comparison harness:

```bash
./scripts/external_memory_comparison_harness.sh \
  --run-hermes-benchmark \
  --run-openmemory-direct-benchmark \
  --run-hard-supervisory-benchmark \
  --stress-events 36 \
  --data-profile production
```

The hard layer adds:
- multi-week noisy usage through `multiday_supervisory_usage_simulation.sh`
- deep guidance probes through `deep_supervisory_guidance_benchmark.sh`
- adversarial long-memory stress through `adversarial_long_memory_stress_benchmark.sh`
- a local judge-style report that separates memory recall, next-direction quality, noisy temporal behavior, outcome-history behavior, cross-host portability, and outcome-trace learning

The `production` data profile is the default for the hard layer. It adds noisier production-like cases:
- release-only CI flake evidence before cleanup
- PR review feedback about connector `context_id` round trips
- blocked external comparator runs recorded as blocked/not-run environment evidence
- cross-workspace noise that must not dominate unrelated task guidance

The production profile also emits `production_case_report`, which records each synthetic case's failure mode, expected evidence, observed top outputs, assertion status, and diagnostic. The hard benchmark turns that into `production_case_judgments`, a deterministic case-level usefulness check. This keeps the synthetic benchmark inspectable instead of hiding behind one aggregate pass/fail score.

When `--judge-mode lmstudio` is used, the LM Studio judge receives the production case report and deterministic case judgments in addition to the aggregate system scores. That judge output is advisory evidence only; the deterministic benchmark gate remains the source of pass/fail.

The adversarial stress layer injects noisy/confusable traces across multiple hosts and workspaces, probes the same target task at increasing memory load, and emits a `degradation_curve`. The hard benchmark fails if current-task guidance drops below the evidence threshold or if unrelated workspace noise leaks into the target task.

This is still not a production proof. It is a stronger validation gate for the current wedge: Aadesh must keep recall and guidance quality while proving supervisory behavior that memory-only systems do not exercise.

See `docs/COMPETITOR_TESTING_NOTES.md` for how this shape relates to Hermes and OpenMemory/Mem0 testing practice.

## Comparator Classes

Hermes is a `host_runtime` comparator. It should answer:
- can a full agent runtime solve the same task with its own memory/skills/session search?
- how much setup is required?
- does it expose enough hooks to treat Aadesh as a host-neutral substrate?

Knowns/OpenMemory/memd are `memory_layer` comparators. They should answer:
- can a memory layer recall decisions, preferences, open loops, and risks?
- can it produce next-direction guidance, or only raw recall?
- does it preserve accepted/ignored/modified outcome traces?

Aadesh is the `supervisory_substrate` treatment. It should answer:
- does cross-host continuity improve current task guidance?
- do linked outcomes and eval evidence create value beyond recall?
- can the same substrate work across different host agents?

## Current Stub Slots

The script creates not-run slots for:
- `memd`
- `knowns`
- `openmemory`
- `hermes`, unless `--run-hermes-probe` is used

## Scenario Shape

The harness uses nine tasks across three workspaces:
- payment retry reliability
- connector/supervisory trace quality
- external comparison proof work

The seeded Aadesh memory includes:
- decisions
- open loops
- preferences
- risks
- stale/conflicting guidance
- sparse-host trace concerns
- explicit comparison/evaluation pressure

This is designed to test whether Aadesh surfaces what matters now, not merely whether it can recall one obvious prior fact.

## Scoring Fields

Each task scores five binary checks:
- expected decision recalled
- expected open loop recalled
- expected preference recalled
- expected next direction surfaced
- no unsupported surfaced memory

Aggregate output includes:
- mean score
- decision recall
- open-loop recall
- preference recall
- next-direction acceptance proxy
- false-memory proxy
- unsupported item count

The report also records the separated comparison dimensions:
- `memory_recall_quality`
- `next_direction_quality`
- `setup_friction`
- `cross_host_portability`
- `outcome_trace_learning`

Runtime probes may populate setup/portability/outcome-trace observations without claiming task-quality scores.

## External Result Import Contract

External systems can be compared in two ways:

1. Run their own adapter against the exported `comparison_tasks.tsv`.
2. Produce a JSON summary matching the aggregate fields below.

Expected JSON:

```json
{
  "system": "memd",
  "status": "run",
  "comparator_class": "memory_layer",
  "tasks": 9,
  "mean_score": 0.0,
  "decision_recall": 0.0,
  "open_loop_recall": 0.0,
  "preference_recall": 0.0,
  "next_direction_acceptance_proxy": 0.0,
  "false_memory_rate_proxy": 0.0,
  "unsupported_count": 0,
  "dimensions": {
    "memory_recall_quality": {
      "decision_recall": 0.0,
      "open_loop_recall": 0.0,
      "preference_recall": 0.0
    },
    "next_direction_quality": {
      "acceptance_proxy": 0.0,
      "unsupported_count": 0
    },
    "setup_friction": {
      "score": 0.0,
      "note": "What setup was required"
    },
    "cross_host_portability": {
      "score": 0.0,
      "note": "Whether the memory is host-neutral"
    },
    "outcome_trace_learning": {
      "score": 0.0,
      "note": "Whether accepted/ignored/modified outcomes are first-class and learnable"
    }
  },
  "notes": "How the external system was prompted and judged"
}
```

Import example:

```bash
./scripts/external_memory_comparison_harness.sh \
  --external-result memd=/tmp/memd-comparison.json \
  --external-result knowns=/tmp/knowns-comparison.json \
  --external-result hermes=/tmp/hermes-comparison.json
```

## Interpretation Rules

Aadesh has a real wedge only if:
- it beats baseline on recall and next-direction quality
- false-memory proxy remains low
- it shows value that memory-only tools do not capture, especially around intervention/outcome-aware guidance

If Aadesh only ties memd/Knowns/Hermes on memory recall, the correct response is not to build more platform. The likely response is to keep Aadesh as the supervisory trace and ranking layer, and consider using an external memory backend later.

## Non-Goals

This benchmark does not:
- install or vendor external tools
- call external cloud APIs
- add MCP adapters
- change Aadesh cognition behavior
- prove production readiness by itself

It gives a repeatable comparison contract so external systems can be added without changing the Aadesh core.
