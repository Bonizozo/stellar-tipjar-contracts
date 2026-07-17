# Design Document: Gas Optimization

## Overview

This feature delivers measurable resource consumption improvements to the TipJar Soroban smart contract, along with tooling to profile, benchmark, and document those improvements. Soroban charges for CPU instructions and memory bytes — not Ethereum-style gas — so the optimization strategy targets the two primary cost drivers: storage tier selection and redundant storage writes.

The work has four concrete deliverables:
1. `scripts/profile_gas.sh` — build + benchmark driver script
2. `benches/gas_benchmarks.rs` — Soroban test-env budget benchmarks
3. Contract changes in `contracts/tipjar/src/lib.rs` — storage tier fixes, batch deduplication, message cap
4. `docs/GAS_OPTIMIZATION.md` — before/after metrics and cost model documentation

### Soroban Resource Model

Soroban measures two resource dimensions per transaction:

| Resource | Unit | Limit (approx.) |
|---|---|---|
| CPU instructions | count | ~100M per tx |
| Memory bytes | bytes | ~40MB per tx |

Storage costs are charged per ledger entry read/write, with the tier determining the base fee multiplier:

| Tier | API | Cost characteristic | Expiry |
|---|---|---|---|
| Instance | `env.storage().instance()` | Single shared entry; cheapest per-read | Never (while contract exists) |
| Persistent | `env.storage().persistent()` | Per-key entry; medium cost | Never (unless TTL lapses) |
| Temporary | `env.storage().temporary()` | Per-key entry; cheapest for ephemeral data | Auto-expires after TTL |

Reading from instance storage costs one ledger entry access regardless of how many keys are fetched in that invocation, making it ideal for frequently-read flags like `Paused`, `Admin`, and `TokenWhitelist`.

---

## Architecture

The optimization work touches three layers:

```
┌─────────────────────────────────────────────────────┐
│  scripts/profile_gas.sh                             │
│  (build WASM → run benchmarks → emit report)        │
└────────────────────┬────────────────────────────────┘
                     │ invokes
┌────────────────────▼────────────────────────────────┐
│  benches/gas_benchmarks.rs                          │
│  (soroban test env + env.budget() measurements)     │
└────────────────────┬────────────────────────────────┘
                     │ exercises
┌────────────────────▼────────────────────────────────┐
│  contracts/tipjar/src/lib.rs                        │
│  ┌──────────────────────────────────────────────┐   │
│  │ Storage tier assignments (Req 3)             │   │
│  │ tip_batch deduplication (Req 4)              │   │
│  │ CreatorMessages ring buffer (Req 5)          │   │
│  └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
                     │ documented in
┌────────────────────▼────────────────────────────────┐
│  docs/GAS_OPTIMIZATION.md                           │
│  (cost model, before/after table, deferred items)   │
└─────────────────────────────────────────────────────┘
```

No new contract entry points are added. All changes are internal to existing functions or additive (new files).

---

## Components and Interfaces

### 1. `scripts/profile_gas.sh`

A bash script with the following flow:

```
1. cargo build --target wasm32v1-none --release
   → on failure: print error, exit 1
   → on success: stat the .wasm file, print human-readable + raw byte size

2. cargo test --package tipjar -- gas_bench
   → captures stdout
   → parses lines matching: BENCH <name> cpu=<n> mem=<n>
   → on no matches: print WARNING: no benchmark results captured
   → on matches: print structured table to stdout

3. enforce 300-second wall-clock timeout (via `timeout 300 ...` or equivalent)
```

The script does not require any arguments. It is self-contained and idempotent.

### 2. `benches/gas_benchmarks.rs`

Because Soroban contracts compile to WASM and run inside the Soroban host VM, Criterion.rs wall-clock benchmarks are not meaningful for instruction/memory measurement. Instead, benchmarks use the Soroban test environment's `env.budget()` API:

```rust
// Pattern used for every benchmark
env.budget().reset_default();
// ... invoke contract function ...
let cpu = env.budget().cpu_instruction_count();
let mem = env.budget().memory_bytes_count();
println!("BENCH {label} cpu={cpu} mem={mem}");
assert!(cpu < THRESHOLD, "cpu {cpu} exceeded threshold {THRESHOLD}");
```

