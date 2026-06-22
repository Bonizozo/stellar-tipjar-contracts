#!/usr/bin/env bash
set -euo pipefail

NETWORK="${1:-testnet}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTPUT_DIR="$REPO_ROOT/packages/contract-client/src"
CONFIG_FILE="$REPO_ROOT/deployment/config.json"

CONTRACT_ID="${CONTRACT_ID:-}"
if [[ -z "$CONTRACT_ID" ]]; then
  if [[ -f "$CONFIG_FILE" ]]; then
    CONTRACT_ID="$(jq -r --arg net "$NETWORK" '.networks[$net].active_contract_id // empty' "$CONFIG_FILE")"
  fi
fi

if [[ -z "$CONTRACT_ID" ]]; then
  echo "ERROR: CONTRACT_ID is required. Set CONTRACT_ID env var or configure deployment/config.json." >&2
  exit 1
fi

echo "[bindings] Generating TypeScript bindings for contract $CONTRACT_ID on $NETWORK"

stellar contract bindings typescript \
  --network "$NETWORK" \
  --contract-id "$CONTRACT_ID" \
  --output-dir "$OUTPUT_DIR"

echo "[bindings] Bindings generated at $OUTPUT_DIR"
