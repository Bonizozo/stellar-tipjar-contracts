#!/usr/bin/env bash
# profile-gas.sh — Run gas profiling and optionally compare against a baseline.
#
# Usage:
#   bash scripts/profile-gas.sh                        # profile only
#   bash scripts/profile-gas.sh --baseline gas-report.json  # profile + compare
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORT="$REPO_ROOT/gas-report.json"
BASELINE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --baseline) BASELINE="$2"; shift 2 ;;
    *) echo "Unknown argument: $1"; exit 1 ;;
  esac
done

echo "=== TipJar Gas Profiler ==="
echo ""

# 1. Run profiler (produces gas-report.json)
PROFILER_ARGS=(--output "$REPORT")
[[ -n "$BASELINE" ]] && PROFILER_ARGS+=(--baseline "$BASELINE")

cargo run --manifest-path "$REPO_ROOT/Cargo.toml" \
  -p tipjar-gas-tools --bin gas-profiler -- "${PROFILER_ARGS[@]}"

echo ""

# 2. Run analyzer (prints recommendations)
cargo run --manifest-path "$REPO_ROOT/Cargo.toml" \
  -p tipjar-gas-tools --bin gas-analyzer -- --report "$REPORT"
