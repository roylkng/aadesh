#!/usr/bin/env bash
set -euo pipefail

if ! command -v git >/dev/null 2>&1; then
  echo "git is required" >&2
  exit 1
fi

COMMAND="${1:-}"
if [[ -z "$COMMAND" ]]; then
  COMMAND="help"
else
  shift
fi

WATCH_TASK="${WATCH_TASK:-}"
WATCH_TASK_HINT="${WATCH_TASK_HINT:-session-watcher}"
WATCH_INTERVAL_SECONDS="${WATCH_INTERVAL_SECONDS:-180}"
WATCH_MAX_FILES="${WATCH_MAX_FILES:-25}"
WATCH_MAX_CYCLES="${WATCH_MAX_CYCLES:-0}"
WATCH_NOTE_FILE="${WATCH_NOTE_FILE:-}"
WATCH_STATE_DIR="${WATCH_STATE_DIR:-}"
DB_URL="${ADESH_DATABASE_URL:-}"
PID_FILE="${WATCH_PID_FILE:-}"
LOG_FILE="${WATCH_LOG_FILE:-}"
WATCHER_SCRIPT="${WATCHER_SCRIPT:-}"
WATCH_DAEMON_ROOT="${WATCH_DAEMON_ROOT:-${ADESH_DAEMON_ROOT:-}}"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Usage:
  session_learning_ctl.sh <start|stop|status> [options]

Commands:
  start    Start background watcher using session_learning_watcher.sh
  stop     Stop background watcher for the selected task hint
  status   Show watcher status for the selected task hint

Options:
  --task "<text>"            Task prompt (required for start unless WATCH_TASK is set)
  --task-hint "<hint>"       Watcher key (default: session-watcher)
  --interval <seconds>       Poll interval passed to watcher (default: 180)
  --max-files <n>            Max files passed to watcher (default: 25)
  --max-cycles <n>           Max cycles passed to watcher (default: 0)
  --note-file <path>         Optional note file passed to watcher
  --state-dir <path>         Optional watcher state dir
  --db-url <url>             Optional ADESH_DATABASE_URL override
  --pid-file <path>          Override pid file path
  --log-file <path>          Override log file path
  --watcher-script <path>    Override watcher script path
  --daemon-root <path>       Path to Aadesh repo root containing Cargo.toml
  -h, --help                 Show this help

Environment:
  WATCH_TASK, WATCH_TASK_HINT, WATCH_INTERVAL_SECONDS, WATCH_MAX_FILES,
  WATCH_MAX_CYCLES, WATCH_NOTE_FILE, WATCH_STATE_DIR, ADESH_DATABASE_URL,
  WATCH_PID_FILE, WATCH_LOG_FILE, WATCHER_SCRIPT, ADESH_DAEMON_ROOT
EOF
}

normalize_key() {
  printf '%s' "$1" \
    | tr '[:upper:]' '[:lower:]' \
    | sed -E 's/[^a-z0-9]+/_/g' \
    | sed -E 's/^_+|_+$//g'
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
    --db-url)
      DB_URL="$2"
      shift 2
      ;;
    --pid-file)
      PID_FILE="$2"
      shift 2
      ;;
    --log-file)
      LOG_FILE="$2"
      shift 2
      ;;
    --watcher-script)
      WATCHER_SCRIPT="$2"
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

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$repo_root" ]]; then
  echo "session learning control currently requires running inside a git workspace" >&2
  exit 1
fi

cd "$repo_root"

hint_key="$(normalize_key "$WATCH_TASK_HINT")"
if [[ -z "$hint_key" ]]; then
  hint_key="session_watcher"
fi

runtime_dir="${repo_root}/.aadesh/session-watcher-runtime"
mkdir -p "$runtime_dir"

if [[ -z "$PID_FILE" ]]; then
  PID_FILE="${runtime_dir}/${hint_key}.pid"
fi
if [[ -z "$LOG_FILE" ]]; then
  LOG_FILE="${runtime_dir}/${hint_key}.log"
fi
if [[ -z "$WATCHER_SCRIPT" ]]; then
  WATCHER_SCRIPT="${script_dir}/session_learning_watcher.sh"
fi
if [[ -z "$WATCH_DAEMON_ROOT" ]]; then
  WATCH_DAEMON_ROOT="$(cd "${script_dir}/.." && pwd)"
fi

if [[ ! -x "$WATCHER_SCRIPT" ]]; then
  echo "watcher script is not executable: $WATCHER_SCRIPT" >&2
  exit 1
