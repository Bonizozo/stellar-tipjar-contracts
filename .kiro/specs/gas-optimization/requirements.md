# Requirements Document

## Introduction

This feature delivers a comprehensive gas (resource) optimization analysis and improvement suite for the TipJar Soroban smart contract. Soroban measures resource consumption in CPU instructions and memory bytes rather than Ethereum-style gas. The primary cost drivers are persistent storage reads/writes and cross-contract token transfer calls. The feature covers: a structured profiling script, Criterion.rs-based benchmarks, an updated optimization guide, and concrete contract-level storage improvements — with before/after metrics to validate each change.

## Glossary

- **Profiler**: The `scripts/profile_gas.sh` shell script that builds the WASM artifact, runs benchmarks, and emits a structured resource report.
- **Benchmark_Suite**: The `benches/gas_benchmarks.rs` Criterion.rs benchmark file that measures CPU instructions and memory bytes per contract entry point.
- **Budget**: The Soroban `env.budget()` API that tracks CPU instruction count and memory byte count consumed during a test invocation.
- **Optimization_Guide**: The `docs/GAS_OPTIMIZATION.md` document that records applied optimizations, the storage cost model, and further opportunities.
- **Contract**: The TipJar Soroban smart contract located at `contracts/tipjar/src/lib.rs`.
- **Instance_Storage**: Soroban `env.storage().instance()` — a single shared ledger entry for the contract; cheapest tier for frequently read flags.
- **Persistent_Storage**: Soroban `env.storage().persistent()` — per-key ledger entries that survive ledger close; medium cost.
- **Temporary_Storage**: Soroban `env.storage().temporary()` — per-key ledger entries that auto-expire; cheapest persistent-style tier.
- **Cold_Storage**: A storage read/write where the ledger entry does not yet exist and must be allocated.
- **Warm_Storage**: A storage read/write where the ledger entry already exists and is updated in place.
- **Batch_Operation**: The `tip_batch` contract entry point that processes multiple tips in a single transaction.
- **Leaderboard**: The time-bucketed (AllTime, Monthly, Weekly) aggregate tracking of tipper and creator statistics.

---

## Requirements

### Requirement 1: Gas Profiling Script

**User Story:** As a developer, I want a single script that builds the WASM artifact and runs all benchmarks, so that I can obtain a complete resource consumption report without manual steps.

#### Acceptance Criteria

1. THE Profiler SHALL build the optimised WASM artifact using `cargo build --target wasm32v1-none --release` before running any benchmarks.
2. WHEN the WASM build succeeds, THE Profiler SHALL report the artifact file size in both human-readable and raw byte formats.
3. WHEN the WASM build fails, THE Profiler SHALL exit with a non-zero status code and print a descriptive error message.
4. THE Profiler SHALL execute the Benchmark_Suite and capture CPU instruction counts and memory byte counts for each benchmark.
5. THE Profiler SHALL emit all benchmark results to standard output in a structured format that includes the benchmark name, CPU instructions, and memory bytes.
6. WHEN the Benchmark_Suite produces no output matching the expected format, THE Profiler SHALL print a warning indicating that no benchmark results were captured.
7. THE Profiler SHALL complete the full build-and-benchmark cycle within 300 seconds on a standard developer machine.

---

### Requirement 2: Criterion.rs Benchmark Suite

**User Story:** As a developer, I want Criterion.rs benchmarks for every major contract entry point, so that I can detect performance regressions in CI and compare before/after optimization metrics.

#### Acceptance Criteria

1. THE Benchmark_Suite SHALL measure CPU instructions and memory bytes for the `tip` entry point under cold storage conditions (first tip for a creator).
2. THE Benchmark_Suite SHALL measure CPU instructions and memory bytes for the `tip` entry point under warm storage conditions (subsequent tip for an existing creator).
3. THE Benchmark_Suite SHALL measure CPU instructions and memory bytes for the `tip_with_message` entry point.
4. THE Benchmark_Suite SHALL measure CPU instructions and memory bytes for the `withdraw` entry point.
5. THE Benchmark_Suite SHALL measure CPU instructions and memory bytes for the `tip_batch` entry point with a batch size of 10.
6. THE Benchmark_Suite SHALL measure CPU instructions and memory bytes for the `tip_batch` entry point with a batch size of 50 (maximum allowed).
7. THE Benchmark_Suite SHALL measure CPU instructions and memory bytes for the `tip_locked` entry point.
8. THE Benchmark_Suite SHALL measure CPU instructions and memory bytes for the `get_total_tips` read-only query entry point.
9. WHEN a benchmark completes, THE Benchmark_Suite SHALL print a result line containing the benchmark label, CPU instruction count, and memory byte count.
10. THE Benchmark_Suite SHALL reset the Budget to default limits before each individual benchmark measurement to ensure isolated results.
11. FOR ALL benchmark entry points, THE Benchmark_Suite SHALL use `env.mock_all_auths()` so that authorization overhead does not distort resource measurements.

---

### Requirement 3: Storage Tier Optimization

**User Story:** As a contract developer, I want frequently-read flags stored in the cheapest storage tier, so that every contract invocation minimises ledger entry read costs.

#### Acceptance Criteria

