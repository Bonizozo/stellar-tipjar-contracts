#!/usr/bin/env bash
# run-tests.sh — unified test runner for local development and CI.
set -euo pipefail

THREADS=${TEST_THREADS:-4}
COVERAGE=${COVERAGE:-0}

echo "==> Building contract (WASM)..."
cargo build -p tipjar --target wasm32v1-none --release

echo "==> Running unit & integration tests (threads=${THREADS})..."
cargo test -p tipjar -- --test-threads="${THREADS}"

echo "==> Running quickcheck property tests..."
cargo test -p tipjar --test quickcheck_properties

echo "==> Running integration test suite..."
cargo test -p tipjar-integration-tests

echo "==> Running gas benchmarks..."
cargo test -p tipjar --test gas_benchmarks -- --nocapture

if [[ "${COVERAGE}" == "1" ]]; then
  echo "==> Generating coverage report..."
  cargo tarpaulin -p tipjar \
    --out Xml --output-dir coverage/ \
    --exclude-files "*/benches/*" \
    --timeout 120
  echo "Coverage report written to coverage/cobertura.xml"
fi

echo "==> All tests passed."
