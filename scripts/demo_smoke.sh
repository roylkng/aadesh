#!/usr/bin/env bash
set -euo pipefail

# Generic wrapper for local demo verification.
# Defaults to a non-side-effect draft path.
export SMOKE_SCENARIO="${SMOKE_SCENARIO:-draft}"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "${script_dir}/wedge_local_smoke.sh"
