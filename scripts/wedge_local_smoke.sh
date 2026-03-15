#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:7777}"
TOKEN="${ADESH_ROOT_OWNER_TOKEN:-dev-root-owner-token}"
SMOKE_APPROVE_SEND="${SMOKE_APPROVE_SEND:-0}"
SMOKE_SCENARIO="${SMOKE_SCENARIO:-send}"
SMOKE_MAX_POLLS="${SMOKE_MAX_POLLS:-120}"
SMOKE_POLL_INTERVAL_SECONDS="${SMOKE_POLL_INTERVAL_SECONDS:-1}"

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required" >&2
  exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required" >&2
  exit 1
fi

auth_header=("Authorization: Bearer ${TOKEN}")
json_header=("Content-Type: application/json")

case "${SMOKE_SCENARIO}" in
  draft)
    default_request_content="Draft a concise project update email with three action items."
    ;;
  send)
    default_request_content="Draft and send this email with a concise project update."
    ;;
  *)
    echo "unsupported SMOKE_SCENARIO=${SMOKE_SCENARIO}; expected draft or send" >&2
    exit 1
    ;;
esac

request_content="${SMOKE_REQUEST_CONTENT:-$default_request_content}"
request_content_json="$(printf '%s' "${request_content}" | jq -Rs .)"

req_id="req-smoke-$(date +%s%N)-$$"
idk_req="idem-${req_id}-request"

echo "[1] health check"
curl -sS "${BASE_URL}/v1/health" | jq . >/dev/null

echo "[2] submit request (scenario=${SMOKE_SCENARIO})"
create_resp="$(curl -sS -X POST "${BASE_URL}/v1/requests" \
  -H "${auth_header[0]}" \
  -H "${json_header[0]}" \
  -H "Idempotency-Key: ${idk_req}" \
  -d "{\"request_id\":\"${req_id}\",\"source\":{\"channel\":\"http\",\"transport\":\"rest\"},\"received_at\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"requesting_principal\":{\"principal_type\":\"root_owner\",\"principal_id\":\"owner-1\"},\"requesting_audience_id\":\"root_owner\",\"input\":{\"kind\":\"text\",\"content\":${request_content_json}},\"constraints\":{\"policy_mode\":\"default\",\"budgets\":{\"token_budget\":512}}}" \
)"

operation_id="$(echo "${create_resp}" | jq -r '.data.primary_operation_id')"
audit_trace_id="$(echo "${create_resp}" | jq -r '.data.audit_trace_ids[0]')"

if [[ "${operation_id}" == "null" || -z "${operation_id}" ]]; then
  echo "request did not return operation_id" >&2
  echo "${create_resp}" | jq . >&2
  exit 1
fi

echo "operation_id=${operation_id}"
echo "audit_trace_id=${audit_trace_id}"

echo "[3] wait for terminal or approval state"
operation_resp=""
operation_state=""
for _ in $(seq 1 "${SMOKE_MAX_POLLS}"); do
  operation_resp="$(curl -sS "${BASE_URL}/v1/operations/${operation_id}" -H "${auth_header[0]}")"
  operation_state="$(echo "${operation_resp}" | jq -r '.data.state')"
  case "${operation_state}" in
    completed|awaiting_approval|blocked|failed|cancelled)
      break
      ;;
    *)
      sleep "${SMOKE_POLL_INTERVAL_SECONDS}"
      ;;
  esac
done

echo "state=${operation_state}"

if [[ "${operation_state}" == "blocked" ]]; then
  reason="$(echo "${operation_resp}" | jq -r '.data.state_reason')"
  echo "operation blocked (fail-closed): ${reason}"
fi

if [[ "${operation_state}" == "failed" || "${operation_state}" == "cancelled" ]]; then
  echo "operation ended in non-success state: ${operation_state}" >&2
  echo "${operation_resp}" | jq . >&2
  exit 1
fi

if [[ "${operation_state}" == "awaiting_approval" ]]; then
  echo "[4] fetch pending approvals"
  approvals_resp="$(curl -sS "${BASE_URL}/v1/approvals/pending" -H "${auth_header[0]}")"
  approval_id="$(echo "${approvals_resp}" | jq -r --arg op "${operation_id}" '.data[] | select(.operation_id == $op) | .approval_id' | head -n1)"

  if [[ -z "${approval_id}" ]]; then
    echo "no pending approval found for operation ${operation_id}" >&2
    echo "${approvals_resp}" | jq . >&2
    exit 1
  fi

  echo "approval_id=${approval_id}"

  if [[ "${SMOKE_APPROVE_SEND}" != "1" ]]; then
    echo "[5] stopping before side effect execution (set SMOKE_APPROVE_SEND=1 to continue)"
  else
    echo "[5] approve pending action"
    idk_approve="idem-${req_id}-approve"
    approve_resp="$(curl -sS -X POST "${BASE_URL}/v1/approvals/${approval_id}" \
      -H "${auth_header[0]}" \
      -H "${json_header[0]}" \
      -H "Idempotency-Key: ${idk_approve}" \
      -d '{"decision":"approve","modified_payload":null,"oob":null}')"

    echo "${approve_resp}" | jq . >/dev/null

    echo "[6] verify syscall execution state"
    syscalls_resp="$(curl -sS "${BASE_URL}/v1/operations/${operation_id}/syscalls" -H "${auth_header[0]}")"
    syscall_status="$(echo "${syscalls_resp}" | jq -r '.data[0].status // ""')"

    if [[ "${syscall_status}" != "executed" ]]; then
      echo "expected executed syscall, got: ${syscall_status}" >&2
      echo "${syscalls_resp}" | jq . >&2
      exit 1
    fi
  fi
fi

echo "[7] verify audit trace is present"
audit_resp="$(curl -sS "${BASE_URL}/v1/audit/${audit_trace_id}" -H "${auth_header[0]}")"
timeline_len="$(echo "${audit_resp}" | jq -r '.data.timeline | length')"

if [[ "${timeline_len}" == "0" ]]; then
  echo "audit trace timeline is empty" >&2
  exit 1
fi

echo "demo smoke completed successfully"
