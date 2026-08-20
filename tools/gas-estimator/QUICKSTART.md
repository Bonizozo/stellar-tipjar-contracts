# Storage-Rent Analysis — Quick Start

## TL;DR

```bash
cd stellar-tipjar-contracts/tools/gas-estimator
cargo run --release --bin storage-rent-analysis -- \
  --creators 100000 \
  --tokens-per-creator 3 \
  --years 3 \
  --activity mixed \
  --verbose
```

**Expected output**: 1.06 XLM total cost over 3 years (~$0.11 USD)

## What This Tool Does

Models long-run storage-rent costs for the TipJar contract as the number of (creator, token) pairs grows.

Answers:
- ✅ How much does storage cost over time?
- ✅ Do dormant entries naturally archive?
- ✅ Does reading a dormant entry keep it alive?

## Quick Reference

### Scenarios

```bash
# 100k creators, mixed activity (50% active, 50% dormant)
cargo run --release --bin storage-rent-analysis -- \
  --creators 100000 --tokens-per-creator 3 --years 3 --activity mixed

# All active (worst case)
cargo run --release --bin storage-rent-analysis -- \
  --creators 100000 --tokens-per-creator 3 --years 3 --activity active

# All dormant (best case)
cargo run --release --bin storage-rent-analysis -- \
  --creators 100000 --tokens-per-creator 3 --years 1 --activity dormant

# 1M creators, 10 years
cargo run --release --bin storage-rent-analysis -- \
  --creators 1000000 --tokens-per-creator 5 --years 10 --activity mixed
```

### Key Findings (100k creators × 3 tokens × 3 years)

| Activity | Active Entries | Archived Entries | Total Cost | Cost/Creator/Year |
|----------|----------------|------------------|------------|-------------------|
| Mixed | 150,000 | 150,000 | 1.06 XLM | 0.0000035 XLM |
| Active | 300,000 | 0 | 21.12 XLM | 0.000007 XLM |
| Dormant | 0 | 300,000 | 13.26 XLM | 0.0000044 XLM |

**Conclusion**: Storage cost is negligible (~$0.11 USD for 100k creators over 3 years).

## Output Files

- **storage-rent-report.json**: Full JSON report with monthly snapshots
- **stdout**: Summary table and recommendations

## Documentation

- **README_STORAGE_RENT.md**: Detailed tool documentation
- **../../docs/STORAGE_RENT_ANALYSIS.md**: Full analysis and findings
- **../../ISSUE_424_SUMMARY.md**: Executive summary for issue #424

## Key Insights

1. **Dormant entries archive naturally** after ~7 days (120,960 ledgers) with no activity
2. **Storage cost scales with active usage**, not total historical usage
3. **Read operations do NOT perpetually extend TTL** (post-migration)
4. **No manual cleanup required** — Stellar's TTL mechanism handles archival

## Activity Models Explained

| Model | Behavior | Use Case |
|-------|----------|----------|
| **active** | All creators tip monthly | Worst-case cost projection |
| **dormant** | All creators tip once at t=0 | Best-case archival scenario |
| **mixed** | 50% active, 50% dormant | Realistic production estimate |

## Cost Breakdown

**First tip (cold storage)**:
- Create Balance entry: 10,000 stroops
- Create Total entry: 10,000 stroops
- Extend TTL (2 entries × 7 days): 24,192 stroops
- **Total**: 44,192 stroops (~0.004 XLM)

**Subsequent tip (warm storage)**:
- Extend Balance TTL: 12,096 stroops
- Extend Total TTL: 12,096 stroops
- **Total**: 24,192 stroops (~0.002 XLM)

## Monitoring in Production

```bash
# Check active entry count on testnet
stellar contract fetch --network testnet --id <CONTRACT_ID> --key Balance

# Alert if active entries exceed 2× expected creator count
# (indicates minimal archival)
```

## Need Help?

See full documentation:
- Tool usage: `README_STORAGE_RENT.md`
- Analysis: `../../docs/STORAGE_RENT_ANALYSIS.md`
- Issue summary: `../../ISSUE_424_SUMMARY.md`
