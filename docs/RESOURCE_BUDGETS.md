# Resource Budget Enforcement

## Overview

The Stellar TipJar contract is subject to Soroban's network resource limits on every transaction. This document describes how we establish, maintain, and enforce per-entrypoint resource budgets to prevent silent regressions that only manifest as fee walls or network capacity exhaustion in production.

### Why Budgets Matter

Soroban meters four key resources against per-transaction limits:

| Resource | Network Limit | Impact of Overage |
|----------|---------------|-------------------|
| CPU Instructions | ~15M per tx | Transaction fee increases (linear) |
| Memory Bytes | ~536MB per tx | Rarely hit; limits large data structures |
| Ledger Reads | 100 per tx | Transaction fee increases; may read-limit hot creators |
| Ledger Writes | 100 per tx | Transaction fee increases; may write-limit high-volume periods |

A PR that doubles an entrypoint's CPU cost will merge silently but fail when users hit fee walls (100s of stroops vs acceptable rates) or when the network reaches capacity.

## Budget Baselines

Machine-readable budgets are stored in `tools/gas/budgets.json`. Each entrypoint has committed baselines for:

- **CPU instructions** — primary driver (token transfers, storage serialization)
- **Memory bytes** — peak heap allocation
- **Ledger reads/writes** — persistent storage access patterns
- **Event emission** — bytes in emitted topics/data

### Baseline Maintenance

1. **First PR** — establish baseline by running contract under budget metering:
   ```bash
   cargo test -p tipjar -- --nocapture | grep BUDGET
   ```
   Record per-entrypoint numbers in `tools/gas/budgets.json` with justification.

2. **Ratcheting** — if a PR improves a baseline, update the JSON with the new (lower) value and document the optimization in the PR description.

3. **Regression** — if a PR increases any dimension by >5%, the PR must fail CI. Either optimize or justify why the regression is acceptable (with measured delta).

## Current Baselines

See `tools/gas/budgets.json` for the authoritative baseline. Summary:

| Entrypoint | CPU Instructions | Ledger Writes | Notes |
|------------|------------------|----------------|-------|
| `init` | ~185K | 1 | Single instance write, no reads |
| `tip` (first-time) | ~1.25M | 3 | Cold storage (new balance/total keys), token transfer |
| `tip` (repeat) | ~890K | 3 | Warm storage (existing keys), token transfer |
| `get_total_tips` | ~125K | 0 | Read-only query, no state changes |
| `withdraw` | ~950K | 2 | Read balance, zero it, transfer tokens |

Token transfers dominate `tip` and `withdraw` costs; there is limited optimization available in the contract itself.

## CI Integration

The `gas-check.yml` workflow runs on every PR:

1. **Profile current branch** — run benchmarks, generate `gas-report.json`
2. **Download main baseline** — fetch previous `gas-report.json` from main branch (if exists)
3. **Compare** — compute per-entrypoint deltas; fail if >5% regression in any dimension
4. **Comment on PR** — post delta table as GitHub comment (worst offenders first)

### PR Comment Format

```
## 📊 Gas Report

| Entrypoint | CPU (baseline) | CPU (current) | Delta | Status |
|------------|---|---|---|---|
| init | 185K | 185K | +0% | ✅ |
| tip (first) | 1.25M | 1.30M | +4% | ✅ |
| withdraw | 950K | 1.05M | +11% | 🔴 REGRESSED |

**Optimization Opportunities:**
- `tip`: Token transfer cannot be optimized (contract interface); no other improvements found.
- `withdraw`: Check if balance zero-check can be optimized or if auth cost changed.

**Action Required:** Baseline update and justification needed for withdraw regression.
```

## Per-Entrypoint Deep Dives

### `init`

**Path:** one-time contract initialization, sets token address.

```
init()
  ├─ storage.instance().has() — instance read (cheap)
  ├─ storage.instance().set() — write Token key (1 ledger entry)
  └─ storage.instance().extend_ttl() — TTL bump
```

**Baseline:** ~185K CPU, 0 reads, 1 write.

**Optimization Notes:**
- Double-init check is necessary; unavoidable.
- No other storage or cross-contract calls; near-minimal.

### `tip` and `tip_with_message`

**Path:** escrow tokens, update balance + total, optionally store message.

