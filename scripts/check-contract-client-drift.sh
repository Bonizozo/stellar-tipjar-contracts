#!/usr/bin/env bash
set -euo pipefail

# Compare checked-in generated bindings with a contract spec without changing
# the working tree. Prefer a local WASM in CI; CONTRACT_ID is for deployed
# interface checks when a network and deployment config are available.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PACKAGE_DIR="$REPO_ROOT/packages/contract-client"
WASM_PATH="${1:-${TIPJAR_WASM:-}}"
NETWORK="${NETWORK:-testnet}"
CONTRACT_ID="${CONTRACT_ID:-}"

if [[ -z "$WASM_PATH" && -z "$CONTRACT_ID" ]]; then
  echo "usage: $0 <tipjar.wasm> or CONTRACT_ID=<id> $0" >&2
  exit 2
fi

TMP_PARENT="$(mktemp -d)"
trap 'rm -rf "$TMP_PARENT"' EXIT
TMP_DIR="$TMP_PARENT/bindings"

ARGS=(contract bindings typescript --output-dir "$TMP_DIR" --overwrite)
if [[ -n "$WASM_PATH" ]]; then
  ARGS+=(--wasm "$WASM_PATH")
else
  ARGS+=(--contract-id "$CONTRACT_ID" --network "$NETWORK")
fi

stellar "${ARGS[@]}"

GENERATED="$TMP_DIR/src/index.ts"
CHECKED_IN="$PACKAGE_DIR/src/generated.ts"

# Network constants and deployment metadata are intentionally excluded: the
# contract WASM is the source of truth for the callable interface, while
# deployment IDs belong to the environment that produced the vendored file.
extract_client_interface() {
  awk '
    /^export interface Client \{/ { in_client = 1 }
    in_client { print }
    in_client && /^\}/ { exit }
  ' "$1"
}

CHECKED_IN_INTERFACE="$TMP_PARENT/checked-in-interface.ts"
GENERATED_INTERFACE="$TMP_PARENT/generated-interface.ts"
extract_client_interface "$CHECKED_IN" > "$CHECKED_IN_INTERFACE"
extract_client_interface "$GENERATED" > "$GENERATED_INTERFACE"

if [[ ! -s "$CHECKED_IN_INTERFACE" || ! -s "$GENERATED_INTERFACE" ]]; then
  echo "ERROR: generated binding output is missing an export interface Client declaration." >&2
  exit 1
fi

if ! diff -u "$CHECKED_IN_INTERFACE" "$GENERATED_INTERFACE"; then
  cat >&2 <<'EOF'

Contract client bindings are out of date. Regenerate them with:
  scripts/generate-bindings.sh
EOF
  exit 1
fi

echo "Contract client bindings match the supplied contract interface."
