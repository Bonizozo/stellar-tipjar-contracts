#!/usr/bin/env bash
set -euo pipefail

echo "Running integration tests..."
cargo test -p tipjar-integration-tests --test tip_flow_test -- --nocapture

echo "Done."
