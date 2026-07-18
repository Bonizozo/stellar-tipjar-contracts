# Integration Tests

Live integration tests for the TipJar contract against Stellar testnet.

## Prerequisites

- Rust toolchain with the `wasm32-unknown-unknown` target:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli) on `$PATH`
- `curl` on `$PATH`
- Internet access to Stellar testnet:
  - RPC: `https://soroban-testnet.stellar.org`
  - Friendbot: `https://friendbot.stellar.org`

## How to Run

```bash
bash scripts/run_integration_tests.sh
```

This script:
1. Compiles the contract to WASM (`target/wasm32-unknown-unknown/release/tipjar.wasm`)
2. Runs all tests in `tests/integration/tip_flow_test.rs` sequentially against testnet

Tests are run with `--test-threads=1` to avoid race conditions when creating
testnet accounts via Friendbot.

## Friendbot & Testnet Availability

Each test creates two ephemeral keypairs and funds them via Friendbot
(`https://friendbot.stellar.org?addr=<public_key>`).  Friendbot funding may
take 1–2 seconds to appear on-chain; the helpers already wait for this.

Tests will fail if the Stellar testnet or Friendbot is unavailable.

## Cleanup

After each test, both ephemeral accounts are merged into a sink account
(`SINK_ACCOUNT` in `helpers.rs`) to avoid testnet pollution.  Update
`SINK_ACCOUNT` to a real funded testnet address before running.

## Contract ABI Warnings

Several tests call contract functions (`tip`, `withdraw`,
`get_withdrawable_balance`, `get_total_tips`) that are referenced in the
existing unit-test suite but are **not yet implemented** in the contract source.
Each such test is annotated with an `⚠️ ABI:` comment.  Verify all function
names against the compiled contract ABI before running.