```
tip()
  ├─ require_auth(sender) — auth check (part of Soroban SDK overhead)
  ├─ token_client.transfer() — cross-contract call (DOMINATES cost, ~70% of CPU)
  ├─ storage.persistent().get(balance_key) — read existing balance (or 0 if new)
  ├─ storage.persistent().get(total_key) — read existing total (or 0 if new)
  ├─ checked_add() operations — safe arithmetic
  ├─ storage.persistent().set(balance_key) — write new balance
  ├─ storage.persistent().set(total_key) — write new total
  ├─ storage.persistent().extend_ttl() × 2 — bump both keys
  ├─ storage.instance().extend_ttl() — bump instance
  └─ event.publish() — emit Tip event
```

**Baseline:**
- **First-time (cold storage):** ~1.25M CPU, 2 reads (both miss), 3 writes (balance, total, instance)
- **Repeat (warm storage):** ~890K CPU, 2 reads (hit), 3 writes

**Why the difference?** Soroban charges more for allocating new ledger entries than updating existing ones (cold vs warm storage).

**Optimization Notes:**
1. **Token transfer cannot be optimized** — fixed cost; contract must interact with token contract.
2. **Merge balance + total into single struct** — reduce 2 persistent writes to 1 (requires migration):
   - Current: `CreatorBalance(addr)` + `CreatorTotal(addr)` = 2 keys
   - Proposed: `CreatorData(addr) = { balance, total }`
   - Benefit: 1 read instead of 2, 1 write instead of 2, ~35% CPU reduction on warm path
   - Cost: Data migration required; existing escrowed balances must be re-keyed
   - Verdict: Worth doing in a future opt PR; too risky for feature flag.

3. **Batch message writes** — `tip_with_message` reads + re-serializes the entire `CreatorMessages` Vec on every call. For heavy tippers, this is expensive. Mitigation: cap message count per creator (e.g., 500) or migrate to append-only event log.

### `get_total_tips`

**Path:** read-only query for creator's historical total.

```
get_total_tips(creator)
  └─ storage.persistent().get(CreatorTotal(creator)) → returns 0 if not found
```

**Baseline:** ~125K CPU, 1 read, 0 writes.

**Optimization Notes:**
- Already optimal (pure read, no mutations).
- May be called frequently by indexers; ensure ledger access is not bottleneck (not a contract concern).

### `withdraw`

**Path:** pay out creator's full balance, reset to zero.

```
withdraw(creator)
  ├─ require_auth(creator) — auth check
  ├─ storage.persistent().get(balance_key) — read current balance
  ├─ validation: balance > 0 or error
  ├─ token_client.transfer() — cross-contract call (DOMINATES cost, ~70% of CPU)
  ├─ storage.persistent().set(balance_key, 0) — zero out balance
  ├─ storage.persistent().extend_ttl(balance_key) — bump TTL
  ├─ storage.instance().extend_ttl() — bump instance
  └─ event.publish() — emit Withdraw event
```

**Baseline:** ~950K CPU, 1 read, 2 writes (balance, instance).

**Note:** Historical total (`CreatorTotal`) is left untouched; only balance is reset.

**Optimization Notes:**
- Token transfer dominates; little else can be optimized.
- Zero-check is necessary and cheap.

## WASM Size Budget

The optimized release WASM is built with `opt-level="z"`, LTO enabled, and target `wasm32v1-none`:

```toml
[profile.release]
opt-level = "z"       # Optimize for size
lto = true            # Link-time optimization
codegen-units = 1     # Enable more aggressive optimization
panic = "abort"       # Avoid panic unwinding overhead
strip = true          # Strip debug symbols
```

**Budget:** < 150 KB.

**Current:** ~98 KB.

**Enforcement:** CI job measures artifact size after `stellar contract optimize` and fails if > budget.

## Test Infrastructure

### Budget Assertion Tests

Located in `contracts/tipjar/src/budget_tests.rs`:

```rust
#[test]
fn budget_tip_first_time() {
    // Run entrypoint under metering
    // Capture budget().cpu_instructions(), budget().memory_bytes(), etc.
    // Assert actual <= baseline * 1.05 (5% tolerance)
    // Fail with clear message if regression
}
```

All budget tests run as part of `cargo test -p tipjar`.

### Running Locally

```bash
# Run budget assertions
cargo test -p tipjar budget_ -- --nocapture

# Profile current state
bash scripts/profile-gas.sh

# Compare against baseline
bash scripts/profile-gas.sh --baseline baseline/gas-report.json
```

## Network Limits & Fee Implications

### Soroban Metering

Transaction fees in Soroban are computed as:

```
base_fee = 100 stroops (per-operation minimum)
resource_fee = resource_cost_per_unit × consumed_units
total_fee = max(base_fee, resource_fee)
```

For a typical `tip`:

