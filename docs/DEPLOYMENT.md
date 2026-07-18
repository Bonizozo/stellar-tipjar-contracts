# TipJar Deployment Guide

## Prerequisites

```bash
# Install Stellar CLI
cargo install --locked stellar-cli --features opt

# Add WASM target
rustup target add wasm32v1-none

# Verify
stellar --version
```

## Build

```bash
cargo build -p tipjar --target wasm32v1-none --release
```

The compiled WASM is at `target/wasm32v1-none/release/tipjar.wasm`.

## Testnet Deployment

```bash
# 1. Generate and fund a deployer account
stellar keys generate deployer --network testnet
stellar keys fund deployer --network testnet

# 2. Deploy the contract
CONTRACT_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/tipjar.wasm \
  --source deployer \
  --network testnet)
echo "Contract ID: $CONTRACT_ID"

# 3. Initialize (fee = 1%, refund window = 7 days)
stellar contract invoke \
  --id $CONTRACT_ID \
  --source deployer \
  --network testnet \
  -- init \
  --admin $(stellar keys address deployer) \
  --fee_basis_points 100 \
  --refund_window_seconds 604800

# 4. Whitelist a token (e.g. native XLM)
XLM_TOKEN=$(stellar contract id asset \
  --asset native \
  --network testnet)

stellar contract invoke \
  --id $CONTRACT_ID \
  --source deployer \
  --network testnet \
  -- add_token \
  --admin $(stellar keys address deployer) \
  --token $XLM_TOKEN
```

Or use the helper script:

```bash
bash scripts/deploy_testnet.sh
```

## Mainnet Deployment

```bash
bash scripts/deploy_mainnet.sh
```

Review `scripts/deploy_mainnet.sh` before running — it requires a funded mainnet account and prompts for confirmation.

## Verify Deployment

```bash
# Check contract is initialized
stellar contract invoke \
  --id $CONTRACT_ID \
  --network testnet \
  -- is_paused

# Check version
stellar contract invoke \
  --id $CONTRACT_ID \
  --network testnet \
  -- get_version
```

## Upgrading

```bash
# Build new WASM
cargo build -p tipjar --target wasm32v1-none --release

# Upload new WASM and get its hash
NEW_HASH=$(stellar contract upload \
  --wasm target/wasm32v1-none/release/tipjar.wasm \
  --source deployer \
  --network testnet)

# Upgrade (admin only)
stellar contract invoke \
  --id $CONTRACT_ID \
  --source deployer \
  --network testnet \
  -- upgrade \
  --new_wasm_hash $NEW_HASH
```

The contract version is incremented automatically. All storage is preserved.

## Emergency Pause

```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --source deployer \
  --network testnet \
  -- pause \
  --admin $(stellar keys address deployer) \
  --reason '"Emergency maintenance"'

# Resume
stellar contract invoke \
  --id $CONTRACT_ID \
  --source deployer \
  --network testnet \
  -- unpause \
  --admin $(stellar keys address deployer)
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `CONTRACT_ID` | Deployed contract address |
| `ADMIN_SECRET` | Admin account secret key |
| `TOKEN_ADDR` | Whitelisted token contract address |
| `STELLAR_NETWORK` | `testnet` or `mainnet` |
| `STELLAR_RPC_URL` | Soroban RPC endpoint |

## Network RPC Endpoints

| Network | RPC URL |
|---------|---------|
| Testnet | `https://soroban-testnet.stellar.org` |
| Mainnet | `https://mainnet.stellar.validationcloud.io/v1/<key>` |
| Futurenet | `https://rpc-futurenet.stellar.org` |

## Running Tests

```bash
# Unit + integration tests
cargo test -p tipjar

# Property tests
cargo test --test property_tests -p tipjar

# QuickCheck tests
cargo test --test quickcheck_properties -p tipjar
```
