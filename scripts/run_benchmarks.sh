#!/usr/bin/env bash
# run_benchmarks.sh — Run the TipJar contract performance benchmarks and
# display a formatted summary table sorted by CPU instruction count.
#
# Usage:
#   ./scripts/run_benchmarks.sh
#
# Exit codes:
#   0  All benchmarks passed their threshold assertions and results were captured.
#   1  cargo test failed, or no BENCH output lines were found.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== TipJar Contract Performance Benchmarks ==="
echo ""

# Run the benchmark test binary and capture combined stdout+stderr.
BENCH_OUTPUT=$(
    cargo test \
        --package tipjar \
        --test gas_benchmarks \
        -- --nocapture 2>&1
) || {
    echo "ERROR: cargo test exited with a non-zero status." >&2
    echo "Output:" >&2
    echo "${BENCH_OUTPUT}" >&2
    exit 1
}

# Extract lines matching: BENCH <name> cpu=<n> mem=<n>
BENCH_LINES=$(echo "${BENCH_OUTPUT}" | grep -E '^BENCH [^ ]+ cpu=[0-9]+ mem=[0-9]+' || true)

if [ -z "${BENCH_LINES}" ]; then
    echo "WARNING: no benchmark results captured." >&2
    echo ""
    echo "Raw output:"
    echo "${BENCH_OUTPUT}"
    exit 1
fi

# Print formatted table header.
printf "\n%-30s %20s %20s\n" "Benchmark" "CPU Instructions" "Memory Bytes"
printf "%-30s %20s %20s\n" "$(printf '%0.s-' {1..30})" "$(printf '%0.s-' {1..20})" "$(printf '%0.s-' {1..20})"

# Parse, sort by CPU descending, and print each row.
echo "${BENCH_LINES}" \
    | awk '{
        name = $2
        cpu  = $3; sub(/cpu=/, "", cpu)
        mem  = $4; sub(/mem=/, "", mem)
        printf "%s %s %s\n", name, cpu, mem
    }' \
    | sort -t' ' -k2 -rn \
    | awk '{
        printf "%-30s %20s %20s\n", $1, $2, $3
    }'

echo ""
echo "=== Benchmark run complete ==="
