#!/usr/bin/env bash
set -euo pipefail

# Builds, optimizes, and deploys the `tipjar` contract to a Stellar network
# (testnet by default), then records the resulting contract ID in
# deployment/config.json.
#
# Usage:
#   scripts/deploy.sh [token_address]
#
# Overridable env vars:
#   NETWORK_NAME        Key under deployment/config.json to write to (default: testnet)
#   RPC_URL              Soroban RPC endpoint (default: testnet RPC)
#   NETWORK_PASSPHRASE   Network passphrase (default: testnet passphrase)
#   DEPLOYER_IDENTITY    Name of the `stellar keys` identity used to deploy (default: tipjar-deployer)
#   TOKEN_ADDRESS        SEP-41 token contract address to pass to `init` (or pass as $1)

NETWORK_NAME="${NETWORK_NAME:-testnet}"
RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
DEPLOYER_IDENTITY="${DEPLOYER_IDENTITY:-tipjar-deployer}"
TOKEN_ADDRESS="${TOKEN_ADDRESS:-${1:-}}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG_FILE="$REPO_ROOT/deployment/config.json"

log() { echo "[deploy] $*"; }

log "Network:    $NETWORK_NAME"
log "RPC URL:    $RPC_URL"
log "Passphrase: $NETWORK_PASSPHRASE"

log "Step 1/6: Building tipjar for wasm32v1-none (release)..."
cargo build -p tipjar --target wasm32v1-none --release --manifest-path "$REPO_ROOT/Cargo.toml"

WASM_PATH="$REPO_ROOT/target/wasm32v1-none/release/tipjar.wasm"
if [[ ! -f "$WASM_PATH" ]]; then
  echo "[deploy] ERROR: expected wasm artifact not found at $WASM_PATH" >&2
  exit 1
fi

log "Step 2/6: Optimizing wasm..."
if stellar contract optimize --help >/dev/null 2>&1; then
  stellar contract optimize --wasm "$WASM_PATH"
  OPTIMIZED_PATH="${WASM_PATH%.wasm}.optimized.wasm"
  if [[ -f "$OPTIMIZED_PATH" ]]; then
    WASM_PATH="$OPTIMIZED_PATH"
    log "Using optimized artifact: $WASM_PATH"
  fi
else
  log "stellar contract optimize is not available in this CLI version, skipping"
fi

log "Step 3/6: Ensuring deployer identity '$DEPLOYER_IDENTITY' exists..."
if stellar keys address "$DEPLOYER_IDENTITY" >/dev/null 2>&1; then
  log "Identity '$DEPLOYER_IDENTITY' already exists"
else
  stellar keys generate "$DEPLOYER_IDENTITY"
fi
DEPLOYER_ADDRESS="$(stellar keys address "$DEPLOYER_IDENTITY")"
log "Deployer address: $DEPLOYER_ADDRESS"

log "Step 4/6: Funding deployer via friendbot (idempotent)..."
stellar keys fund "$DEPLOYER_IDENTITY" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE" \
  || log "Funding request failed or account already funded — continuing"

log "Step 5/6: Deploying contract..."
CONTRACT_ID="$(stellar contract deploy \
  --wasm "$WASM_PATH" \
  --source-account "$DEPLOYER_IDENTITY" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE")"
log "Deployed contract ID: $CONTRACT_ID"

log "Step 6/6: Recording contract ID in $CONFIG_FILE..."
mkdir -p "$(dirname "$CONFIG_FILE")"
if [[ ! -f "$CONFIG_FILE" ]]; then
  echo '{"networks":{}}' > "$CONFIG_FILE"
fi

TMP_FILE="$(mktemp)"
jq \
  --arg net "$NETWORK_NAME" \
  --arg id "$CONTRACT_ID" \
  --arg rpc "$RPC_URL" \
  --arg pass "$NETWORK_PASSPHRASE" \
  '.networks[$net] = {"rpc_url": $rpc, "network_passphrase": $pass, "active_contract_id": $id}' \
  "$CONFIG_FILE" > "$TMP_FILE"
mv "$TMP_FILE" "$CONFIG_FILE"
log "Wrote networks.$NETWORK_NAME.active_contract_id (other networks left untouched)"

if [[ -n "$TOKEN_ADDRESS" ]]; then
  log "Initializing contract with token $TOKEN_ADDRESS..."
  stellar contract invoke \
    --id "$CONTRACT_ID" \
    --source-account "$DEPLOYER_IDENTITY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    -- init --token "$TOKEN_ADDRESS"
  log "Contract initialized with token $TOKEN_ADDRESS"
else
  log "No token address supplied (set TOKEN_ADDRESS env var or pass as \$1). Initialize manually with:"
  cat <<EOF

  stellar contract invoke \\
    --id $CONTRACT_ID \\
    --source-account $DEPLOYER_IDENTITY \\
    --rpc-url $RPC_URL \\
    --network-passphrase "$NETWORK_PASSPHRASE" \\
    -- init --token <TOKEN_CONTRACT_ADDRESS>

EOF
fi

log "Done. Contract ID: $CONTRACT_ID"
