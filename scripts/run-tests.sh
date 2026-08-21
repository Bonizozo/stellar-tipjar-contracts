#!/usr/bin/env bash
# run-tests.sh — unified test runner for local development and CI.
set -euo pipefail

THREADS=${TEST_THREADS:-4}
COVERAGE=${COVERAGE:-0}

echo "==> Building contract (WASM)..."
cargo build -p tipjar --target wasm32v1-none --release

echo "==> Building v2 upgrade test fixture (WASM)..."
# contracts/tipjar/src/test_upgrade.rs embeds this via contractimport! at
# compile time, so it must exist on disk before `cargo test -p tipjar` runs.
cargo build -p tipjar-v2-fixture --target wasm32v1-none --release

echo "==> Verifying pause_tests / partial_pause_tests wiring..."
# Guard against the orphaned-test-tree failure mode: confirm the two
# root-level integration tests are still registered as real [[test]]
# targets of the `tipjar` package (see tests/README.md).
cargo test -p tipjar --test pause_tests --test partial_pause_tests -- --list

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
