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
if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required" >&2
  exit 1
fi

WATCH_TASK="${WATCH_TASK:-}"
WATCH_TASK_HINT="${WATCH_TASK_HINT:-session-watcher}"
WATCH_INTERVAL_SECONDS="${WATCH_INTERVAL_SECONDS:-180}"
WATCH_MAX_FILES="${WATCH_MAX_FILES:-25}"
WATCH_MAX_CYCLES="${WATCH_MAX_CYCLES:-0}"
WATCH_NOTE_FILE="${WATCH_NOTE_FILE:-}"
WATCH_STATE_DIR="${WATCH_STATE_DIR:-}"
WATCH_DAEMON_ROOT="${WATCH_DAEMON_ROOT:-${ADESH_DAEMON_ROOT:-}}"

usage() {
  cat <<'EOF'
Usage:
  session_learning_watcher.sh --task "<task prompt>" [options]

Options:
  --task "<text>"            Task prompt for auto-captured episodes (required unless WATCH_TASK is set)
  --task-hint "<hint>"       Optional task hint (default: session-watcher)
  --interval <seconds>       Poll interval seconds (default: 180)
  --max-files <n>            Max changed files to include per episode (default: 25)
  --max-cycles <n>           Stop after n cycles (0 means run forever; default: 0)
  --note-file <path>         Optional text file appended to summary each capture
  --state-dir <path>         State directory (default: <repo>/.aadesh/session-watcher)
  --daemon-root <path>       Path to Aadesh repo root containing Cargo.toml
  -h, --help                 Show this help

Environment:
  ADESH_DATABASE_URL         Optional DB override for host store calls
  ADESH_DAEMON_ROOT          Optional Aadesh repo root override
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --task)
      WATCH_TASK="$2"
      shift 2
      ;;
    --task-hint)
      WATCH_TASK_HINT="$2"
      shift 2
      ;;
    --interval)
      WATCH_INTERVAL_SECONDS="$2"
      shift 2
      ;;
    --max-files)
      WATCH_MAX_FILES="$2"
      shift 2
      ;;
    --max-cycles)
      WATCH_MAX_CYCLES="$2"
      shift 2
      ;;
    --note-file)
      WATCH_NOTE_FILE="$2"
      shift 2
      ;;
    --state-dir)
      WATCH_STATE_DIR="$2"
      shift 2
      ;;
    --daemon-root)
      WATCH_DAEMON_ROOT="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$WATCH_TASK" ]]; then
  echo "missing required --task (or WATCH_TASK env)" >&2
  usage >&2
  exit 1
fi

if ! [[ "$WATCH_INTERVAL_SECONDS" =~ ^[0-9]+$ ]] || [[ "$WATCH_INTERVAL_SECONDS" -lt 1 ]]; then
  echo "interval must be a positive integer" >&2
  exit 1
fi
if ! [[ "$WATCH_MAX_FILES" =~ ^[0-9]+$ ]] || [[ "$WATCH_MAX_FILES" -lt 1 ]]; then
  echo "max-files must be a positive integer" >&2
  exit 1
fi
if ! [[ "$WATCH_MAX_CYCLES" =~ ^[0-9]+$ ]]; then
  echo "max-cycles must be a non-negative integer" >&2
  exit 1
fi

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$repo_root" ]]; then
  echo "session watcher currently requires running inside a git workspace" >&2
  exit 1
fi

if [[ -z "$WATCH_DAEMON_ROOT" ]]; then
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  WATCH_DAEMON_ROOT="$(cd "${script_dir}/.." && pwd)"
fi
if [[ ! -f "${WATCH_DAEMON_ROOT}/Cargo.toml" ]]; then
  echo "daemon root does not contain Cargo.toml: ${WATCH_DAEMON_ROOT}" >&2
  exit 1
fi

cd "$repo_root"
if [[ -z "$WATCH_STATE_DIR" ]]; then
  WATCH_STATE_DIR="${repo_root}/.aadesh/session-watcher"
fi
mkdir -p "$WATCH_STATE_DIR"

