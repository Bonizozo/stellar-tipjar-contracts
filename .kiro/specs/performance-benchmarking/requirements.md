# Requirements Document

## Introduction

This feature delivers a standalone Contract Performance Benchmarking Suite for the TipJar Soroban smart contract. Soroban measures resource consumption in CPU instructions and memory bytes via `env.budget()` rather than Ethereum-style gas. The suite provides reproducible, threshold-guarded benchmarks for every major contract entry point, a shell script driver for local and CI execution, a Python analysis tool for comparison reports, and documentation of optimization recommendations — all without modifying the contract source.

## Glossary

- **Benchmark_Suite**: The `contracts/tipjar/benches/gas_benchmarks.rs` file containing `#[test]` functions that measure CPU instructions and memory bytes per contract entry point.
- **Budget**: The Soroban `env.budget()` API that tracks `cpu_instruction_count()` and `memory_bytes_count()` consumed during a test invocation.
- **Threshold**: A maximum CPU instruction count defined per benchmark; exceeding it causes the test to fail with a descriptive message.
- **Cold_Storage**: A storage read/write where the ledger entry does not yet exist and must be allocated for the first time.
- **Warm_Storage**: A storage read/write where the ledger entry already exists and is updated in place.
- **Batch_Operation**: A `tip` call repeated N times within a single benchmark to simulate batch-like load.
- **Regression**: An increase in CPU instruction count or memory bytes that exceeds a defined threshold between code revisions.
- **Profiler**: The `scripts/run_benchmarks.sh` shell script that invokes the Benchmark_Suite and formats output.
- **Analyzer**: The `scripts/analyze_results.py` Python script that parses `BENCH` output lines and generates a comparison table.
- **Contract**: The TipJar Soroban smart contract at `contracts/tipjar/src/lib.rs`.

---

## Requirements

### Requirement 1: Benchmark All Contract Methods

**User Story:** As a developer, I want a benchmark for every major contract entry point, so that I have complete visibility into the resource cost of each operation.

#### Acceptance Criteria

1. THE Benchmark_Suite SHALL include a benchmark for the `tip` entry point under cold storage conditions (first tip for a creator).
2. THE Benchmark_Suite SHALL include a benchmark for the `tip` entry point under warm storage conditions (subsequent tip for an existing creator).
3. THE Benchmark_Suite SHALL include a benchmark for the `tip` entry point with a non-empty message string.
4. THE Benchmark_Suite SHALL include a benchmark for the `withdraw` entry point.
5. THE Benchmark_Suite SHALL include a benchmark for the `tip` entry point repeated 10 times to simulate a batch of 10.
6. THE Benchmark_Suite SHALL include a benchmark for the `tip` entry point repeated 50 times to simulate a batch of 50.
7. THE Benchmark_Suite SHALL include a benchmark for a locked-tip scenario using a future unlock timestamp.
8. THE Benchmark_Suite SHALL include a benchmark for the `get_total_tips` read-only query entry point.
9. THE Benchmark_Suite SHALL include a benchmark for a leaderboard top-tippers query.
10. THE Benchmark_Suite SHALL include a benchmark for a leaderboard top-creators query.

---

### Requirement 2: Measure CPU Instructions and Memory Bytes via env.budget()

**User Story:** As a developer, I want each benchmark to report CPU instruction count and memory byte count, so that I have precise, deterministic resource measurements independent of wall-clock time.

#### Acceptance Criteria

1. BEFORE each benchmark measurement, THE Benchmark_Suite SHALL call `env.budget().reset_default()` to clear any accumulated resource counts from prior operations.
2. AFTER each benchmark measurement, THE Benchmark_Suite SHALL read `env.budget().cpu_instruction_count()` and assign it to a named variable.
3. AFTER each benchmark measurement, THE Benchmark_Suite SHALL read `env.budget().memory_bytes_count()` and assign it to a named variable.
4. FOR ALL benchmarks, THE Benchmark_Suite SHALL call `env.mock_all_auths()` before any contract invocation so that authorization overhead does not distort resource measurements.
5. WHEN a benchmark completes, THE Benchmark_Suite SHALL print a result line in the exact format `BENCH <name> cpu=<n> mem=<n>` where `<name>` is the benchmark label, `<n>` for cpu is the integer CPU instruction count, and `<n>` for mem is the integer memory byte count.

---

### Requirement 3: Test with Varying Data Sizes

**User Story:** As a developer, I want benchmarks that exercise different data sizes, so that I can understand how resource consumption scales with input complexity.

