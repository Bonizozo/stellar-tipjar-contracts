#!/usr/bin/env bash
set -e

NETWORK="${1:?Usage: rollback.sh <network>}"
CONFIG="deployment/config.json"

echo "[rollback] Looking up previous deployment for $NETWORK..."

# Find the second-to-last history entry for this network.
PREV_ID=$(jq -r \
  --arg net "$NETWORK" \
  '[.history[] | select(.network == $net)] | .[-2].contract_id // empty' \
  "$CONFIG")

if [ -z "$PREV_ID" ]; then
  echo "[rollback] No previous deployment found for $NETWORK. Aborting."
  exit 1
fi

CURRENT_ID=$(jq -r --arg net "$NETWORK" '.networks[$net].active_contract_id' "$CONFIG")
echo "[rollback] Rolling back $NETWORK: $CURRENT_ID → $PREV_ID"

TMP=$(mktemp)
jq \
  --arg net "$NETWORK" \
  --arg id "$PREV_ID" \
  '.networks[$net].active_contract_id = $id' \
  "$CONFIG" > "$TMP" && mv "$TMP" "$CONFIG"

echo "[rollback] Verifying rolled-back contract..."
bash scripts/verify_deployment.sh "$PREV_ID" "$NETWORK"

echo "[rollback] Done. Active contract on $NETWORK is now $PREV_ID"
