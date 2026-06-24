# Stellar Tip Jar Contracts

Rust + Soroban smart contract for a Stellar-based tipping application.
Supporters tip creators with a Stellar token (SEP-41 interface). Tipped
tokens are escrowed in the contract per creator; the contract tracks each
creator's withdrawable balance and historical total, emits events on every
tip and withdrawal, and lets creators withdraw their escrowed balance.

## Repository Structure

```text
contracts/
  tipjar/             # the tip jar contract (this README's focus)
    src/
      lib.rs
      test.rs
    Cargo.toml
  tipjar-legacy/       # pre-existing, unrelated contract code kept as-is (see note below)
  derivatives/
  risk-management/
  limit-orders/
  arbitrage/

packages/
  contract-client/     # generated TypeScript client + frontend wrapper

deployment/
  config.json          # source of truth for TESTNET_CONTRACT_ID / MAINNET_CONTRACT_ID

scripts/
  deploy.sh
  generate-bindings.sh

tests/
README.md
CONTRIBUTING.md
```

> **Note:** `contracts/tipjar-legacy/` holds code that previously lived at
> `contracts/tipjar/`. It implements a much larger, unrelated set of features
> under the same function names (`init`, `tip`, `withdraw`, `get_total_tips`)
> with different signatures, which made it impossible to keep both in one
> crate (Soroban contracts export functions into one flat WASM name table, so
> two contracts in the same crate can't both export a function called `tip`).
> It was relocated byte-for-byte rather than deleted. It currently does not
> compile (pre-existing, unrelated to the contract documented here).

## Contract Capabilities

- `init(env, token: Address)` — one-time configuration of the SEP-41 token this jar accepts; errors if called twice.
- `tip(env, sender: Address, creator: Address, amount: i128)` — escrows `amount` of the configured token from `sender` for `creator`.
- `get_total_tips(env, creator: Address) -> i128` — returns a creator's historical total tips (0 if never tipped).
- `withdraw(env, creator: Address)` — pays out a creator's full withdrawable balance and resets it to zero; the historical total is left untouched.

## Storage Model

`DataKey` (instance/persistent storage):

- `Token` — the configured SEP-41 token contract address (instance storage).
- `CreatorBalance(Address)` — a creator's current withdrawable balance (persistent storage).
- `CreatorTotal(Address)` — a creator's historical total ever tipped, never decreases (persistent storage).

Every write bumps the relevant ledger TTL (instance TTL on every call;
persistent TTL on the specific keys touched) so escrowed balances and totals
don't expire while still in use.

## Events

- topics `("tip", creator: Address)`, data `(sender: Address, amount: i128)` — emitted by `tip`.
- topics `("withdraw", creator: Address)`, data `amount: i128` — emitted by `withdraw`.

## Prerequisites

- Rust toolchain (stable)
- [Stellar CLI](https://developers.stellar.org/docs/tools/cli/stellar-cli) (`stellar`)
- The Soroban WASM target:

```bash
rustup target add wasm32v1-none
```

## Build

```bash
cargo build -p tipjar --target wasm32v1-none --release
```

The release profile (defined at the workspace root, since Cargo only honors
`[profile.*]` there) is tuned for WASM size: `opt-level = "z"`, `lto = true`,
`codegen-units = 1`, `panic = "abort"`, `strip = true`.

## Test

```bash
cargo test -p tipjar
```

Unit tests live in `contracts/tipjar/src/test.rs` and deploy a real Stellar
Asset Contract as the test token (via `mock_all_auths()` and
`register_stellar_asset_contract_v2`), so transfers actually move tokens.
They cover:

- tipping escrows tokens and raises both withdrawable balance and historical total
- multiple tips accumulating for the same creator
- `get_total_tips` returning 0 for an unknown creator and the correct sum after tips
- withdrawing the full escrowed balance, resetting it to zero while the total is unchanged
- rejecting zero/negative tip amounts
- rejecting a second `init` call
- rejecting `withdraw` when there is nothing to withdraw
- the exact `tip` and `withdraw` events (topics and data)

## Deploy to Testnet

```bash
bash scripts/deploy.sh [token_address]
```

This builds the release WASM, optimizes it (via `stellar contract optimize`
if available), creates and funds a deployer identity on testnet (idempotent),
deploys with `stellar contract deploy`, and records the resulting contract ID
in `deployment/config.json`. Pass a token address (or set `TOKEN_ADDRESS`) to
have it call `init` automatically; otherwise the script prints the manual
`stellar contract invoke ... -- init --token <address>` command.

Overridable env vars: `NETWORK_NAME`, `RPC_URL`, `NETWORK_PASSPHRASE`,
`DEPLOYER_IDENTITY` (all default to testnet values).

### Manual alternative

```bash
stellar keys generate tipjar-deployer --fund --network testnet
stellar contract deploy \
  --wasm target/wasm32v1-none/release/tipjar.wasm \
  --source-account tipjar-deployer \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"
```

## Frontend Client

```bash
bash scripts/generate-bindings.sh [network]   # defaults to "testnet"
```

This reads the contract ID for `[network]` from `deployment/config.json` and
runs `stellar contract bindings typescript`, vendoring the result into
`packages/contract-client/src/generated.ts`. The package's `index.ts`
re-exports the generated client plus `TESTNET_CONTRACT_ID` /
`MAINNET_CONTRACT_ID`, both read live from `deployment/config.json` — that
file is the single source of truth for which contract ID each network
points at. See `packages/contract-client/README.md` for usage.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for branching strategy, coding
standards, test requirements, and the pull request checklist.

## License

MIT
