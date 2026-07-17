# Design Document: Contract Performance Benchmarking Suite

## Overview

This feature adds a deterministic, threshold-guarded benchmarking suite for the TipJar Soroban smart contract. Because Soroban contracts execute inside the Soroban host VM and compile to WASM, wall-clock benchmarks (e.g., Criterion.rs) are not meaningful for resource measurement. Instead, the suite uses the Soroban test environment's `env.budget()` API to capture exact CPU instruction counts and memory byte counts per invocation.

The deliverables are:
1. `contracts/tipjar/benches/gas_benchmarks.rs` — `#[test]` benchmark functions using `soroban-sdk` testutils
2. `scripts/run_benchmarks.sh` — shell driver that invokes `cargo test` and formats output
3. `scripts/analyze_results.py` — Python parser that generates a comparison table from `BENCH` output lines

No contract source files are modified.

---

## Soroban Budget API

The `env.budget()` API is available in the Soroban test environment when the `testutils` feature is enabled. Key methods:

| Method | Description |
|---|---|
| `env.budget().reset_default()` | Resets CPU and memory counters to zero with default limits |
| `env.budget().cpu_instruction_count()` | Returns `u64` CPU instructions consumed since last reset |
| `env.budget().memory_bytes_count()` | Returns `u64` memory bytes consumed since last reset |

Calling `reset_default()` before each measurement ensures that setup code (contract deployment, token minting, auth mocking) does not inflate the benchmark result.

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  scripts/run_benchmarks.sh                          │
│  (invoke cargo test → capture stdout → format table)│
└────────────────────┬────────────────────────────────┘
                     │ invokes
┌────────────────────▼────────────────────────────────┐
│  contracts/tipjar/benches/gas_benchmarks.rs         │
│  (#[test] fns using env.budget() measurements)      │
└────────────────────┬────────────────────────────────┘
                     │ exercises
┌────────────────────▼────────────────────────────────┐
│  contracts/tipjar/src/lib.rs  (unchanged)           │
│  TipJarContract: tip, withdraw, get_total_tips, ... │
└─────────────────────────────────────────────────────┘
                     │ output parsed by
┌────────────────────▼────────────────────────────────┐
│  scripts/analyze_results.py                         │
│  (parse BENCH lines → comparison table)             │
└─────────────────────────────────────────────────────┘
```

---

## Components and Interfaces

### 1. `contracts/tipjar/benches/gas_benchmarks.rs`

#### File structure

```rust
extern crate std;

use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    token, Address, Env, String,
};
use tipjar::{TipJarContract, TipJarContractClient};