- **CPU:** ~1M instructions at current fee per unit ~= 1000 stroops (~0.0001 XLM)
- **Memory:** ~100KB at current fee per unit ~= 100 stroops (~0.00001 XLM)
- **Ledger writes:** ~3 entries at current fee per unit ~= 300 stroops (~0.00003 XLM)
- **Token transfer:** Cross-contract call adds ~2000 stroops

**Total typical `tip` fee:** ~3400 stroops ≈ 0.00034 XLM (varies with network load).

If CPU usage doubles due to a regression:
- Resource fee could reach ~2000 stroops for tip alone
- **Total fee:** ~5000 stroops ≈ 0.0005 XLM (50% increase)

At scale (millions of tips), even small per-transaction increases add up.

### Preventing Silent Regressions

By enforcing budgets in CI with a 5% tolerance:
- Regressions are caught before merge
- Developers must justify or optimize
- Network remains sustainable

## Update Procedure

### Scenario 1: Intentional Optimization

You've optimized `tip` and CPU instructions dropped from 1.25M to 1.18M (5% improvement).

```bash
# 1. Update tools/gas/budgets.json
{
  "entrypoints": {
    "tip_first_time": {
      "cpu_instructions": 1_180_000,  // was 1_250_000
      // ... other fields unchanged ...
      "notes": "Optimized by merging balance/total reads into single struct access (see PR #XXX)"
    }
  }
}

# 2. Commit
git add tools/gas/budgets.json
git commit -m "chore: ratchet CPU budget for tip (1.25M → 1.18M)"

# 3. CI passes; PR merges
```

### Scenario 2: Unavoidable Regression

A feature addition increases memory by 15%; you've determined it's necessary.

```bash
# 1. Document in PR description with measured delta + justification
# 2. Update tools/gas/budgets.json with new baseline + clear "Reason" field
# 3. Ensure CI passes (new baseline is within tolerance of itself)
# 4. Request review highlighting the tradeoff

# Example commit message:
# feat: add creator preferences storage (memory +15%)
# 
# New persistent storage for per-creator settings (language, tax ID, etc.)
# increases memory baseline from 67KB to 77KB (+15%).
# 
# Rationale: feature requested by 50+ creators in feedback survey;
#           storage cost (~77KB per active creator) is acceptable
#           (network has 1GB per-ledger budget for all contracts).
#
# See tools/gas/budgets.json for updated baseline.
```

### Scenario 3: Regression Without Justification

CI fails; `tip` CPU increased by 12% and there's no optimization.

```
❌ CI FAILURE: tip_first_time CPU regression
   Baseline: 1_250_000 instructions
   Actual:   1_400_000 instructions
   Regression: +12% (tolerance: +5%)
   
   To fix:
   1. Identify the cause (profile the code)
   2. Optimize (reduce storage reads, batch operations, etc.)
   3. Update tools/gas/budgets.json and re-push
   
   OR
   
   Revert the change and try a different approach.
```

## Dashboard Integration

The PR comment from `gas-check.yml` is posted to every PR. Use it to:

1. **Spot trends** — if multiple PRs show 2-3% increases, something systemic changed
2. **Identify hotspots** — which entrypoints are most expensive?
3. **Plan optimizations** — prioritize low-hanging fruit

Example:
```
🔴 withdraw is most expensive at 950K CPU (75% token transfer)
🟡 tip_first_time at 1.25M (67% token transfer)
🟢 get_total_tips at 125K (optimal for read-only)

Top optimization opportunity: Switch to batch transfer for multi-creator payouts?
```

## Related Documents

- `GAS_OPTIMIZATION.md` — Optimizations already landed, techniques used
- `ARCHITECTURE.md` — Storage model, entry point design
- `STORAGE.md` — Detailed per-key ledger semantics
- `RECOVERY.md` — Handling storage archival (TTL expiry)

## Questions & Troubleshooting

**Q: My PR reduced CPU by 50% but CI still fails.**
A: CI compares against main's baseline, not your new code. If main's baseline is stale, fetch the latest and rebuild. If your code is genuinely faster, update `budgets.json` with the improvement and re-push.

**Q: Budget tolerance is too strict (+5%). Can we relax it?**
A: The tolerance balances catching regressions vs allowing natural variation in the test environment. If you observe >5% variance on unmodified code, investigate whether the Soroban SDK or test harness changed. Do not raise tolerance without data.

**Q: I'm hitting the budget limit. What can I do?**
A: Profile to identify the cost driver (token transfer, storage, auth?). If it's unavoidable (e.g., token transfer), consider architectural changes: batch operations, off-chain computation, indexer-assisted queries. Discuss with team before implementing major changes.
