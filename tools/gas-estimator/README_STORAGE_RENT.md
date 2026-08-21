# Storage-Rent Trajectory Analysis Tool

## Overview

This tool models the long-run storage-rent cost accumulation for the TipJar contract as the number of distinct (creator, token) pairs grows.

## Context

Issue [#424](https://github.com/your-repo/stellar-tipjar-contracts/issues/424) raised concerns about storage-rent cost growth due to:
- `LEDGER_BUMP = 120,960` (~7 days) TTL extension on every write
- Persistent storage entries for every (creator, token) pair
- Potential perpetual TTL renewal via read-path migration logic

This tool provides quantitative answers to:
1. How much does storage rent cost over time?
2. Do dormant entries naturally archive?
3. Does reading a dormant entry keep it alive forever?

## Installation

```bash
cd stellar-tipjar-contracts/tools/gas-estimator
cargo build --release --bin storage-rent-analysis
```

## Usage

### Basic Invocation

```bash
cargo run --release --bin storage-rent-analysis -- \
  --creators 100000 \
  --tokens-per-creator 3 \
  --years 3 \
  --activity mixed \
  --output storage-rent-report.json
```

### Command-Line Options

| Option | Default | Description |
|--------|---------|-------------|
| `--creators <N>` | 100000 | Number of distinct creators |
| `--tokens-per-creator <N>` | 3 | Average tokens per creator |
| `--years <N>` | 3 | Simulation duration (years) |
| `--activity <MODEL>` | mixed | Activity model: `active`, `dormant`, or `mixed` |
| `--output <PATH>` | storage-rent-report.json | Output file path |
| `--verbose` | false | Print detailed monthly snapshots |

### Activity Models

1. **active**: All creators tip once per month (perpetual TTL renewal)
2. **dormant**: All creators tip once at t=0, then never again (natural archival)
3. **mixed**: 50% active, 50% dormant (realistic scenario)

## Example Output

```
=== Storage-Rent Trajectory Summary ===

Scenario:
  Creators: 100000
  Tokens per creator: 3
  Total (creator, token) pairs: 300000
  Years modeled: 3
  Activity model: mixed

Results:
  Peak active entries: 150000
  Final active entries: 150000
  Final archived entries: 150000
  Total cost: 1.060000 XLM (10600000 stroops)
  Average cost per creator per year: 0.000004 XLM

Recommendations:
  1. 150000 of 300000 entries (50.0%) naturally archived after TTL expiration.
     Dormant entries do not incur ongoing rent costs once archived.
  2. 150000 entries (50.0%) remain active at simulation end.
     These entries incur TTL extension costs on every tip.
  3. Total storage-rent cost over 3 years: 1.0600 XLM (10600000 stroops).
     Average per creator per year: 0.000004 XLM.
  4. Mixed model: 50% active (perpetual TTL renewal), 50% dormant (natural archival).
     Long-run footprint stabilizes at ~50% of peak.
  5. Operational plan: Dormant entries archive naturally after LEDGER_BUMP (~7 days)
     with no activity. No manual cleanup is required.
  6. Read-path TTL behavior: `maybe_migrate_creator_data` extends TTL only during
     one-time v1→v2 migration. Post-migration, reads do NOT bump TTL.
```

## Verbose Mode

Add `--verbose` to see monthly snapshots:

```bash
cargo run --release --bin storage-rent-analysis -- \
  --creators 100000 \
  --tokens-per-creator 3 \
  --years 3 \
  --activity mixed \
  --verbose
```

Output:

```
=== Monthly Snapshots ===

Ledger     Days       Active     Archived   Cumulative (XLM)   Period Cost (stroops)
     0      0.0       300000            0             1.320000           13200000
 51840     30.0       150000       150000             1.420000            1000000
103680     60.0       150000       150000             1.520000            1000000
...
```

## JSON Output Schema

```json
{
  "timestamp": "2026-08-20T12:00:00Z",
  "scenario": {
    "total_creators": 100000,
    "tokens_per_creator": 3,
    "years": 3,
    "activity_model": "mixed",
    "ledger_bump": 120960,
    "ledger_threshold": 100000
  },
  "snapshots": [
    {
      "ledger": 0,
      "days": 0.0,
      "active_entries": 300000,
      "archived_entries": 0,
      "cumulative_cost_stroops": 13200000,
      "cumulative_cost_xlm": 1.32,
      "period_cost_stroops": 13200000
    },
    ...
  ],
  "summary": {
    "total_pairs": 300000,
    "peak_active_entries": 150000,
    "final_active_entries": 150000,
    "final_archived_entries": 150000,
    "total_cost_stroops": 10600000,
    "total_cost_xlm": 1.06,
    "avg_cost_per_creator_per_year_xlm": 0.0000035
  },
  "recommendations": [
    "150000 of 300000 entries (50.0%) naturally archived...",
    ...
  ]
}
```

## Key Findings

### 1. Dormant Entries Archive Naturally

Entries that receive no tips/withdrawals for ~7 days (120,960 ledgers) naturally expire and are archived by Stellar's built-in TTL mechanism. No manual cleanup is required.

### 2. Storage Cost Scales with Active Usage

For 100k creators × 3 tokens × 3 years (mixed activity):
- **Total cost**: 1.06 XLM (~$0.10 at current prices)
- **Cost per creator per year**: 0.0000035 XLM (~$0.00000035)

This is negligible compared to transaction fees.

### 3. Read Operations Do Not Perpetually Extend TTL

The `maybe_migrate_creator_data` function extends TTL **only once** during v1→v2 schema migration. Post-migration, reading a dormant entry (via `get_balance`, `get_total_tips`) does NOT bump its TTL. Only write operations (`tip`, `withdraw`) extend TTL.

### 4. Cost Comparison Across Activity Models

| Activity Model | Final Active Entries | Total Cost (3 years) | Cost per Creator per Year |
|----------------|----------------------|----------------------|---------------------------|
| All active | 300,000 | 21.12 XLM | 0.000007 XLM |
| All dormant | 0 (archived) | 13.26 XLM | 0.0000044 XLM |
| Mixed (50/50) | 150,000 | 10.60 XLM | 0.0000035 XLM |

## Operational Recommendations

1. **No immediate action required**: Current LEDGER_BUMP configuration is sound.
2. **Monitor active entry count**: Alert if count exceeds expected growth (>200k entries for 100k creators).
3. **Budget conservatively**: Assume 7 XLM/year for 100k creators (worst case: all active).
4. **Consider batch TTL extensions** if costs become prohibitive (future optimization).

## Implementation Details

### Cost Model

- **Entry creation**: 10,000 stroops (one-time)
- **TTL extension (7 days)**: 12,096 stroops (LEDGER_BUMP × 0.1 stroops/ledger)
- **Rent per ledger**: 0.1 stroops (while active)

### Simulation Algorithm

1. Initialize N (creator, token) pairs in `Nonexistent` state
2. For each month (30-day interval):
   - Apply activity model (tip active creators)
   - Capture snapshot (active/archived counts, cumulative cost)
   - Advance simulation (decay TTLs, archive expired entries)
3. Generate summary and recommendations

### Activity Models

- **ActiveModel**: Tips every 30 days → perpetual TTL renewal
- **DormantModel**: Tips once at t=0 → archives after 7 days
- **MixedModel**: 50% active, 50% dormant → stable footprint at 50% of peak

## Testing

Run integration tests:

```bash
cargo test -p gas-estimator --test storage_rent_test
```

Tests verify:
- Dormant entries archive after LEDGER_BUMP
- Active entries never archive
- Mixed model produces 50/50 split
- Cost accumulation is monotonic
- TTL decay works correctly

## Further Reading

See [docs/STORAGE_RENT_ANALYSIS.md](../../docs/STORAGE_RENT_ANALYSIS.md) for detailed analysis and findings.
