# Stellar Tip Jar Contracts

Rust + Soroban starter repository for a Stellar-based tipping application.

The `tipjar` smart contract lets supporters tip creators with a Stellar token, tracks creator totals in contract storage, emits on-chain events, and supports creator withdrawals from escrowed balances.

## Repository Structure

```text
contracts/
	tipjar/
		src/
			lib.rs
		Cargo.toml

tests/
scripts/

README.md
CONTRIBUTING.md
```

## Contract Capabilities

- `init(token)`: one-time token configuration.
- `tip(sender, creator, amount)`: transfers tokens from sender into contract escrow for creator and updates storage totals.
- `get_total_tips(creator)`: returns total historical tips for creator.
- `withdraw(creator)`: allows creator to withdraw escrowed tips.

## Storage Model

The contract stores:

- token contract address (`DataKey::Token`)
- creator withdrawable balance (`DataKey::CreatorBalance`)
- creator total historical tips (`DataKey::CreatorTotal`)

## Events

- `("tip", creator)` with data `(sender, amount)`
- `("withdraw", creator)` with data `amount`

## Prerequisites

- Rust toolchain
- Stellar CLI (`stellar`)
- Soroban target support for WASM:

```bash
rustup target add wasm32v1-none
```

## Build

```bash
cargo build -p tipjar --target wasm32v1-none --release
```

## Test

```bash
cargo test -p tipjar
```

Unit tests use Soroban's test framework and cover:

- tipping flow
- total and withdrawable balance tracking
- invalid tip amount rejection

## Deploy (Testnet)

Use the helper script:

```bash
bash scripts/deploy.sh
```

Or run commands manually with `stellar contract deploy` and `stellar contract invoke`.

## Frontend Contract Client

A generated TypeScript contract client lives in `packages/contract-client/`.
Run the generator after deployment:

```bash
bash scripts/generate-bindings.sh [network]
```

The package exports `TESTNET_CONTRACT_ID` and `MAINNET_CONTRACT_ID` from `deployment/config.json`.

## Contributing

See `CONTRIBUTING.md` for:

- branching strategy
- coding standards
- test requirements
- pull request checklist

## License

MIT