#### Acceptance Criteria

1. THE Benchmark_Suite SHALL benchmark the `tip` entry point with a batch size of 10 (10 sequential tip calls) to measure linear scaling at small batch sizes.
2. THE Benchmark_Suite SHALL benchmark the `tip` entry point with a batch size of 50 (50 sequential tip calls) to measure linear scaling at large batch sizes.
3. THE Benchmark_Suite SHALL benchmark the `tip` entry point with a message string to measure the overhead of string serialization.
4. THE Benchmark_Suite SHALL benchmark the `get_total_tips` query after at least one prior tip to ensure the storage entry exists and is non-trivially sized.
5. THE Benchmark_Suite SHALL benchmark leaderboard queries after seeding at least 3 distinct tipper addresses so the result set is non-trivial.

---

### Requirement 4: Performance Regression Detection

**User Story:** As a CI engineer, I want benchmark assertions on maximum CPU instruction thresholds, so that performance regressions are caught automatically before merging.

#### Acceptance Criteria

1. WHEN the `gas_bench_tip_warm` benchmark completes, THE Benchmark_Suite SHALL assert that CPU instructions consumed are strictly below 5,000,000.
2. WHEN the `gas_bench_tip_with_message` benchmark completes, THE Benchmark_Suite SHALL assert that CPU instructions consumed are strictly below 8,000,000.
3. WHEN the `gas_bench_tip_batch_50` benchmark completes, THE Benchmark_Suite SHALL assert that CPU instructions consumed are strictly below 50,000,000.
4. WHEN the `gas_bench_withdraw` benchmark completes, THE Benchmark_Suite SHALL assert that CPU instructions consumed are strictly below 5,000,000.
5. WHEN the `gas_bench_get_total_tips` benchmark completes, THE Benchmark_Suite SHALL assert that CPU instructions consumed are strictly below 1,000,000.
6. WHEN any threshold assertion fails, THE Benchmark_Suite SHALL print the actual CPU instruction count and the threshold that was exceeded before the assertion panic propagates.

---

### Requirement 5: Optimization Recommendations

**User Story:** As a developer, I want benchmark output to highlight which operations are most expensive, so that I can prioritize optimization efforts.

#### Acceptance Criteria

1. THE Benchmark_Suite SHALL print the CPU instruction count and memory byte count for every benchmark, enabling manual comparison of relative costs.
2. THE Profiler SHALL parse all `BENCH` output lines and display them in a formatted table sorted by CPU instruction count descending, so the most expensive operations appear first.
3. THE Analyzer SHALL compute the ratio of each benchmark's CPU count to the lowest CPU count in the run, providing a relative cost multiplier column in the comparison table.

---

### Requirement 6: Comparison Reports

**User Story:** As a developer, I want a Python script that parses benchmark output and generates a comparison table, so that I can track changes across runs.

#### Acceptance Criteria

1. THE Analyzer SHALL accept benchmark output piped via stdin or provided as a file path argument.
2. THE Analyzer SHALL parse lines matching the pattern `BENCH <name> cpu=<n> mem=<n>` and extract the name, CPU count, and memory byte count from each line.
3. THE Analyzer SHALL output a formatted table with columns: Benchmark, CPU Instructions, Memory Bytes, and Relative Cost.
4. THE Analyzer SHALL skip lines that do not match the `BENCH` pattern without raising an error.
5. WHEN no matching lines are found, THE Analyzer SHALL print a warning message and exit with a non-zero status code.
6. THE Analyzer SHALL sort the output table by CPU instruction count in descending order.

---

### Requirement 7: CI Integration

**User Story:** As a CI engineer, I want the benchmark suite runnable via a single `cargo test` command, so that it integrates into existing CI pipelines without additional tooling.

#### Acceptance Criteria

1. THE Benchmark_Suite SHALL be executable via `cargo test --package tipjar -- gas_bench --nocapture` without any additional build steps.
2. THE Profiler SHALL invoke `cargo test --package tipjar -- gas_bench --nocapture` internally and capture its output.
3. THE Profiler SHALL exit with a non-zero status code if `cargo test` exits with a non-zero status code.
4. THE Profiler SHALL exit with status 0 and print a summary when all benchmarks pass their threshold assertions.
5. THE Benchmark_Suite SHALL NOT require any external benchmark framework (e.g., Criterion.rs) beyond the standard `soroban-sdk` test utilities already present in `[dev-dependencies]`.
