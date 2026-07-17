#!/usr/bin/env bash
# start-simulator.sh — build and run the TipJar local simulator.
set -euo pipefail

TIPS=${TIPS:-5}
AMOUNT=${AMOUNT:-100}
VERBOSE=${VERBOSE:-}
SAVE=${SAVE:-}
STATE=${STATE:-sim-state.json}

VERBOSE_FLAG=""
[[ -n "${VERBOSE}" ]] && VERBOSE_FLAG="--verbose"

SAVE_FLAG=""
[[ -n "${SAVE}" ]] && SAVE_FLAG="--save"

echo "==> Building simulator..."
cargo build -p tipjar-simulator --release 2>&1

echo "==> Starting simulation (tips=${TIPS}, amount=${AMOUNT})..."
cargo run -p tipjar-simulator --release -- \
  ${VERBOSE_FLAG} \
  --state "${STATE}" \
  run \
  --tips "${TIPS}" \
  --amount "${AMOUNT}" \
  ${SAVE_FLAG}
