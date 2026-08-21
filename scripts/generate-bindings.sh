#!/usr/bin/env bash
set -euo pipefail

# Generates TypeScript client bindings for the `tipjar` contract and vendors
# them into packages/contract-client/src/generated.ts.
#
# Usage:
#   scripts/generate-bindings.sh [network]
#   TIPJAR_WASM=path/to/tipjar.wasm scripts/generate-bindings.sh
#
# [network] defaults to "testnet" and must have an entry in
# deployment/config.json (written by scripts/deploy.sh), unless CONTRACT_ID
# is set explicitly.

NETWORK="${1:-testnet}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG_FILE="$REPO_ROOT/deployment/config.json"
PACKAGE_DIR="$REPO_ROOT/packages/contract-client"

log() { echo "[bindings] $*"; }

CONTRACT_ID="${CONTRACT_ID:-}"
WASM_PATH="${TIPJAR_WASM:-}"
if [[ -z "$WASM_PATH" && -z "$CONTRACT_ID" ]]; then
  if [[ ! -f "$CONFIG_FILE" ]]; then
    echo "[bindings] ERROR: $CONFIG_FILE not found. Run scripts/deploy.sh first, or set CONTRACT_ID." >&2
    exit 1
  fi
  CONTRACT_ID="$(jq -r --arg net "$NETWORK" '.networks[$net].active_contract_id // empty' "$CONFIG_FILE")"
fi

if [[ -z "$WASM_PATH" && -z "$CONTRACT_ID" ]]; then
  echo "[bindings] ERROR: no contract ID found for network '$NETWORK' in $CONFIG_FILE." >&2
  echo "[bindings]        Deploy first with scripts/deploy.sh, or set CONTRACT_ID explicitly." >&2
  exit 1
fi

log "Network:     $NETWORK"
if [[ -n "$WASM_PATH" ]]; then
  log "WASM:        $WASM_PATH"
else
  log "Contract ID: $CONTRACT_ID"
fi

TMP_PARENT="$(mktemp -d)"
trap 'rm -rf "$TMP_PARENT"' EXIT
# --output-dir's basename must be a valid (lowercase) npm package name.
TMP_DIR="$TMP_PARENT/bindings"

log "Running stellar contract bindings typescript..."
ARGS=(contract bindings typescript --output-dir "$TMP_DIR" --overwrite)
if [[ -n "$WASM_PATH" ]]; then
  ARGS+=(--wasm "$WASM_PATH")
else
  ARGS+=(--contract-id "$CONTRACT_ID" --network "$NETWORK")
fi
stellar "${ARGS[@]}"

mkdir -p "$PACKAGE_DIR/src"
cp "$TMP_DIR/src/index.ts" "$PACKAGE_DIR/src/generated.ts"

log "Wrote $PACKAGE_DIR/src/generated.ts"
log "Run 'npm install && npm run build' in $PACKAGE_DIR to compile the package."