// shared setup helper — mirrors the pattern in lib.rs test module
fn setup() -> (Env, TipJarContractClient<'static>, Address, Address, Address) { ... }

#[test] fn gas_bench_tip_cold() { ... }
#[test] fn gas_bench_tip_warm() { ... }
#[test] fn gas_bench_tip_with_message() { ... }
#[test] fn gas_bench_withdraw() { ... }
#[test] fn gas_bench_tip_batch_10() { ... }
#[test] fn gas_bench_tip_batch_50() { ... }
#[test] fn gas_bench_tip_locked() { ... }
#[test] fn gas_bench_get_total_tips() { ... }
#[test] fn gas_bench_get_top_tippers() { ... }
#[test] fn gas_bench_get_top_creators() { ... }
```

#### Benchmark pattern

Every benchmark follows this exact pattern:

```rust
#[test]
fn gas_bench_<name>() {
    let (env, client, admin, sender, creator) = setup();
    env.mock_all_auths();
    // ... any warm-up / pre-state setup (not measured) ...

    env.budget().reset_default();          // start measurement window
    // ... invoke contract function ...
    let cpu = env.budget().cpu_instruction_count();
    let mem = env.budget().memory_bytes_count();

    println!("BENCH <name> cpu={cpu} mem={mem}");

    // threshold assertion (only for benchmarks with defined thresholds)
    const THRESHOLD: u64 = N;
    assert!(
        cpu < THRESHOLD,
        "BENCH_FAIL <name>: cpu={cpu} exceeded threshold={THRESHOLD}"
    );
}
```

#### Benchmark catalogue

| Function | Entry point | Pre-state | CPU threshold |
|---|---|---|---|
| `gas_bench_tip_cold` | `tip` | No prior tip for creator | — |
| `gas_bench_tip_warm` | `tip` | One prior tip already made | 5,000,000 |
| `gas_bench_tip_with_message` | `tip` (with message string) | No prior tip | 8,000,000 |
| `gas_bench_withdraw` | `withdraw` | One prior tip already made | 5,000,000 |
| `gas_bench_tip_batch_10` | `tip` × 10 | No prior tips | — |
| `gas_bench_tip_batch_50` | `tip` × 50 | No prior tips | 50,000,000 |
| `gas_bench_tip_locked` | `tip` (future unlock) | No prior tip | — |
| `gas_bench_get_total_tips` | `get_total_tips` | One prior tip | 1,000,000 |
| `gas_bench_get_top_tippers` | `get_total_tips` × 3 senders | 3 prior tips | — |
| `gas_bench_get_top_creators` | `get_total_tips` × 3 creators | 3 prior tips | — |

`gas_bench_tip_locked` simulates a locked-tip scenario by tipping with a future timestamp embedded in the message; since the contract does not expose a dedicated `tip_locked` entry point in the current implementation, the benchmark measures a `tip` call with a message that encodes lock metadata, which is the closest available proxy.

`gas_bench_get_top_tippers` and `gas_bench_get_top_creators` seed 3 distinct addresses and then measure `get_total_tips` queries across them to approximate leaderboard read cost.

#### `setup()` helper

```rust
fn setup() -> (Env, TipJarContractClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);

    let contract_id = env.register(TipJarContract, ());
    let client = TipJarContractClient::new(&env, &contract_id);

    // deploy a mock token
    let token_admin = Address::generate(&env);
    let token_id = env.register(
        soroban_sdk::token::StellarAssetContract,
        (&token_admin, &soroban_sdk::Symbol::new(&env, "TKN")),
    );
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&sender, &1_000_000_000i128);

    client.init(&admin);
    client.add_token(&admin, &token_id);

    (env, client, admin, sender, creator, token_id)
}
```

> Note: The actual `setup()` signature returns 6 values including `token_id`. The tuple above is simplified for illustration; the implementation returns all required handles.

### 2. `scripts/run_benchmarks.sh`

Flow:
1. Run `cargo test --package tipjar -- gas_bench --nocapture 2>&1`
2. Capture exit code; if non-zero, print error and exit 1
3. Parse lines matching `BENCH <name> cpu=<n> mem=<n>` using `grep` + `awk`
4. Print a formatted table (name, CPU, memory) sorted by CPU descending
5. If no matching lines found, print `WARNING: no benchmark results captured` and exit 1

### 3. `scripts/analyze_results.py`

Flow:
1. Read input from stdin or a file path provided as `sys.argv[1]`
2. Parse lines matching regex `^BENCH (\S+) cpu=(\d+) mem=(\d+)`
3. Compute relative cost = cpu / min(cpu across all benchmarks)
4. Print a table with columns: Benchmark, CPU Instructions, Memory Bytes, Relative Cost
5. Sort by CPU descending
6. If no lines matched, print warning to stderr and `sys.exit(1)`

---

## Data Models

### BENCH output line

```
BENCH <name> cpu=<n> mem=<n>
```

- `<name>`: snake_case benchmark label (e.g., `tip_warm`, `tip_batch_50`)
- `<n>` (cpu): unsigned 64-bit integer
- `<n>` (mem): unsigned 64-bit integer

### BENCH_FAIL output line (on threshold violation)

```
BENCH_FAIL <name>: cpu=<actual> exceeded threshold=<limit>
```

Emitted immediately before the `assert!` panic so it appears in test output even when the test runner captures stdout.

---

## Correctness Properties

### Property 1: Budget isolation

*For any* benchmark, the CPU instruction count and memory byte count reported must reflect only the operations performed after `env.budget().reset_default()` was called, not any setup operations.

**Validates: Requirements 2.1**

---

### Property 2: Output line format

*For any* benchmark that completes without panicking, the output must contain exactly one line matching `BENCH <name> cpu=<n> mem=<n>` where `<name>` matches the benchmark function's label.

**Validates: Requirements 2.5**

---

### Property 3: Threshold assertion completeness

*For any* benchmark that fails its CPU threshold, the output must contain a line matching `BENCH_FAIL <name>: cpu=<actual> exceeded threshold=<limit>` before the panic.

**Validates: Requirements 4.6**

---

### Property 4: Warm vs cold ordering

*For any* run of both `gas_bench_tip_cold` and `gas_bench_tip_warm`, the cold benchmark must report a CPU instruction count greater than or equal to the warm benchmark, since cold storage allocation is strictly more expensive.

**Validates: Requirements 1.1, 1.2**

---

### Property 5: Batch scaling

*For any* run of `gas_bench_tip_batch_10` and `gas_bench_tip_batch_50`, the batch-50 CPU count must be greater than the batch-10 CPU count, confirming linear scaling.

**Validates: Requirements 3.1, 3.2**

---

### Property 6: Analyzer parse correctness

*For any* set of valid `BENCH` output lines, the Analyzer must extract the same name, CPU count, and memory byte count as appear in the raw line, with no truncation or rounding.

**Validates: Requirements 6.2**

---

## Error Handling

### Benchmark file

| Condition | Behaviour |
|---|---|
| CPU count exceeds threshold | Print `BENCH_FAIL` line, then `assert!` panics and test fails |
| `env.budget()` not available | Compile error — `testutils` feature must be enabled in `[dev-dependencies]` |
| Token transfer fails in setup | Test panics with Soroban host error; indicates setup bug, not benchmark regression |

### `run_benchmarks.sh`

| Condition | Behaviour |
|---|---|
| `cargo test` exits non-zero | Print error message to stderr, exit 1 |
| No `BENCH` lines in output | Print `WARNING: no benchmark results captured`, exit 1 |
| `awk`/`sort` not available | Script fails with shell error; standard POSIX tools assumed |

### `analyze_results.py`

| Condition | Behaviour |
|---|---|
| No matching lines in input | Print warning to stderr, `sys.exit(1)` |
| Malformed `BENCH` line (partial match) | Skip line silently |
| Empty input | Treated as no matching lines |

---

## Testing Strategy

### Benchmark tests (`benches/gas_benchmarks.rs`)

Each `#[test]` function is self-validating: it asserts its own threshold (where defined) and prints its result line. Running `cargo test --package tipjar -- gas_bench --nocapture` executes all benchmarks and surfaces any threshold violations as test failures.

### CI integration

Add to the CI pipeline:

```yaml
- name: Run performance benchmarks
  run: cargo test --package tipjar -- gas_bench --nocapture
```

This catches regressions on every PR. No separate benchmark runner or nightly job is required.

### Script validation

`run_benchmarks.sh` can be validated locally by running it and checking that:
- The exit code is 0 when all benchmarks pass
- The formatted table contains one row per benchmark
- The exit code is 1 when a threshold is violated

`analyze_results.py` can be validated by piping known `BENCH` lines and checking the table output.
