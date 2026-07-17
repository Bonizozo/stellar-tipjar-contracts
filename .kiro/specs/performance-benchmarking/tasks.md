# Implementation Plan: Contract Performance Benchmarking Suite

## Overview

Create a deterministic, threshold-guarded benchmarking suite for the TipJar Soroban contract using `env.budget()` for CPU and memory measurement. Deliverables: `contracts/tipjar/benches/gas_benchmarks.rs`, `scripts/run_benchmarks.sh`, and `scripts/analyze_results.py`. No contract source files are modified.

## Tasks

- [ ] 1. Create contracts/tipjar/benches/gas_benchmarks.rs
  - [ ] 1.1 Create the `benches/` directory and `gas_benchmarks.rs` file with `extern crate std;`, required `soroban_sdk` imports, and a `setup()` helper that registers `TipJarContract`, deploys a mock token, mints tokens to the sender, calls `client.init()`, and calls `client.add_token()`
    - Mirror the test setup pattern from `contracts/tipjar/src/lib.rs`
    - _Requirements: 1.1–1.10, 2.4_
  - [ ] 1.2 Implement `gas_bench_tip_cold`: call `env.budget().reset_default()`, invoke `client.tip()` for a creator with no prior balance, read and print `BENCH tip_cold cpu=<n> mem=<n>`
    - No threshold assertion for cold benchmark
    - _Requirements: 1.1, 2.1–2.5_
  - [ ] 1.3 Implement `gas_bench_tip_warm`: make one tip before the measurement window, call `env.budget().reset_default()`, invoke `client.tip()` again, assert `cpu < 5_000_000`
    - Print `BENCH_FAIL tip_warm: cpu=<n> exceeded threshold=5000000` before assert on failure
    - _Requirements: 1.2, 4.1_
  - [ ] 1.4 Implement `gas_bench_tip_with_message`: call `env.budget().reset_default()`, invoke `client.tip()` with a non-empty message string, assert `cpu < 8_000_000`
    - _Requirements: 1.3, 4.2_
  - [ ] 1.5 Implement `gas_bench_withdraw`: make one tip before the measurement window, call `env.budget().reset_default()`, invoke `client.withdraw()`, assert `cpu < 5_000_000`
    - _Requirements: 1.4, 4.4_
  - [ ] 1.6 Implement `gas_bench_tip_batch_10`: call `env.budget().reset_default()`, invoke `client.tip()` 10 times in a loop, print `BENCH tip_batch_10 cpu=<n> mem=<n>`
    - No threshold assertion
    - _Requirements: 1.5, 3.1_
  - [ ] 1.7 Implement `gas_bench_tip_batch_50`: call `env.budget().reset_default()`, invoke `client.tip()` 50 times in a loop, assert `cpu < 50_000_000`
    - _Requirements: 1.6, 3.2, 4.3_
  - [ ] 1.8 Implement `gas_bench_tip_locked`: call `env.budget().reset_default()`, invoke `client.tip()` with a message encoding a future unlock timestamp, print `BENCH tip_locked cpu=<n> mem=<n>`
    - No threshold assertion; simulates locked-tip overhead via message payload
    - _Requirements: 1.7_
  - [ ] 1.9 Implement `gas_bench_get_total_tips`: make one tip before the measurement window, call `env.budget().reset_default()`, invoke `client.get_total_tips()`, assert `cpu < 1_000_000`
    - _Requirements: 1.8, 3.4, 4.5_
  - [ ] 1.10 Implement `gas_bench_get_top_tippers`: seed 3 distinct sender addresses each making a tip, call `env.budget().reset_default()`, invoke `client.get_total_tips()` for each sender, print `BENCH get_top_tippers cpu=<n> mem=<n>`
    - No threshold assertion; measures leaderboard read cost across multiple addresses
    - _Requirements: 1.9, 3.5_
  - [ ] 1.11 Implement `gas_bench_get_top_creators`: seed 3 distinct creator addresses each receiving a tip, call `env.budget().reset_default()`, invoke `client.get_total_tips()` for each creator, print `BENCH get_top_creators cpu=<n> mem=<n>`
    - No threshold assertion
    - _Requirements: 1.10, 3.5_

- [ ] 2. Wire benches/gas_benchmarks.rs into Cargo.toml
  - [ ] 2.1 Add a `[[test]]` section to `contracts/tipjar/Cargo.toml` pointing to `benches/gas_benchmarks.rs` with `name = "gas_benchmarks"` so it is compiled as a test binary with the `testutils` feature enabled
    - Ensure `soroban-sdk` testutils feature is active for this target
    - _Requirements: 7.1, 7.5_

- [ ] 3. Create scripts/run_benchmarks.sh
  - [ ] 3.1 Implement the bash script: run `cargo test --package tipjar -- gas_bench --nocapture 2>&1`, capture exit code, exit 1 with error message on failure, parse `BENCH` lines with `grep`/`awk`, print formatted table sorted by CPU descending, print `WARNING: no benchmark results captured` and exit 1 if no lines matched
    - _Requirements: 7.2, 7.3, 7.4, 5.2_

- [ ] 4. Create scripts/analyze_results.py
  - [ ] 4.1 Implement the Python script: accept stdin or file path argument, parse lines matching `^BENCH (\S+) cpu=(\d+) mem=(\d+)`, compute relative cost column, print table sorted by CPU descending, warn and exit 1 if no lines matched
    - _Requirements: 6.1–6.6, 5.3_

- [ ] 5. Verify benchmark suite runs via cargo test
  - [ ] 5.1 Confirm `cargo test --package tipjar --test gas_benchmarks -- --nocapture` compiles and all benchmark functions are discovered
    - Do not run the tests; just verify the file structure and Cargo.toml wiring are correct
    - _Requirements: 7.1_

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- The `benches/` directory is non-standard for `#[test]` functions; a `[[test]]` entry in Cargo.toml is required to compile it as a test binary
- All benchmarks use `env.mock_all_auths()` to suppress authorization overhead
- `env.budget().reset_default()` must be called immediately before the measured invocation, after all setup is complete
- The contract source (`lib.rs`) must NOT be modified
