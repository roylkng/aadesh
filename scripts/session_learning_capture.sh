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
SUMMARY=""
SUMMARY_FILE=""
TASK_HINT=""
AUTO_FILES=1
AUTO_MAX_FILES=20
CAPTURE_DAEMON_ROOT="${CAPTURE_DAEMON_ROOT:-${ADESH_DAEMON_ROOT:-}}"

declare -a PASSTHROUGH_ARGS=()
declare -a FILE_ARGS=()

usage() {
  cat <<'EOF'
Usage:
  session_learning_capture.sh --task "<task>" --summary "<what happened>" [options]

Required:
  --task "<text>"            Task prompt
  --summary "<text>"         Work summary
    or --summary-file <path>

Optional:
  --task-hint "<hint>"       Workstream/task hint
  --file "<path>"            File touched (repeatable)
  --auto-files               Auto-add changed git files when --file not given (default)
  --no-auto-files            Disable auto file detection
  --auto-max-files <n>       Max auto-detected files (default: 20)
  --daemon-root <path>       Path to Aadesh repo root containing Cargo.toml
  --dry-run                  Print generated host store command only
  -h, --help                 Show help

All additional supported `host store` flags are passed through, including:
  --workspace-kind --workspace-locator --cwd --branch --external-ref
  --decision --unresolved --preference --risk --issue --artifact --test

Examples:
  session_learning_capture.sh \
    --task "Harden retry path" \
    --summary "Separated dedupe boundary; timeout tests still pending" \
    --task-hint retry-hardening \
    --decision "Keep dedupe in service boundary::Avoid transport-layer coupling" \
    --unresolved "Timeout coverage still missing" \
    --test "fail::retry_timeout::Timeout path fails under packet loss"
EOF
}

DRY_RUN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --task)
      TASK="$2"
      shift 2
      ;;
    --summary)
      SUMMARY="$2"
      shift 2
      ;;
    --summary-file)
      SUMMARY_FILE="$2"
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
      CAPTURE_DAEMON_ROOT="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
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

if [[ -z "$TASK" ]]; then
  echo "missing --task" >&2
  usage >&2
  exit 1
fi
if [[ -n "$SUMMARY" && -n "$SUMMARY_FILE" ]]; then
  echo "provide only one of --summary or --summary-file" >&2
  exit 1
fi
if [[ -z "$SUMMARY" && -n "$SUMMARY_FILE" ]]; then
  SUMMARY="$(cat "$SUMMARY_FILE")"
fi
if [[ -z "$SUMMARY" ]]; then
  echo "missing --summary (or --summary-file)" >&2
  usage >&2
  exit 1
fi
if ! [[ "$AUTO_MAX_FILES" =~ ^[0-9]+$ ]] || [[ "$AUTO_MAX_FILES" -lt 1 ]]; then
  echo "auto-max-files must be a positive integer" >&2
  exit 1
fi

if [[ -z "$CAPTURE_DAEMON_ROOT" ]]; then
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  CAPTURE_DAEMON_ROOT="$(cd "${script_dir}/.." && pwd)"
fi
if [[ ! -f "${CAPTURE_DAEMON_ROOT}/Cargo.toml" ]]; then
  echo "daemon root does not contain Cargo.toml: ${CAPTURE_DAEMON_ROOT}" >&2
  exit 1
fi

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -n "$repo_root" ]]; then
  cd "$repo_root"
fi

if [[ "${#FILE_ARGS[@]}" -eq 0 && "$AUTO_FILES" -eq 1 && -n "$repo_root" ]]; then
  mapfile -t auto_files < <(
    {
      git diff --name-only
      git diff --cached --name-only
      git ls-files --others --exclude-standard
    } | awk 'NF' | sort -u | head -n "$AUTO_MAX_FILES"
  )
  for path in "${auto_files[@]}"; do
    FILE_ARGS+=("--file" "$path")
  done
fi

cmd=(cargo run -q --manifest-path "${CAPTURE_DAEMON_ROOT}/Cargo.toml" -p adesh-daemon -- host store --task "$TASK" --summary "$SUMMARY")
if [[ -n "$TASK_HINT" ]]; then
  cmd+=(--task-hint "$TASK_HINT")
fi
cmd+=("${FILE_ARGS[@]}")
cmd+=("${PASSTHROUGH_ARGS[@]}")

if [[ "$DRY_RUN" -eq 1 ]]; then
  printf 'Generated command:\n'
  printf '%q ' "${cmd[@]}"
  printf '\n'
  exit 0
fi

"${cmd[@]}"
