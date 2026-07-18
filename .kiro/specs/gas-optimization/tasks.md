# Implementation Plan: Gas Optimization

## Overview

Implement storage tier corrections, batch deduplication, message cap, benchmarks, profiling script, and documentation for the TipJar Soroban contract. All contract changes are in `contracts/tipjar/src/lib.rs`; new files are `benches/gas_benchmarks.rs`, `scripts/profile_gas.sh`, and `docs/GAS_OPTIMIZATION.md`.

## Tasks

- [x] 1. Migrate CreatorBalance and CreatorTotal to persistent storage
  - [x] 1.1 Audit `lib.rs` for all `.instance().get/set` calls on `CreatorBalance` and `CreatorTotal` keys and replace with `.persistent().get/set`
    - Add a fallback read from instance storage during the transition so existing ledger entries are not silently lost
    - _Requirements: 3.5_
  - [ ]* 1.2 Write unit test: after `tip`, assert `CreatorBalance` is readable from persistent storage and absent from instance storage
    - _Requirements: 3.5_
  - [ ]* 1.3 Write property test for per-creator keys in persistent storage
    - **Property 3: Per-creator keys live in persistent storage**
    - **Validates: Requirements 3.5**

- [~] 2. Migrate TipperAggregate and CreatorAggregate to temporary storage
  - [x] 2.1 Replace all `.persistent().get/set` calls on `TipperAggregate` and `CreatorAggregate` keys with `.temporary().get/set`
    - _Requirements: 3.6_
  - [ ]* 2.2 Write unit test: after `tip`, assert `TipperAggregate` and `CreatorAggregate` are readable from temporary storage
    - _Requirements: 3.6_
  - [ ]* 2.3 Write property test for leaderboard aggregates in temporary storage
    - **Property 4: Leaderboard aggregates live in temporary storage**
    - **Validates: Requirements 3.6**

- [~] 3. Checkpoint — ensure all 52 existing tests still pass
  - Ensure all tests pass, ask the user if questions arise.

- [~] 4. Implement tip_batch leaderboard write deduplication
  - [~] 4.1 Refactor `tip_batch` to accumulate leaderboard deltas in two in-memory `Map<(Address, u32), (i128, u32)>` structures (creator deltas, tipper deltas) and flush once per unique `(address, bucket)` key after all entries are processed
    - Per-entry `CreatorBalance` and `CreatorTotal` writes remain unchanged (still per-entry)
    - _Requirements: 4.1, 4.2_
  - [ ]* 4.2 Write unit test: call `tip_batch` with 10 entries for the same creator; assert aggregate write count equals 3 (one per bucket)
    - _Requirements: 4.1, 4.2_
  - [ ]* 4.3 Write property test for tip_batch aggregate write deduplication
    - **Property 5: tip_batch aggregate write deduplication**
    - **Validates: Requirements 4.1, 4.2**
  - [ ]* 4.4 Write property test for tip_batch equivalence to N individual tips
    - **Property 6: tip_batch equivalence to N individual tips**
    - Use `proptest::collection::vec(arb_batch_tip(), 1..=50)` generator
    - Compare `CreatorBalance`, `CreatorTotal`, `TipperAggregate`, and `CreatorAggregate` values after batch vs individual calls
    - **Validates: Requirements 4.3, 8.4**

- [~] 5. Implement CreatorMessages ring buffer cap
  - [~] 5.1 Define `MAX_MESSAGES: u32 = 500` constant and update `tip_with_message` to remove index 0 (oldest) before appending when `msgs.len() >= MAX_MESSAGES`
    - Deserialize and re-serialize the list exactly once per invocation
    - _Requirements: 5.1, 5.2, 5.3, 5.4_
  - [ ]* 5.2 Write unit test: add exactly 500 messages then one more; assert list length is 500, oldest entry is gone, and new entry is present
    - _Requirements: 5.1, 5.2_
  - [ ]* 5.3 Write unit test: add fewer than 500 messages; assert all prior messages are present in original order
    - _Requirements: 5.3_
  - [ ]* 5.4 Write property test for CreatorMessages length invariant
    - **Property 7: CreatorMessages length invariant**
    - Use `proptest::collection::vec(arb_message(), 1..=1000)` generator; assert `len <= 500` after each call
    - **Validates: Requirements 5.1, 5.2**
  - [ ]* 5.5 Write property test for CreatorMessages append correctness below capacity
    - **Property 8: CreatorMessages append correctness below capacity**
    - Use `proptest::collection::vec(arb_message(), 0..500)` generator; assert `len + 1` and all prior messages present
    - **Validates: Requirements 5.3**

