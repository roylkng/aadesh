#!/usr/bin/env bash
set -euo pipefail

repo_root="${1:-.}"
cd "$repo_root"

found=0

run_forbidden_scan() {
  local label="$1"
  shift
  echo "$label"
  if "$@"; then
    found=1
  fi
}

validate_root_doc_boundary() {
  if [ ! -f index.md ]; then
    echo "index.md is missing."
    return 1
  fi

  local root_docs
  root_docs="$(find . -maxdepth 1 -type f -name '*.md' -printf '%f\n' | sort)"

  local alignment_ok=0
  local doc

  while IFS= read -r doc; do
    [ -z "$doc" ] && continue
    case "$doc" in
      README.md|AGENTS.md|index.md) continue ;;
    esac
    echo "Root markdown must be entry-only; move this file under docs/: $doc"
    alignment_ok=1
  done <<< "$root_docs"

  return "$alignment_ok"
}

run_forbidden_scan "[1/5] stale filename scan" \
grep -RInE \
  'Audience_graph_and_disclosure_policy\.md|JIT_compiler\.md|control_plane-apispec\.md|governanace_kernal_logic\.md|replay_and_deterministic_re-exection\.md|threat_mode\.spec\.md|modelprovider_port_contract\.md|toolprovider_port_contract\.md|Provider_Interfaces\.md|contracts\.md|rust_contracts\.md|Problem\.md|task\.md|code_skeleton\.md|Api_spec\.md|sandboxed_actuator_capabiltiy\.md' \
  . --include='*.md' --exclude-dir='.codex'

run_forbidden_scan "[2/5] stale approval endpoint scan" \
grep -RInE '/v1/approvals/\{operation_id\}|approvals/\{operation_id\}' . --include='*.md' --exclude-dir='.codex'

run_forbidden_scan "[3/5] stale pinned-state field scan" \
grep -RIn 'pinned_state_version' . --include='*.md' --exclude='README.md' --exclude-dir='.codex'

run_forbidden_scan "[4/5] prompt/wrapper artifact scan" \
grep -RInE '^```md|^````md|Goal understood:' . --include='*.md' --exclude-dir='.codex'

echo "[5/7] index and root/spec alignment"
if ! validate_root_doc_boundary; then
  found=1
fi

echo "[6/7] structure and hygiene checks"
if [ ! -f docs/REPO_ORGANIZATION.md ]; then
  echo "Missing docs/REPO_ORGANIZATION.md"
  found=1
fi
if [ ! -f docs/README.md ]; then
  echo "Missing docs/README.md"
  found=1
fi
if [ ! -f docs/DOCS_MAP.md ]; then
  echo "Missing docs/DOCS_MAP.md"
  found=1
fi
if [ ! -f docs/CODEBASE_MAP.md ]; then
  echo "Missing docs/CODEBASE_MAP.md"
  found=1
fi
if [ ! -f docs/specs/README.md ]; then
  echo "Missing docs/specs/README.md"
  found=1
fi
if [ ! -d docs/specs/active ]; then
  echo "Missing docs/specs/active"
  found=1
fi
if [ ! -d docs/specs/deferred ]; then
  echo "Missing docs/specs/deferred"
  found=1
fi
if find docs/specs -maxdepth 1 -type f -name '*.md' ! -name 'README.md' | grep -q '.'; then
  echo "Loose spec markdown found directly under docs/specs; use active/ or deferred/:"
  find docs/specs -maxdepth 1 -type f -name '*.md' ! -name 'README.md'
  found=1
fi
if [ ! -f registry/README.md ]; then
  echo "Missing registry/README.md"
  found=1
fi
if [ ! -f crates/README.md ]; then
  echo "Missing crates/README.md"
  found=1
fi
if [ ! -f .gitignore ] || ! grep -q '^/target/$' .gitignore; then
  echo ".gitignore missing required /target/ ignore rule"
  found=1
fi
if find . -maxdepth 1 -type f \
  \( -name 'control_plane-apispec.md' \
  -o -name 'governanace_kernal_logic.md' \
  -o -name 'replay_and_deterministic_re-exection.md' \
  -o -name 'threat_mode.spec.md' \
  -o -name 'sandboxed_actuator_capabiltiy.md' \) | grep -q '.'; then
  echo "Legacy typo-named canonical files exist in repository root."
  find . -maxdepth 1 -type f \
    \( -name 'control_plane-apispec.md' \
    -o -name 'governanace_kernal_logic.md' \
    -o -name 'replay_and_deterministic_re-exection.md' \
    -o -name 'threat_mode.spec.md' \
    -o -name 'sandboxed_actuator_capabiltiy.md' \)
  found=1
fi

echo "[7/7] root/spec/reference/archive summary"
find . -maxdepth 4 -type f -name '*.md' | sort

if [ "$found" -ne 0 ]; then
  echo "Spec drift detected."
  exit 1
fi
