# Gas Optimization Report

## Overview

Soroban charges fees based on two resources: **CPU instructions** and **memory bytes**.
Every storage read/write and cross-contract call (token transfer) is the dominant cost driver.
This document records the optimizations applied to the TipJar contract and how to measure them.

---

## Optimizations Applied

### 1. Removed dead refund-tracking code (`tip` / `tip_with_message`)

The previous code contained references to `TipRecord`, `DataKey::TipRecord`, and `next_tip_id`
that were never defined — causing a compile error and adding conceptual overhead.
Removing them eliminates two extra persistent storage writes per `tip` call.

| Before | After |
|--------|-------|
| 4 persistent writes (balance, total, tip_record, counter) | 2 persistent writes (balance, total) |

### 2. Reuse the `persistent()` storage handle within a function

Calling `env.storage().persistent()` is a cheap accessor, but caching it in a local
variable makes the intent explicit and avoids repeated method dispatch:

```rust
// Before — repeated accessor calls
env.storage().persistent().get(&balance_key).unwrap_or(0);
env.storage().persistent().set(&balance_key, &next_balance);
env.storage().persistent().get(&total_key).unwrap_or(0);
env.storage().persistent().set(&total_key, &next_total);

// After — single accessor, reused
let storage = env.storage().persistent();
let next_balance: i128 = storage.get(&balance_key).unwrap_or(0) + amount;
let next_total:   i128 = storage.get(&total_key).unwrap_or(0) + amount;
storage.set(&balance_key, &next_balance);
storage.set(&total_key, &next_total);
```

### 3. Removed intermediate `contract_address` variable

`env.current_contract_address()` is an inline host function call. Storing it in a
named variable added a stack slot with no benefit; the call is now inlined directly
into `token_client.transfer(...)`.

### 4. Removed unused `GRACE_PERIOD_SECS` constant

Dead code that the compiler warned about. Removing it keeps the binary lean.

---

## Storage Cost Model (Soroban)

| Operation | Relative cost |
|-----------|--------------|
| `instance` read/write | Cheapest — shared across all callers in one ledger entry |
| `persistent` read/write | Medium — per-key ledger entry, survives ledger close |
| `temporary` read/write | Cheapest persistent-style — auto-expires |
| Cross-contract call (token transfer) | Most expensive — separate contract invocation |

**Key insight:** the token `transfer` call dominates every tip operation.
Storage optimizations reduce the surrounding overhead but cannot eliminate that cost.

---

## Benchmark Results

Run benchmarks yourself:

```bash
bash scripts/analyze_gas.sh
# or directly:
cargo test -p tipjar -- bench --nocapture
```

The benchmarks in `tests/gas/benchmarks.rs` use `env.budget()` to capture
CPU instructions and memory bytes for each entry point:

| Benchmark | What it measures |
|-----------|-----------------|
| `bench_tip_single` | Cold-storage tip (first tip for a creator) |
| `bench_tip_warm_storage` | Warm-storage tip (existing balance/total keys) |
| `bench_tip_with_message` | Tip with message + metadata |
| `bench_withdraw` | Creator withdrawal |
| `bench_tip_batch_10` | Batch of 10 tips |
| `bench_tip_batch_50` | Batch of 50 tips (maximum allowed) |
| `bench_get_total_tips` | Read-only query |
| `bench_tip_locked` | Time-locked tip creation |

Cold vs warm storage: the first tip for a creator is more expensive because
Soroban must allocate new ledger entries. Subsequent tips update existing entries
at lower cost.

---

## Further Optimization Opportunities

1. **Use `temporary` storage for leaderboard aggregates** — leaderboard data is
   time-bucketed (weekly/monthly) and naturally expires. Switching from `persistent`
   to `temporary` storage would reduce rent fees significantly.

2. **Batch leaderboard updates** — `update_leaderboard_aggregates` writes to 6
   storage keys (2 aggregates × 3 periods + up to 2 participant lists). For
   `tip_batch`, these writes repeat per entry. A future optimization could
   accumulate deltas in memory and flush once per batch.

3. **Cap `CreatorMessages` list growth** — the messages `Vec` is loaded and
   re-serialised on every `tip_with_message`. Introducing a per-creator message
   count cap (e.g. 500) bounds the serialisation cost.

4. **Instance storage for hot flags** — `Paused` and `TokenWhitelist` are already
   in instance storage (cheapest tier). No change needed there.