Each benchmark function is a `#[test]` function (not a Criterion benchmark group) so it runs under `cargo test`. The profile script invokes `cargo test -- gas_bench` to run only these functions.

Benchmarks defined:

| Function name | Entry point | Condition | CPU threshold |
|---|---|---|---|
| `gas_bench_tip_cold` | `tip` | cold storage | — |
| `gas_bench_tip_warm` | `tip` | warm storage | 5,000,000 |
| `gas_bench_tip_with_message` | `tip_with_message` | — | 8,000,000 |
| `gas_bench_withdraw` | `withdraw` | — | 5,000,000 |
| `gas_bench_tip_batch_10` | `tip_batch` | batch=10 | — |
| `gas_bench_tip_batch_50` | `tip_batch` | batch=50 | 50,000,000 |
| `gas_bench_tip_locked` | `tip_locked` | — | — |
| `gas_bench_get_total_tips` | `get_total_tips` | — | 1,000,000 |

All benchmarks call `env.mock_all_auths()` before invocation.

### 3. Contract Changes (`lib.rs`)

#### 3a. Storage Tier Corrections

The current code stores `CreatorBalance` and `CreatorTotal` in instance storage (`.instance().get/set`). Per the requirements, these must move to persistent storage (`.persistent().get/set`). `Paused`, `Admin`, and `TokenWhitelist` are already in instance storage and remain there.

Leaderboard aggregates (`TipperAggregate`, `CreatorAggregate`) move from persistent to temporary storage, since they are time-bucketed and can be regenerated.

#### 3b. `tip_batch` Deduplication

Current behavior: `tip_batch` calls `update_leaderboard_aggregates` for each entry, causing N writes per unique (address, bucket) pair.

New behavior: accumulate deltas in a `Map<(Address, u32), i128>` in memory, then flush once per unique key after all entries are processed.

```
tip_batch(entries):
  let mut creator_deltas: Map<(Address, u32), i128> = {}
  let mut tipper_deltas: Map<(Address, u32), i128> = {}

  for entry in entries:
    // update CreatorBalance, CreatorTotal (persistent) — still per-entry
    // accumulate leaderboard deltas in memory
    for bucket in [AllTime, Monthly, Weekly]:
      creator_deltas[(entry.creator, bucket)] += entry.amount
      tipper_deltas[(entry.sender, bucket)] += entry.amount

  // single write per unique (address, bucket) pair
  for (key, delta) in creator_deltas:
    flush_creator_aggregate(key, delta)
  for (key, delta) in tipper_deltas:
    flush_tipper_aggregate(key, delta)
```

This reduces leaderboard writes from O(N × buckets) to O(unique_addresses × buckets).

#### 3c. `CreatorMessages` Ring Buffer

Current behavior: unbounded `Vec<TipWithMessage>` appended on every `tip_with_message` call.

New behavior: cap at `MAX_MESSAGES = 500`. When the list is at capacity, remove index 0 (oldest) before pushing the new entry.

```rust
const MAX_MESSAGES: u32 = 500;

// in tip_with_message:
let mut msgs: Vec<TipWithMessage> = storage.get(&key).unwrap_or_default();
if msgs.len() >= MAX_MESSAGES {
    msgs.remove(0);  // drop oldest
}
msgs.push_back(new_msg);
storage.set(&key, &msgs);
```

The list is deserialized and re-serialized exactly once per invocation (satisfying Req 5.4).

### 4. `docs/GAS_OPTIMIZATION.md`

Structure:
1. Soroban storage cost model (instance vs persistent vs temporary)
2. Before/after metrics table (populated after benchmarks run)
3. Applied optimizations (one section per change, with key(s) affected and mechanism)
4. Deferred opportunities (with rationale)

---

## Data Models

### Modified Storage Key Routing

