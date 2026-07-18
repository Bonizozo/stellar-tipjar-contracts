#!/usr/bin/env bash
set -e

: "${DEPLOYER_SECRET:?DEPLOYER_SECRET env var is required}"

NETWORK="mainnet"
WASM="target/wasm32v1-none/release/tipjar.wasm"
WASM_OPT="target/wasm32v1-none/release/tipjar.optimized.wasm"
CONFIG="deployment/config.json"

# Safety check: require explicit confirmation unless CI flag is set.
if [ "${CI_MAINNET_CONFIRMED:-}" != "true" ]; then
  echo "WARNING: You are about to deploy to MAINNET."
  read -r -p "Type 'deploy mainnet' to confirm: " CONFIRM
  if [ "$CONFIRM" != "deploy mainnet" ]; then
    echo "[deploy] Aborted."
    exit 1
  fi
fi

echo "[deploy] Building contract..."
cargo build -p tipjar --target wasm32v1-none --release

echo "[deploy] Optimizing WASM..."
stellar contract optimize --wasm "$WASM"

echo "[deploy] Deploying to $NETWORK..."
CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM_OPT" \
  --source "$DEPLOYER_SECRET" \
  --network "$NETWORK")

echo "[deploy] Deployed contract ID: $CONTRACT_ID"

echo "[deploy] Verifying deployment..."
bash scripts/verify_deployment.sh "$CONTRACT_ID" "$NETWORK"

echo "[deploy] Recording deployment in $CONFIG..."
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
TMP=$(mktemp)
jq \
  --arg net "$NETWORK" \
  --arg id "$CONTRACT_ID" \
  --arg ts "$TIMESTAMP" \
  '.networks[$net].active_contract_id = $id |
   .history += [{"network": $net, "contract_id": $id, "timestamp": $ts}]' \
  "$CONFIG" > "$TMP" && mv "$TMP" "$CONFIG"

echo "[deploy] Done. Active contract on $NETWORK: $CONTRACT_ID"
