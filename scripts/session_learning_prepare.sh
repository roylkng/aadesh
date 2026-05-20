#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required" >&2
  exit 1
fi
if ! command -v git >/dev/null 2>&1; then
  echo "git is required" >&2
  exit 1
fi

TASK=""
TASK_FILE=""
TASK_HINT=""
AUTO_FILES=0
AUTO_MAX_FILES=10
PREPARE_DAEMON_ROOT="${PREPARE_DAEMON_ROOT:-${ADESH_DAEMON_ROOT:-}}"

declare -a FILE_ARGS=()
declare -a PASSTHROUGH_ARGS=()

usage() {
  cat <<'EOF'
Usage:
  session_learning_prepare.sh --task "<task prompt>" [options]

Required:
  --task "<text>"            Task prompt
    or --task-file <path>

Optional:
  --task-hint "<hint>"       Workstream/task hint
  --file "<path>"            File in focus (repeatable)
  --auto-files               Auto-add changed git files when --file not given
  --no-auto-files            Disable auto file detection (default)
  --auto-max-files <n>       Max auto-detected files (default: 10)
  --daemon-root <path>       Path to Aadesh repo root containing Cargo.toml
  -h, --help                 Show help

All additional supported `host prepare` flags are passed through, including:
  --workspace-kind --workspace-locator --cwd --branch --external-ref
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --task)
      TASK="$2"
      shift 2
      ;;
    --task-file)
      TASK_FILE="$2"
      shift 2
      ;;
    --task-hint)
      TASK_HINT="$2"
      shift 2
      ;;
    --file)
      FILE_ARGS+=("--file" "$2")
      shift 2
      ;;
    --auto-files)
      AUTO_FILES=1
      shift
      ;;
    --no-auto-files)
      AUTO_FILES=0
      shift
      ;;
    --auto-max-files)
      AUTO_MAX_FILES="$2"
      shift 2
      ;;
    --daemon-root)
      PREPARE_DAEMON_ROOT="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      PASSTHROUGH_ARGS+=("$1")
      shift
      if [[ "$#" -gt 0 && ! "$1" =~ ^-- ]]; then
        PASSTHROUGH_ARGS+=("$1")
        shift
      fi
      ;;
  esac
done

if [[ -n "$TASK" && -n "$TASK_FILE" ]]; then
  echo "provide only one of --task or --task-file" >&2
  exit 1
fi
if [[ -z "$TASK" && -n "$TASK_FILE" ]]; then
  TASK="$(cat "$TASK_FILE")"
fi
if [[ -z "$TASK" ]]; then
  echo "missing --task (or --task-file)" >&2
  usage >&2
  exit 1
fi
if ! [[ "$AUTO_MAX_FILES" =~ ^[0-9]+$ ]] || [[ "$AUTO_MAX_FILES" -lt 1 ]]; then
  echo "auto-max-files must be a positive integer" >&2
  exit 1
fi

if [[ -z "$PREPARE_DAEMON_ROOT" ]]; then
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  PREPARE_DAEMON_ROOT="$(cd "${script_dir}/.." && pwd)"
fi
if [[ ! -f "${PREPARE_DAEMON_ROOT}/Cargo.toml" ]]; then
  echo "daemon root does not contain Cargo.toml: ${PREPARE_DAEMON_ROOT}" >&2
  exit 1
fi

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -n "$repo_root" ]]; then
  cd "$repo_root"
else
  repo_root="$(pwd)"
fi

if [[ "${#FILE_ARGS[@]}" -eq 0 && "$AUTO_FILES" -eq 1 ]]; then
  mapfile -t auto_files < <(
    {
      git diff --name-only 2>/dev/null || true
      git diff --cached --name-only 2>/dev/null || true
      git ls-files --others --exclude-standard 2>/dev/null || true
    } | awk 'NF' | sort -u | head -n "$AUTO_MAX_FILES"
  )
  for path in "${auto_files[@]}"; do
    FILE_ARGS+=("--file" "$path")
  done
fi

cmd=(
  cargo run -q --manifest-path "${PREPARE_DAEMON_ROOT}/Cargo.toml" -p adesh-daemon -- host prepare
  --cwd "$repo_root"
  --task "$TASK"
)

if [[ -n "$TASK_HINT" ]]; then
  cmd+=(--task-hint "$TASK_HINT")
fi
cmd+=("${FILE_ARGS[@]}")
cmd+=("${PASSTHROUGH_ARGS[@]}")

"${cmd[@]}"