| DataKey | Before | After | Rationale |
|---|---|---|---|
| `Paused` | Instance | Instance | No change — already optimal |
| `Admin` | Instance | Instance | No change — already optimal |
| `TokenWhitelist(addr)` | Instance | Instance | No change — already optimal |
| `CreatorBalance(creator, token)` | Instance | Persistent | Per-creator data; should not share instance entry |
| `CreatorTotal(creator, token)` | Instance | Persistent | Per-creator data; should not share instance entry |
| `TipperAggregate(addr, bucket)` | Persistent | Temporary | Time-bucketed; auto-expiry is acceptable |
| `CreatorAggregate(addr, bucket)` | Persistent | Temporary | Time-bucketed; auto-expiry is acceptable |

### In-Memory Accumulator (tip_batch)

Used only during a single `tip_batch` invocation; never persisted directly.

```rust
struct LeaderboardDelta {
    address: Address,
    bucket_id: u32,
    amount_delta: i128,
    count_delta: u32,
}
```

Implemented as two `soroban_sdk::Map` values (creator deltas, tipper deltas) keyed by `(Address, u32)`.

### CreatorMessages Invariant

```
len(CreatorMessages(creator)) <= MAX_MESSAGES (500)
```

The ring buffer maintains this invariant after every `tip_with_message` call.

---

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system — essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Benchmark result line format

*For any* benchmark that completes, the output line emitted by the benchmark suite must contain the benchmark label, a CPU instruction count, and a memory byte count in the structured `BENCH <name> cpu=<n> mem=<n>` format.

**Validates: Requirements 1.5, 2.9**

---

### Property 2: Instance storage keys are read from instance tier

*For any* invocation of `tip`, the reads of `Paused`, `Admin`, and `TokenWhitelist` entries must all be satisfied from the single instance storage ledger entry, not from separate persistent or temporary entries.

**Validates: Requirements 3.1, 3.2, 3.3, 3.4**

---

### Property 3: Per-creator keys live in persistent storage

*For any* creator address and token address, the `CreatorBalance` and `CreatorTotal` values must be stored in and retrieved from persistent storage (not instance storage).

**Validates: Requirements 3.5**

---

### Property 4: Leaderboard aggregates live in temporary storage

*For any* leaderboard aggregate write (`TipperAggregate`, `CreatorAggregate`) for any time bucket, the value must be stored in temporary storage.

**Validates: Requirements 3.6**

---

### Property 5: tip_batch aggregate write deduplication

*For any* `tip_batch` call with N entries, the number of storage writes to any single `(address, bucket)` aggregate key must be at most 1, regardless of how many entries in the batch share that address.

**Validates: Requirements 4.1, 4.2**

---

### Property 6: tip_batch equivalence to N individual tips

*For any* sequence of tip entries, executing them as a single `tip_batch` call must produce identical `CreatorBalance`, `CreatorTotal`, `TipperAggregate`, and `CreatorAggregate` values as executing the same entries as N individual `tip` calls.

**Validates: Requirements 4.3, 8.4**

---

### Property 7: CreatorMessages length invariant

*For any* creator and any number of `tip_with_message` calls, the length of that creator's message list must never exceed 500.

**Validates: Requirements 5.1, 5.2**

---

### Property 8: CreatorMessages append correctness below capacity

*For any* creator whose message list has fewer than 500 entries, after one `tip_with_message` call the list length must increase by exactly 1 and all previously existing messages must still be present in their original order.

**Validates: Requirements 5.3**

---

### Property 9: Storage round-trip correctness

*For any* valid value written to storage under a `CreatorBalance`, `CreatorMessages`, `TipperAggregate`, or `CreatorAggregate` key, reading that key back in the same ledger must return a value equal to what was written (same amount/count for numeric types; same length and entry order for list types).

**Validates: Requirements 8.1, 8.2, 8.3**

---

### Property 10: Threshold failure message completeness

*For any* benchmark that fails its CPU instruction threshold assertion, the error output must include both the actual CPU instruction count and the threshold value that was exceeded.

**Validates: Requirements 7.6**

---

## Error Handling

### Script errors (`profile_gas.sh`)

| Condition | Behaviour |
|---|---|
| `cargo build` exits non-zero | Print descriptive error to stderr, exit 1 |
| WASM artifact not found after build | Print error, exit 1 |
| Benchmark output contains no matching lines | Print `WARNING: no benchmark results captured` to stdout, continue (exit 0) |
| Script exceeds 300s | `timeout` wrapper kills child processes, script exits non-zero |

