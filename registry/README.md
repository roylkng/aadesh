# Registry Bootstrap Artifacts

This directory contains bootstrap artifacts for:

- capability snapshots
- schema registry entries

These files are implementation inputs for initializing a fresh deployment and must remain aligned with:

- `schema_registry_and_versioning.md`
- `capability_mcp.md`
- `schema_based_tools_and_actions.md`

## Layout

- `bootstrap/schema_registry/`: schema payload files used to register immutable schema entries
- `bootstrap/capability_snapshots/`: capability snapshot payloads that pin tool/action schema refs

## Rules

- Treat files here as configuration/data artifacts, not behavior specs.
- Changes must preserve deterministic hashing/canonicalization assumptions in canonical specs.
- If a bootstrap artifact introduces a new action or schema, update relevant canonical docs and tests in the same PR.
