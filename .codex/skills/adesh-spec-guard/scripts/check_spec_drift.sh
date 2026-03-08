#!/usr/bin/env bash
set -euo pipefail

repo_root="${1:-.}"
cd "$repo_root"

echo "[1/5] stale filename scan"
grep -RInE \
  'Audience_graph_and_disclosure_policy\.md|JIT_compiler\.md|control_plane-apispec\.md|governanace_kernal_logic\.md|replay_and_deterministic_re-exection\.md|threat_mode\.spec\.md|modelprovider_port_contract\.md|toolprovider_port_contract\.md|Provider_Interfaces\.md|contracts\.md|rust_contracts\.md|Problem\.md|task\.md|code_skeleton\.md|Api_spec\.md' \
  . --include='*.md' --exclude-dir='.codex' || true

echo "[2/5] stale approval endpoint scan"
grep -RInE '/v1/approvals/\{operation_id\}|approvals/\{operation_id\}' . --include='*.md' --exclude-dir='.codex' || true

echo "[3/5] stale pinned-state field scan"
grep -RIn 'pinned_state_version' . --include='*.md' --exclude='README.md' --exclude-dir='.codex' || true

echo "[4/5] prompt/wrapper artifact scan"
grep -RInE '^```md|^````md|Goal understood:' . --include='*.md' --exclude-dir='.codex' || true

echo "[5/5] root/reference/archive summary"
find . -maxdepth 2 -type f -name '*.md' | sort