### Contract errors

| Condition | Error | Notes |
|---|---|---|
| `tip_batch` with > 50 entries | `BatchTooLarge` | Existing guard, unchanged |
| `tip_with_message` with message > max length | `MessageTooLong` | Existing guard, unchanged |
| `tip` with non-whitelisted token | `TokenNotWhitelisted` | Existing guard, unchanged |
| `tip` with amount ≤ 0 | `InvalidAmount` | Existing guard, unchanged |
| `withdraw` with zero balance | `NothingToWithdraw` | Existing guard, unchanged |

The storage tier migration (instance → persistent for `CreatorBalance`/`CreatorTotal`) must handle the case where a key was previously written to instance storage. A one-time migration helper or a fallback read from instance storage during the transition period should be considered during implementation.

### Benchmark assertion failures

When a CPU threshold assertion fails, the benchmark must not panic silently. It must print:

```
BENCH_FAIL <label>: cpu=<actual> exceeded threshold=<limit>
```

before the assertion panic propagates.

---

## Testing Strategy

### Dual approach

Both unit tests and property-based tests are required. They are complementary:

- Unit tests catch concrete bugs at specific inputs and verify integration points.
- Property tests verify universal correctness across randomly generated inputs.

### Unit tests (in `contracts/tipjar/src/lib.rs` test module)

Focus areas:
- Storage tier verification: assert that after `tip`, `CreatorBalance` is readable from persistent storage and not from instance storage.
- Ring buffer boundary: add exactly 500 messages, then add one more; assert oldest is gone and new one is present (edge case for Property 7).
- Batch deduplication: call `tip_batch` with 10 entries for the same creator; assert aggregate write count = 3 (one per bucket).
- Batch equivalence: run 5 individual `tip` calls then compare state to a single `tip_batch` with the same 5 entries.
- Threshold assertions: each benchmark function in `benches/gas_benchmarks.rs` includes an inline `assert!(cpu < THRESHOLD)`.

### Property-based tests

Use the `proptest` crate (already compatible with `no_std` via `proptest` feature flags, or use `quickcheck` as an alternative). Each property test runs a minimum of 100 iterations.

Each test is tagged with a comment referencing the design property:

```rust
// Feature: gas-optimization, Property 7: CreatorMessages length invariant
```

| Property | Test description | Generator |
|---|---|---|
| P6: tip_batch equivalence | Generate random Vec<BatchTip> (len 1–50), run as batch and as N individual tips, compare all storage values | `proptest::collection::vec(arb_batch_tip(), 1..=50)` |
| P7: CreatorMessages length invariant | Generate random sequence of tip_with_message calls (len 1–1000), assert list len ≤ 500 after each | `proptest::collection::vec(arb_message(), 1..=1000)` |
| P8: CreatorMessages append correctness | Generate list with len 0–499, add one message, assert len+1 and all prior messages present | `proptest::collection::vec(arb_message(), 0..500)` |
| P9: Storage round-trip | Generate random i128 balance, write to persistent storage, read back, assert equal | `proptest::num::i128::ANY` (positive range) |

### Benchmark tests (`benches/gas_benchmarks.rs`)

Each benchmark is a `#[test]` function using the Soroban test environment:

```rust
#[test]
fn gas_bench_tip_warm() {
    // Feature: gas-optimization, Property 2: Instance storage keys are read from instance tier
    let env = Env::default();
    env.mock_all_auths();
    // ... setup ...
    env.budget().reset_default();
    // ... invoke tip ...
    let cpu = env.budget().cpu_instruction_count();
    let mem = env.budget().memory_bytes_count();
    println!("BENCH tip_warm cpu={cpu} mem={mem}");
    assert!(cpu < 5_000_000, "BENCH_FAIL tip_warm: cpu={cpu} exceeded threshold=5000000");
}
```

The `profile_gas.sh` script runs these via `cargo test --package tipjar -- gas_bench --nocapture`.

### CI integration

Add to the CI pipeline:

```yaml
- name: Gas benchmarks
  run: cargo test --package tipjar -- gas_bench --nocapture
```

This catches regressions on every PR without requiring a separate benchmark runner.
