#!/usr/bin/env bash
# upgrade_contract.sh — Upgrade a deployed TipJar contract to a new WASM hash.
#
# Usage:
#   ./scripts/upgrade_contract.sh <CONTRACT_ID> <NETWORK> <NEW_WASM_HASH>
#
# Environment variables (optional overrides):
#   ADMIN_ADDRESS  — Stellar address of the contract admin (defaults to `stellar keys address default`)
#   ADMIN_KEY      — Key name used for signing (defaults to "default")

set -euo pipefail

# ── Argument validation ────────────────────────────────────────────────────────
if [[ $# -lt 3 ]]; then
  echo "Usage: $0 <CONTRACT_ID> <NETWORK> <NEW_WASM_HASH>" >&2
  exit 1
fi

CONTRACT_ID="$1"
NETWORK="$2"
NEW_WASM_HASH="$3"

if [[ -z "$CONTRACT_ID" || -z "$NETWORK" || -z "$NEW_WASM_HASH" ]]; then
  echo "Error: CONTRACT_ID, NETWORK, and NEW_WASM_HASH must all be non-empty." >&2
  exit 1
fi

# ── Resolve admin ──────────────────────────────────────────────────────────────
ADMIN_KEY="${ADMIN_KEY:-default}"
ADMIN_ADDRESS="${ADMIN_ADDRESS:-$(stellar keys address "$ADMIN_KEY")}"

echo "Contract ID  : $CONTRACT_ID"
echo "Network      : $NETWORK"
echo "Admin address: $ADMIN_ADDRESS"
echo "New WASM hash: $NEW_WASM_HASH"
echo ""

# ── Record previous version for rollback reference ────────────────────────────
PREV_VERSION=$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  -- version 2>/dev/null || echo "unknown")
echo "Current version before upgrade: $PREV_VERSION"
echo "  (to roll back, re-run this script with the previous WASM hash)"
echo ""

# ── Invoke upgrade ─────────────────────────────────────────────────────────────
echo "Invoking upgrade..."
if stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$ADMIN_KEY" \
  --network "$NETWORK" \
  -- upgrade \
  --admin "$ADMIN_ADDRESS" \
  --new_wasm_hash "$NEW_WASM_HASH"; then

  NEW_VERSION=$(stellar contract invoke \
    --id "$CONTRACT_ID" \
    --network "$NETWORK" \
    -- version 2>/dev/null || echo "unknown")
  echo ""
  echo "Upgrade successful. New version: $NEW_VERSION"
else
  echo ""
  echo "Upgrade FAILED. The contract has not been modified." >&2
  exit 1
fi