1. THE Contract SHALL store the `Paused` flag in Instance_Storage.
2. THE Contract SHALL store the `Admin` address in Instance_Storage.
3. THE Contract SHALL store `TokenWhitelist` entries in Instance_Storage.
4. WHEN a `tip` invocation reads the `Paused` flag and a `TokenWhitelist` entry, THE Contract SHALL read both values from Instance_Storage in a single ledger entry access.
5. THE Contract SHALL store per-creator balance (`CreatorBalance`) and total-tips (`CreatorTotal`) keys in Persistent_Storage.
6. WHEN leaderboard aggregate keys (`TipperAggregate`, `CreatorAggregate`) are written for a time bucket that has a defined expiry, THE Contract SHALL store those keys in Temporary_Storage.

---

### Requirement 4: Batch Leaderboard Write Reduction

**User Story:** As a contract developer, I want leaderboard storage writes batched within a `tip_batch` call, so that repeated per-entry writes to the same aggregate keys are eliminated.

#### Acceptance Criteria

1. WHEN `tip_batch` processes N tips for the same creator in a single call, THE Contract SHALL write the creator's leaderboard aggregate keys at most once per time bucket per call, regardless of N.
2. WHEN `tip_batch` processes N tips from the same sender in a single call, THE Contract SHALL write the sender's leaderboard aggregate keys at most once per time bucket per call, regardless of N.
3. THE Contract SHALL produce identical leaderboard aggregate values whether N individual `tip` calls or one `tip_batch` call with N entries is used.

---

### Requirement 5: CreatorMessages Growth Cap

**User Story:** As a contract developer, I want the per-creator message list capped at a defined maximum, so that `tip_with_message` serialisation cost is bounded regardless of historical tip volume.

#### Acceptance Criteria

1. THE Contract SHALL define a maximum message list length of 500 entries per creator.
2. WHEN a `tip_with_message` call would cause a creator's message list to exceed 500 entries, THE Contract SHALL remove the oldest entry before appending the new one.
3. WHEN a `tip_with_message` call is made and the creator's message list contains fewer than 500 entries, THE Contract SHALL append the new message without removing any existing entry.
4. THE Contract SHALL deserialise and re-serialise the `CreatorMessages` list at most once per `tip_with_message` invocation.

---

### Requirement 6: Gas Cost Comparison Report

**User Story:** As a developer, I want documented before/after resource metrics for each optimization, so that I can verify improvements are real and communicate them to stakeholders.

#### Acceptance Criteria

1. THE Optimization_Guide SHALL document the CPU instruction count and memory byte count for each benchmarked entry point before optimizations are applied.
2. THE Optimization_Guide SHALL document the CPU instruction count and memory byte count for each benchmarked entry point after optimizations are applied.
3. THE Optimization_Guide SHALL include a comparison table showing the absolute and percentage reduction in CPU instructions and memory bytes for each entry point.
4. THE Optimization_Guide SHALL describe each optimization applied, including the storage key(s) affected and the mechanism by which cost is reduced.
5. THE Optimization_Guide SHALL document the Soroban storage cost model, distinguishing between Instance_Storage, Persistent_Storage, and Temporary_Storage tiers.
6. THE Optimization_Guide SHALL list further optimization opportunities that were identified but not implemented in this iteration, with a rationale for deferral.

---

### Requirement 7: Benchmark Regression Detection

**User Story:** As a CI engineer, I want benchmark results to include assertions on maximum resource thresholds, so that regressions are caught automatically before merging.

#### Acceptance Criteria

1. WHEN the `tip` benchmark (warm storage) completes, THE Benchmark_Suite SHALL assert that CPU instructions consumed are below 5,000,000.
2. WHEN the `tip_with_message` benchmark completes, THE Benchmark_Suite SHALL assert that CPU instructions consumed are below 8,000,000.
3. WHEN the `tip_batch` benchmark with 50 entries completes, THE Benchmark_Suite SHALL assert that CPU instructions consumed are below 50,000,000.
4. WHEN the `withdraw` benchmark completes, THE Benchmark_Suite SHALL assert that CPU instructions consumed are below 5,000,000.
5. WHEN the `get_total_tips` benchmark completes, THE Benchmark_Suite SHALL assert that CPU instructions consumed are below 1,000,000.
6. IF any benchmark assertion fails, THEN THE Benchmark_Suite SHALL print the actual CPU instruction count alongside the threshold that was exceeded.

---

### Requirement 8: Round-Trip Storage Serialization Correctness

**User Story:** As a developer, I want to verify that storage serialization and deserialization of contract state is lossless, so that optimization changes do not introduce data corruption.

#### Acceptance Criteria

1. FOR ALL valid `CreatorBalance` values written to Persistent_Storage, THE Contract SHALL read back an identical value in the same ledger.
2. FOR ALL valid `CreatorMessages` lists written to Persistent_Storage, THE Contract SHALL read back a list with identical length and entry order.
3. FOR ALL valid leaderboard aggregate values written to storage, THE Contract SHALL read back identical aggregate totals and counts.
4. WHEN a `tip_batch` call completes, THE Contract SHALL reflect the sum of all tip amounts in the affected `CreatorBalance` entries, equal to the sum that would result from the same number of individual `tip` calls.
