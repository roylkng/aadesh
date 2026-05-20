#!/usr/bin/env bash
set -euo pipefail

if ! command -v sqlite3 >/dev/null 2>&1; then
  echo "sqlite3 is required" >&2
  exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required" >&2
  exit 1
fi

DB_PATH=""
LIMIT="${LIMIT:-10}"
TASK_HINT="${TASK_HINT:-}"

usage() {
  cat <<'EOF'
Usage:
  session_learning_recent.sh [options]

Options:
  --db-path <path>       SQLite DB file path
  --limit <n>            Number of episodes to print (default: 10)
  --task-hint <hint>     Filter by watcher task hint (maps to task:hint:<normalized>)
  -h, --help             Show help

If --db-path is omitted, the script attempts to derive it from ADESH_DATABASE_URL
when it is in sqlite:///... form.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --db-path)
      DB_PATH="$2"
      shift 2
      ;;
    --limit)
      LIMIT="$2"
      shift 2
      ;;
    --task-hint)
      TASK_HINT="$2"
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

if ! [[ "$LIMIT" =~ ^[0-9]+$ ]] || [[ "$LIMIT" -lt 1 ]]; then
  echo "limit must be a positive integer" >&2
  exit 1
fi

if [[ -z "$DB_PATH" ]]; then
  if [[ -n "${ADESH_DATABASE_URL:-}" ]]; then
    case "$ADESH_DATABASE_URL" in
      sqlite:///*)
        DB_PATH="${ADESH_DATABASE_URL#sqlite://}"
        DB_PATH="${DB_PATH%%\?*}"
        ;;
      *)
        echo "ADESH_DATABASE_URL is set but not in sqlite:///... form; pass --db-path explicitly" >&2
        exit 1
        ;;
    esac
  else
    echo "missing database path; pass --db-path or set ADESH_DATABASE_URL" >&2
    exit 1
  fi
fi

if [[ ! -f "$DB_PATH" ]]; then
  echo "database file not found: $DB_PATH" >&2
  exit 1
fi

task_scope_filter=""
if [[ -n "$TASK_HINT" ]]; then
  normalized_hint="$(printf '%s' "$TASK_HINT" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/_/g' | sed -E 's/^_+|_+$//g')"
  task_scope_filter="task:hint:${normalized_hint}"
fi

if [[ -n "$task_scope_filter" ]]; then
  where_clause="WHERE task_scope_key = '${task_scope_filter}'"
else
  where_clause=""
fi

sql="
SELECT
  ended_at,
  episode_id,
  task_scope_key,
  json_extract(workspace_json, '\$.branch') AS branch,
  json_array_length(files_touched_json) AS file_count,
  summary
FROM episodes
${where_clause}
ORDER BY ended_at DESC
LIMIT ${LIMIT};
"

rows_json="$(sqlite3 -json "$DB_PATH" "$sql")"

if [[ "$rows_json" == "[]" ]]; then
  echo "no matching episodes found"
  exit 0
fi

printf '%s' "$rows_json" | jq -r '
  .[] |
  "\(.ended_at) | \(.episode_id) | task_scope=\(.task_scope_key // "-") | branch=\(.branch // "-") | files=\(.file_count // 0)\n  summary: \(.summary)"
'
