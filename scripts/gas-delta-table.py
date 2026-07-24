#!/usr/bin/env python3
"""
gas-delta-table.py — Compare two gas reports and generate a markdown PR comment table.

Usage:
    python3 scripts/gas-delta-table.py <current-report.json> <baseline-report.json>

Outputs a markdown table sorted by worst regression first, with visual indicators.
"""

import json
import sys
from typing import Dict, List, Tuple

TOLERANCE_PCT = 5.0


def load_report(path: str) -> Dict:
    """Load a gas-report.json file."""
    with open(path, 'r') as f:
        return json.load(f)


def compute_delta(current: int, baseline: int) -> Tuple[int, float]:
    """Compute absolute and percentage delta."""
    if baseline == 0:
        return 0, 0.0
    delta_pct = ((current - baseline) / baseline) * 100.0
    return current - baseline, delta_pct


def status_icon(delta_pct: float) -> str:
    """Return visual indicator based on delta."""
    if delta_pct > TOLERANCE_PCT:
        return "🔴"
    elif delta_pct > TOLERANCE_PCT * 0.6:  # 3% = 60% of 5%
        return "🟡"
    else:
        return "✅"


def format_number(n: int) -> str:
    """Format large numbers with separators."""
    return f"{n:,}"


def main():
    if len(sys.argv) != 3:
        print("Usage: python3 gas-delta-table.py <current> <baseline>", file=sys.stderr)
        sys.exit(1)

    current_path = sys.argv[1]
    baseline_path = sys.argv[2]

    try:
        current = load_report(current_path)
        baseline = load_report(baseline_path)
    except Exception as e:
        print(f"Error loading reports: {e}", file=sys.stderr)
        sys.exit(1)

    # Extract results
    current_results = {r["function"]: r for r in current.get("results", [])}
    baseline_results = {r["function"]: r for r in baseline.get("results", [])}

    # Compute deltas, sorted by worst regression
    deltas = []
    for func, base in baseline_results.items():
        if func not in current_results:
            continue

        curr = current_results[func]
        cpu_delta, cpu_pct = compute_delta(curr["cpu_instructions"], base["cpu_instructions"])
        mem_delta, mem_pct = compute_delta(curr["memory_bytes"], base["memory_bytes"])

        # Use CPU as primary sort key
        deltas.append({
            "function": func,
            "cpu_delta": cpu_delta,
            "cpu_pct": cpu_pct,
            "mem_delta": mem_delta,
            "mem_pct": mem_pct,
            "cpu_base": base["cpu_instructions"],
            "cpu_curr": curr["cpu_instructions"],
            "mem_base": base["memory_bytes"],
            "mem_curr": curr["memory_bytes"],
        })

    # Sort by CPU delta (worst first)
    deltas.sort(key=lambda x: x["cpu_pct"], reverse=True)

    # Generate markdown table
    print("| Function | CPU (baseline) | CPU (current) | CPU Delta | Memory (baseline) | Memory (current) | Memory Delta | Status |")
    print("|----------|---|---|---|---|---|---|---|")

    regression_count = 0
    for d in deltas:
        icon = status_icon(d["cpu_pct"])
        if d["cpu_pct"] > TOLERANCE_PCT:
            regression_count += 1

        cpu_fmt = f"{d['cpu_pct']:+.1f}%"
        mem_fmt = f"{d['mem_pct']:+.1f}%"

        print(
            f"| {d['function']} | {format_number(d['cpu_base'])} | {format_number(d['cpu_curr'])} | "
            f"{cpu_fmt} | {format_number(d['mem_base'])} | {format_number(d['mem_curr'])} | "
            f"{mem_fmt} | {icon} |"
        )

    # Summary
    print()
    if regression_count > 0:
        print(f"**⚠️  {regression_count} regression(s) detected** (>5% increase)")
        print()
        print("**Regression Details:**")
        for d in deltas:
            if d["cpu_pct"] > TOLERANCE_PCT:
                print(
                    f"- **{d['function']}**: CPU +{d['cpu_pct']:.1f}% "
                    f"({format_number(d['cpu_base'])} → {format_number(d['cpu_curr'])})"
                )
    else:
        print("✅ **All operations within tolerance (+5%)**")


if __name__ == "__main__":
    main()
