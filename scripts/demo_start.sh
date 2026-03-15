#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

if [[ -f .env ]]; then
  # shellcheck disable=SC1091
  set -a
  source .env
  set +a
fi

export ADESH_BIND_ADDR="${ADESH_BIND_ADDR:-127.0.0.1:7777}"
export ADESH_ROOT_OWNER_TOKEN="${ADESH_ROOT_OWNER_TOKEN:-demo-root-owner-token}"
export ADESH_DATABASE_URL="${ADESH_DATABASE_URL:-sqlite://adesh.db?mode=rwc}"

export ADESH_MODEL_PROVIDER_BACKEND="${ADESH_MODEL_PROVIDER_BACKEND:-fake}"
export ADESH_MODEL_PROVIDER_BASE_URL="${ADESH_MODEL_PROVIDER_BASE_URL:-http://127.0.0.1:1234}"
export ADESH_MODEL_PROVIDER_MODEL="${ADESH_MODEL_PROVIDER_MODEL:-qwen3.5-27b}"
export ADESH_MODEL_PROVIDER_TIMEOUT_SECONDS="${ADESH_MODEL_PROVIDER_TIMEOUT_SECONDS:-180}"

export ADESH_EMAIL_PROVIDER_BACKEND="${ADESH_EMAIL_PROVIDER_BACKEND:-fake}"
export ADESH_WEBHOOK_PROVIDER_BACKEND="${ADESH_WEBHOOK_PROVIDER_BACKEND:-fake}"

echo "Starting Adesh OS demo daemon"
echo "  bind:   ${ADESH_BIND_ADDR}"
echo "  model:  ${ADESH_MODEL_PROVIDER_BACKEND}:${ADESH_MODEL_PROVIDER_MODEL}"
echo "  email:  ${ADESH_EMAIL_PROVIDER_BACKEND}"
echo "  token:  ${ADESH_ROOT_OWNER_TOKEN}"

echo ""
echo "Open UI: http://${ADESH_BIND_ADDR}"

cargo run -p adesh-daemon