- [~] 6. Checkpoint — ensure all tests pass including new storage and ring buffer tests
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 7. Add storage round-trip property tests
  - [ ]* 7.1 Write property test for storage round-trip correctness
    - **Property 9: Storage round-trip correctness**
    - Use `proptest::num::i128::ANY` (positive range) for `CreatorBalance`; write to persistent storage, read back, assert equal
    - Also cover `CreatorMessages` list length and entry order, and aggregate totals/counts
    - **Validates: Requirements 8.1, 8.2, 8.3**

- [~] 8. Create benches/gas_benchmarks.rs with budget-based benchmark functions
  - [~] 8.1 Create `benches/gas_benchmarks.rs` with `#[test]` functions for all 8 benchmarks listed in the design; each function calls `env.mock_all_auths()`, `env.budget().reset_default()`, invokes the entry point, reads `cpu_instruction_count()` and `memory_bytes_count()`, prints `BENCH <name> cpu=<n> mem=<n>`, and asserts against the threshold where specified
    - Benchmarks: `gas_bench_tip_cold`, `gas_bench_tip_warm` (threshold 5M), `gas_bench_tip_with_message` (threshold 8M), `gas_bench_withdraw` (threshold 5M), `gas_bench_tip_batch_10`, `gas_bench_tip_batch_50` (threshold 50M), `gas_bench_tip_locked`, `gas_bench_get_total_tips` (threshold 1M)
    - On threshold failure print `BENCH_FAIL <label>: cpu=<actual> exceeded threshold=<limit>` before the assert panic
    - _Requirements: 2.1–2.11, 7.1–7.6_
  - [ ]* 8.2 Write property test for benchmark result line format
    - **Property 1: Benchmark result line format**
    - Assert that each benchmark emits a line matching `BENCH <name> cpu=<n> mem=<n>`
    - **Validates: Requirements 1.5, 2.9**
  - [ ]* 8.3 Write property test for threshold failure message completeness
    - **Property 10: Threshold failure message completeness**
    - Assert that a simulated threshold failure message contains both the actual CPU count and the threshold value
    - **Validates: Requirements 7.6**

- [~] 9. Add benches/gas_benchmarks.rs to Cargo.toml and wire into the package
  - Update `contracts/tipjar/Cargo.toml` (or workspace `Cargo.toml`) to include `proptest` as a dev-dependency and ensure `benches/gas_benchmarks.rs` is reachable via `cargo test --package tipjar -- gas_bench`
  - _Requirements: 2.1–2.11_

- [~] 10. Create scripts/profile_gas.sh
  - Implement the bash script: build WASM with `cargo build --target wasm32v1-none --release`, print artifact size (human-readable + raw bytes), run `cargo test --package tipjar -- gas_bench --nocapture` under a 300-second `timeout`, parse lines matching `BENCH <name> cpu=<n> mem=<n>`, print structured table, and emit `WARNING: no benchmark results captured` if no lines match
  - Exit 1 with a descriptive error on build failure
  - _Requirements: 1.1–1.7_

- [~] 11. Create docs/GAS_OPTIMIZATION.md
  - Write the documentation file with: Soroban storage cost model section (instance vs persistent vs temporary), before/after metrics table (placeholder values to be filled after benchmarks run), one section per applied optimization describing the key(s) affected and cost-reduction mechanism, and a deferred opportunities section with rationale
  - _Requirements: 6.1–6.6_

- [~] 12. Final checkpoint — ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests use the `proptest` crate and run a minimum of 100 iterations
- Each property test includes a comment: `// Feature: gas-optimization, Property N: <title>`
- The storage tier migration (tasks 1–2) must handle keys previously written to instance storage via fallback reads
