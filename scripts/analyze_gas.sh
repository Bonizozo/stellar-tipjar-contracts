#!/usr/bin/env bash
# analyze_gas.sh — Build the WASM and report size; run benchmarks and capture output.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WASM_PATH="$REPO_ROOT/target/wasm32v1-none/release/tipjar.wasm"

echo "=== TipJar Gas Analysis ==="
echo ""

# ── 1. Build optimised WASM ──────────────────────────────────────────────────
echo "[1/3] Building optimised WASM..."
cargo build -p tipjar --target wasm32v1-none --release \
    --manifest-path "$REPO_ROOT/Cargo.toml" 2>&1

if [[ -f "$WASM_PATH" ]]; then
    SIZE=$(du -h "$WASM_PATH" | cut -f1)
    BYTES=$(wc -c < "$WASM_PATH")
    echo ""
    echo "WASM artifact : $WASM_PATH"
    echo "WASM size     : $SIZE  ($BYTES bytes)"
else
    echo "WARNING: WASM not found at $WASM_PATH (wasm32v1-none target may not be installed)"
fi

echo ""

# ── 2. Run unit tests (correctness) ─────────────────────────────────────────
echo "[2/3] Running unit tests..."
cargo test -p tipjar --manifest-path "$REPO_ROOT/Cargo.toml" 2>&1 | tail -5
echo ""

# ── 3. Run benchmarks and capture CPU/memory output ─────────────────────────
echo "[3/3] Running gas benchmarks (--nocapture)..."
cargo test -p tipjar --manifest-path "$REPO_ROOT/Cargo.toml" \
    -- bench --nocapture 2>&1 | grep -E "\[BENCH\]|test bench|FAILED|ok" || true

echo ""
echo "=== Analysis complete ==="
echo "See docs/GAS_OPTIMIZATION.md for interpretation of results."
