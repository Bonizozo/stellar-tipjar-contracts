# Stellar Tip Jar Contracts

Rust + Soroban smart contract for a Stellar-based tipping application.
Supporters tip creators with a Stellar token (SEP-41 interface). Tipped
tokens are escrowed in the contract per creator; the contract tracks each
creator's withdrawable balance and historical total, emits events on every
tip and withdrawal, and lets creators withdraw their escrowed balance.

## Related Repositories

This contract is one part of the Stellar Tip Jar project:

- [stellar-tipjar-backend](https://github.com/Bonizozo/stellar-tipjar-backend) — REST API that integrates with this contract on-chain
- [stellar-tipjar-frontend](https://github.com/Bonizozo/stellar-tipjar-frontend) — Next.js web app for creators and supporters

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
>
> **`contracts/tipjar-legacy/` is excluded from the default workspace build.**
> Running `cargo build`, `cargo check`, `cargo clippy`, or `cargo test` at the
> workspace root never compiles it — it has its own `[workspace]` table in
> `contracts/tipjar-legacy/Cargo.toml` and is listed in the root `exclude`
> array. `simulator` and `tools/gas-estimator` still depend on it via direct
> path dependencies and are built/tested in a separate CI job. See
> [CONTRIBUTING.md](CONTRIBUTING.md) for how to build or test it explicitly.

## Contract Capabilities

- `init(env, token: Address, admin: Address)` — one-time configuration of the SEP-41 token this jar accepts and its admin; errors if called twice.
- `tip(env, sender: Address, creator: Address, amount: i128)` — escrows `amount` of the configured token from `sender` for `creator`, less the protocol fee (if any is configured); the creator's balance is credited with `amount - fee`.
- `get_total_tips(env, creator: Address) -> i128` — returns a creator's historical gross total tips (0 if never tipped).
- `withdraw(env, caller: Address, creator: Address, to: Address, amount: Option<i128>)` — pays out a creator's full or partial withdrawable balance to their payout address.
- `set_fee(env, caller: Address, bps: u32)` — admin-only; sets the protocol fee rate (hard-capped on-chain at 1,000 bps / 10%). `bps = 0` disables fees. The fee collector is deliberately NOT settable here — rotate it through the two-step `propose_fee_collector` / `accept_fee_collector` flow so a typo'd or malicious collector can't immediately and irreversibly redirect accrued protocol revenue.
- `withdraw_fees(env, caller: Address, token: Address, amount: Option<i128>)` — pays out the fee collector's full or partial share of accrued protocol fees for `token` only.
- `propose_admin` / `accept_admin` / `cancel_admin_transfer` — two-step admin handover: a proposal only takes effect once the proposed address itself accepts, so a typoed address can't brick governance.
- `propose_fee_collector` / `accept_fee_collector` / `cancel_fee_collector_transfer` — two-step fee-collector rotation, mirroring admin handover: a proposed collector only takes effect once that address itself accepts, and can be cancelled before then.

See `contracts/tipjar/src/lib.rs` for the full function list, including operator delegation and payout-address rotation.

### Payout-address delay threat model

Payout-address rotation uses an approximately one-day ledger delay before a
new payout address can receive withdrawals. This protects against an attacker
who gets a creator/operator key and tries to redirect future withdrawals to a
new destination. It does **not** lock existing escrow: an authorized creator,
or an authorized operator within its allowance and expiry, can still withdraw
immediately to the currently valid payout address while a payout-address change
is pending. Pausing and unpausing withdrawals does not change the pending
change's absolute effective ledger; after unpause, `withdraw` still applies the
change only when the ledger sequence has reached that original checkpoint.


## Storage Model

`DataKey` (instance/persistent storage):

- `Token` — the configured SEP-41 token contract address (instance storage).
- `Admin` / `PendingAdmin` — the contract admin and any address proposed as its replacement (instance storage).
- `FeeBps` / `FeeCollector` / `PendingFeeCollector` — the configured protocol fee rate, its collector, and any collector proposed as its replacement (instance storage). Absent `FeeBps` means no fee.
- `FeeBalance` — legacy unparameterized fee counter; migrated into `FeeBalanceToken(token)` for the primary token on first fee access.
- `FeeBalanceToken(Address)` — the fee collector's withdrawable accrued balance for a specific SEP-41 token (persistent storage).
- `CreatorBalance(Address)` — a creator's current withdrawable balance, net of fees (persistent storage).
- `CreatorTotal(Address)` — a creator's historical gross total ever tipped, never decreases (persistent storage).

Every write bumps the relevant ledger TTL (instance TTL on every call;
persistent TTL on the specific keys touched) so escrowed balances and totals
don't expire while still in use.

## Events

See [`docs/EVENTS.md`](docs/EVENTS.md) for the full event schema reference, including `FeeCharged`, `FeeConfigured`, `FeeWithdraw`, and the admin-transfer events.

- topics `("tip", creator: Address)`, data `(sender: Address, amount: i128)` — emitted by `tip`.
- topics `("withdraw", creator: Address)`, data `(amount: i128, to: Address)` — emitted by `withdraw`.
- topics `("fee_charged", creator: Address)`, data `(gross: i128, fee: i128, net: i128)` — emitted by `tip` alongside `Tip`, only when a nonzero fee rate is configured.

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
