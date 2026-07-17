#!/usr/bin/env bash
set -euo pipefail

: "${DATABASE_URL:?DATABASE_URL is required}"
: "${CONTRACT_ID:?CONTRACT_ID is required}"
: "${HORIZON_URL:=https://horizon-testnet.stellar.org}"

echo "Running migrations..."
sqlx migrate run --source indexer/migrations

echo "Starting indexer for contract $CONTRACT_ID..."
cargo run -p tipjar-indexer
