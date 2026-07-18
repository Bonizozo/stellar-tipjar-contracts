#!/usr/bin/env python3
"""analyze_results.py — Parse BENCH output lines and generate a comparison table.

Reads benchmark output from stdin or a file path provided as the first argument.
Each line must match the format:

    BENCH <name> cpu=<n> mem=<n>

Output is a table sorted by CPU instruction count (descending) with columns:
    Benchmark | CPU Instructions | Memory Bytes | Relative Cost

Relative Cost is the ratio of each benchmark's CPU count to the minimum CPU
count in the run, rounded to two decimal places.

Exit codes:
    0  At least one BENCH line was parsed and the table was printed.
    1  No matching lines were found in the input.

Usage:
    # From a file:
    python3 scripts/analyze_results.py results.txt

    # From stdin (pipe from run_benchmarks.sh):
    ./scripts/run_benchmarks.sh | python3 scripts/analyze_results.py

    # From a saved cargo test run:
    cargo test --package tipjar --test gas_benchmarks -- --nocapture 2>&1 \
        | python3 scripts/analyze_results.py
"""

import re
import sys

# Regex matching: BENCH <name> cpu=<n> mem=<n>
_BENCH_RE = re.compile(r"^BENCH\s+(\S+)\s+cpu=(\d+)\s+mem=(\d+)")


def parse_lines(lines):
    """Return a list of (name, cpu, mem) tuples from matching input lines."""
    results = []
    for line in lines:
        m = _BENCH_RE.match(line.strip())
        if m:
            name = m.group(1)
            cpu = int(m.group(2))
            mem = int(m.group(3))
            results.append((name, cpu, mem))
    return results


def format_table(results):
    """Return a formatted comparison table string sorted by CPU descending."""
    sorted_results = sorted(results, key=lambda r: r[1], reverse=True)
    min_cpu = min(r[1] for r in sorted_results) or 1  # avoid division by zero

    col_bench = "Benchmark"
    col_cpu   = "CPU Instructions"
    col_mem   = "Memory Bytes"
    col_rel   = "Relative Cost"

    # Determine column widths.
    w_bench = max(len(col_bench), max(len(r[0]) for r in sorted_results))
    w_cpu   = max(len(col_cpu),   max(len(f"{r[1]:,}") for r in sorted_results))
    w_mem   = max(len(col_mem),   max(len(f"{r[2]:,}") for r in sorted_results))
    w_rel   = max(len(col_rel),   6)  # "999.99"

    sep = (
        f"{'─' * w_bench}  {'─' * w_cpu}  {'─' * w_mem}  {'─' * w_rel}"
    )
    header = (
        f"{col_bench:<{w_bench}}  {col_cpu:>{w_cpu}}  "
        f"{col_mem:>{w_mem}}  {col_rel:>{w_rel}}"
    )

    lines = [header, sep]
    for name, cpu, mem in sorted_results:
        relative = cpu / min_cpu
        lines.append(
            f"{name:<{w_bench}}  {cpu:>{w_cpu},}  {mem:>{w_mem},}  {relative:>{w_rel}.2f}x"
        )

    return "\n".join(lines)


def main():
    # Determine input source.
    if len(sys.argv) > 1:
        path = sys.argv[1]
        try:
            with open(path, "r", encoding="utf-8") as fh:
                lines = fh.readlines()
        except OSError as exc:
            print(f"ERROR: cannot open '{path}': {exc}", file=sys.stderr)
            sys.exit(1)
    else:
        lines = sys.stdin.readlines()

    results = parse_lines(lines)

    if not results:
        print(
            "WARNING: no BENCH lines found in input. "
            "Run with --nocapture to ensure benchmark output is visible.",
            file=sys.stderr,
        )
        sys.exit(1)

    print(f"\nTipJar Contract Performance Benchmark Results")
    print(f"({len(results)} benchmark(s) parsed)\n")
    print(format_table(results))
    print()


if __name__ == "__main__":
    main()