fi
if [[ ! -f "${WATCH_DAEMON_ROOT}/Cargo.toml" ]]; then
  echo "daemon root does not contain Cargo.toml: ${WATCH_DAEMON_ROOT}" >&2
  exit 1
fi

current_pid() {
  if [[ ! -f "$PID_FILE" ]]; then
    return 1
  fi
  local pid
  pid="$(cat "$PID_FILE" 2>/dev/null || true)"
  if [[ ! "$pid" =~ ^[0-9]+$ ]]; then
    return 1
  fi
  printf '%s' "$pid"
}

is_running() {
  local pid
  pid="$(current_pid)" || return 1
  kill -0 "$pid" 2>/dev/null
}

start_watcher() {
  if [[ -z "$WATCH_TASK" ]]; then
    echo "missing --task for start (or WATCH_TASK env)" >&2
    exit 1
  fi

  if is_running; then
    local pid
    pid="$(current_pid)"
    echo "watcher already running"
    echo "pid=${pid}"
    echo "task_hint=${WATCH_TASK_HINT}"
    echo "pid_file=${PID_FILE}"
    echo "log_file=${LOG_FILE}"
    exit 0
  fi

  rm -f "$PID_FILE"

  watcher_cmd=(
    "$WATCHER_SCRIPT"
    --task "$WATCH_TASK"
    --task-hint "$WATCH_TASK_HINT"
    --interval "$WATCH_INTERVAL_SECONDS"
    --max-files "$WATCH_MAX_FILES"
    --max-cycles "$WATCH_MAX_CYCLES"
    --daemon-root "$WATCH_DAEMON_ROOT"
  )
  if [[ -n "$WATCH_NOTE_FILE" ]]; then
    watcher_cmd+=(--note-file "$WATCH_NOTE_FILE")
  fi
  if [[ -n "$WATCH_STATE_DIR" ]]; then
    watcher_cmd+=(--state-dir "$WATCH_STATE_DIR")
  fi

  if [[ -n "$DB_URL" ]]; then
    nohup env ADESH_DATABASE_URL="$DB_URL" "${watcher_cmd[@]}" >"$LOG_FILE" 2>&1 &
  else
    nohup "${watcher_cmd[@]}" >"$LOG_FILE" 2>&1 &
  fi
  local pid=$!
  echo "$pid" >"$PID_FILE"

  sleep 1
  if ! kill -0 "$pid" 2>/dev/null; then
    echo "watcher failed to stay running; check log: $LOG_FILE" >&2
    tail -n 20 "$LOG_FILE" 2>/dev/null || true
    rm -f "$PID_FILE"
    exit 1
  fi

  echo "watcher started"
  echo "pid=${pid}"
  echo "task_hint=${WATCH_TASK_HINT}"
  echo "pid_file=${PID_FILE}"
  echo "log_file=${LOG_FILE}"
}

stop_watcher() {
  local pid
  if ! pid="$(current_pid)"; then
    echo "watcher is not running (no pid file)"
    echo "task_hint=${WATCH_TASK_HINT}"
    echo "pid_file=${PID_FILE}"
    exit 0
  fi

  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    for _ in $(seq 1 30); do
      if ! kill -0 "$pid" 2>/dev/null; then
        break
      fi
      sleep 0.1
    done
  fi

  if kill -0 "$pid" 2>/dev/null; then
    echo "watcher did not stop cleanly (pid=${pid}); stop it manually" >&2
    exit 1
  fi

  rm -f "$PID_FILE"
  echo "watcher stopped"
  echo "task_hint=${WATCH_TASK_HINT}"
}

status_watcher() {
  if is_running; then
    local pid
    pid="$(current_pid)"
    echo "watcher is running"
    echo "pid=${pid}"
    echo "task_hint=${WATCH_TASK_HINT}"
    echo "pid_file=${PID_FILE}"
    echo "log_file=${LOG_FILE}"
    exit 0
  fi

  echo "watcher is not running"
  echo "task_hint=${WATCH_TASK_HINT}"
  echo "pid_file=${PID_FILE}"
  echo "log_file=${LOG_FILE}"
  if [[ -f "$PID_FILE" ]]; then
    echo "note=stale pid file detected"
  fi
}

case "$COMMAND" in
  start)
    start_watcher
    ;;
  stop)
    stop_watcher
    ;;
  status)
    status_watcher
    ;;
  help|-h|--help)
    usage
    ;;
  *)
    echo "unknown command: $COMMAND" >&2
    usage >&2
    exit 1
    ;;
esac
