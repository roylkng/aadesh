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

validate_index_root_alignment() {
  if [ ! -f index.md ]; then
    echo "index.md is missing."
    return 1
  fi

  local index_entries
  index_entries="$(grep -oE '`[^`]+\.md`' index.md | tr -d '`' | sort -u)"
  local root_docs
  root_docs="$(find . -maxdepth 1 -type f -name '*.md' -printf '%f\n' | sort)"

  local alignment_ok=0
  local doc

  while IFS= read -r doc; do
    [ -z "$doc" ] && continue
    case "$doc" in
      README.md|AGENTS.md|index.md) continue ;;
    esac
    if ! printf '%s\n' "$index_entries" | grep -qx "$doc"; then
      echo "Unindexed root markdown file: $doc"
      alignment_ok=1
    fi
  done <<< "$root_docs"

  while IFS= read -r doc; do
    [ -z "$doc" ] && continue
    case "$doc" in
      */*) continue ;;
      README.md|AGENTS.md|index.md) continue ;;
    esac
    if [ ! -f "$doc" ]; then
      echo "Indexed root markdown file missing from repository root: $doc"
      alignment_ok=1
    fi
  done <<< "$index_entries"

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

echo "[5/7] index and root alignment"
if ! validate_index_root_alignment; then
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

echo "[7/7] root/reference/archive summary"
find . -maxdepth 2 -type f -name '*.md' | sort

if [ "$found" -ne 0 ]; then
  echo "Spec drift detected."
  exit 1
fi
