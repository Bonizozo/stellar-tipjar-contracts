#!/usr/bin/env bash
# upgrade-contract.sh — build, upload, and upgrade the TipJar contract.
#
# Usage:
#   ADMIN_KEY=<key-name>  CONTRACT_ID=<id>  bash scripts/upgrade-contract.sh
#
# Optional env vars:
#   NETWORK        testnet (default) | mainnet | futurenet
#   RPC_URL        override Stellar RPC endpoint
#   NETWORK_PASSPHRASE  override network passphrase
set -euo pipefail

NETWORK="${NETWORK:-testnet}"
ADMIN_KEY="${ADMIN_KEY:?ADMIN_KEY is required}"
CONTRACT_ID="${CONTRACT_ID:?CONTRACT_ID is required}"

case "$NETWORK" in
  mainnet)
    RPC_URL="${RPC_URL:-https://horizon.stellar.org}"
    NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Public Global Stellar Network ; September 2015}"
    ;;
  futurenet)
    RPC_URL="${RPC_URL:-https://rpc-futurenet.stellar.org}"
    NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Test SDF Future Network ; October 2022}"
    ;;
  *)
    RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
    NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
    ;;
esac

WASM="target/wasm32v1-none/release/tipjar.wasm"

echo "==> Building contract (release)..."
cargo build -p tipjar --target wasm32v1-none --release

echo "==> Querying current version..."
CURRENT_VERSION=$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE" \
  -- get_version 2>/dev/null || echo "0")
echo "    Current version: $CURRENT_VERSION"

echo "==> Uploading new WASM to $NETWORK..."
NEW_HASH=$(stellar contract upload \
  --wasm "$WASM" \
  --source "$ADMIN_KEY" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE")
echo "    New WASM hash: $NEW_HASH"

echo "==> Invoking upgrade (admin: $ADMIN_KEY)..."
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$ADMIN_KEY" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE" \
  -- upgrade \
  --new_wasm_hash "$NEW_HASH"

echo "==> Verifying new version..."
NEW_VERSION=$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE" \
  -- get_version)
echo "    New version: $NEW_VERSION"

echo "==> Upgrade complete: v${CURRENT_VERSION} -> v${NEW_VERSION}"
echo "    Contract : $CONTRACT_ID"
echo "    WASM hash: $NEW_HASH"
echo ""
echo "To roll back, re-upload the previous WASM and run:"
echo "  NEW_HASH=<previous-hash> bash scripts/upgrade-contract.sh"