state_id="$(printf '%s|%s' "$repo_root" "$WATCH_TASK_HINT" | sha256sum | awk '{print $1}')"
state_file="${WATCH_STATE_DIR}/${state_id}.json"
state_dir_rel=""
if [[ "$WATCH_STATE_DIR" == "$repo_root"* ]]; then
  state_dir_rel="${WATCH_STATE_DIR#${repo_root}/}"
fi

last_signature=""
if [[ -f "$state_file" ]]; then
  last_signature="$(jq -r '.last_signature // ""' "$state_file" 2>/dev/null || true)"
fi

echo "session watcher started"
echo "repo_root=${repo_root}"
echo "task_hint=${WATCH_TASK_HINT}"
echo "interval_seconds=${WATCH_INTERVAL_SECONDS}"
echo "daemon_root=${WATCH_DAEMON_ROOT}"
echo "state_file=${state_file}"

cycles=0
while true; do
  mapfile -t changed_files < <(
    {
      git diff --name-only
      git diff --cached --name-only
      git ls-files --others --exclude-standard
    } | awk 'NF' | sort -u | while IFS= read -r path; do
      if [[ -n "$state_dir_rel" && "$path" == "$state_dir_rel"* ]]; then
        continue
      fi
      printf '%s\n' "$path"
    done
  )

  file_count="${#changed_files[@]}"
  if [[ "$file_count" -eq 0 ]]; then
    :
  else
    branch="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)"
    head="$(git rev-parse --short HEAD 2>/dev/null || echo no-head)"
    signature="$(
      {
        printf 'branch=%s\n' "$branch"
        printf 'head=%s\n' "$head"
        printf 'files=%s\n' "$file_count"
        printf '%s\n' "${changed_files[@]}"
      } | sha256sum | awk '{print $1}'
    )"

    if [[ "$signature" != "$last_signature" ]]; then
      limited_files=("${changed_files[@]:0:$WATCH_MAX_FILES}")
      file_preview="$(printf '%s, ' "${limited_files[@]}")"
      file_preview="${file_preview%, }"
      summary="Auto-captured coding activity on branch ${branch} (head ${head}) with ${file_count} changed files: ${file_preview}."

      if [[ -n "$WATCH_NOTE_FILE" && -f "$WATCH_NOTE_FILE" ]]; then
        note_text="$(tr '\n' ' ' < "$WATCH_NOTE_FILE" | sed 's/[[:space:]]\+/ /g' | sed 's/^ //; s/ $//')"
        if [[ -n "$note_text" ]]; then
          summary="${summary} Note: ${note_text}"
        fi
      fi

      store_cmd=(cargo run -q --manifest-path "${WATCH_DAEMON_ROOT}/Cargo.toml" -p adesh-daemon -- host store --cwd "$repo_root" --task "$WATCH_TASK" --summary "$summary")
      if [[ -n "$WATCH_TASK_HINT" ]]; then
        store_cmd+=(--task-hint "$WATCH_TASK_HINT")
      fi
      for file in "${limited_files[@]}"; do
        store_cmd+=(--file "$file")
      done

      response="$("${store_cmd[@]}")"
      episode_id="$(printf '%s' "$response" | jq -r '.episode_id // empty')"
      timestamp="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
      jq -n \
        --arg repo_root "$repo_root" \
        --arg task_hint "$WATCH_TASK_HINT" \
        --arg last_signature "$signature" \
        --arg last_episode_id "$episode_id" \
        --arg last_stored_at "$timestamp" \
        '{
          repo_root: $repo_root,
          task_hint: $task_hint,
          last_signature: $last_signature,
          last_episode_id: $last_episode_id,
          last_stored_at: $last_stored_at
        }' > "$state_file"

      last_signature="$signature"
      echo "stored episode_id=${episode_id} changed_files=${file_count} at=${timestamp}"
    fi
  fi

  cycles=$((cycles + 1))
  if [[ "$WATCH_MAX_CYCLES" -gt 0 && "$cycles" -ge "$WATCH_MAX_CYCLES" ]]; then
    echo "session watcher completed max cycles (${WATCH_MAX_CYCLES})"
    break
  fi

  sleep "$WATCH_INTERVAL_SECONDS"
done
