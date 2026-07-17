# Multi-Network Support Documentation

## Overview

TipJar contract supports deployment and operation across three Stellar networks:
- **Testnet**: Development and testing environment
- **Mainnet**: Production environment with real value
- **Futurenet**: Experimental features and protocol testing

## Network Configuration

### Environment Variables

Set these environment variables to configure contract addresses:

```bash
# Testnet
export CONTRACT_ADDRESS_TESTNET="CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4"

# Mainnet
export CONTRACT_ADDRESS_MAINNET="CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBSC4"

# Futurenet
export CONTRACT_ADDRESS_FUTURENET="CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCSC4"
```

### Network Details

#### Testnet
- **Purpose**: Development, testing, and staging
- **RPC Endpoint**: `https://soroban-testnet.stellar.org`
- **Network Passphrase**: `Test SDF Network ; September 2015`
- **Faucet**: Available for test account funding
- **Reset Frequency**: Periodic (check Stellar documentation)

#### Mainnet
- **Purpose**: Production with real value
- **RPC Endpoint**: `https://soroban.stellar.org`
- **Network Passphrase**: `Public Global Stellar Network ; September 2015`
- **Faucet**: None (requires real XLM)
- **Stability**: High - production network

#### Futurenet
- **Purpose**: Experimental features and protocol testing
- **RPC Endpoint**: `https://rpc-futurenet.stellar.org`
- **Network Passphrase**: `Test SDF Future Network ; October 2022`
- **Faucet**: Available for test account funding
- **Stability**: Lower - experimental features

## Deployment

### Prerequisites

1. Install Stellar CLI:
```bash
curl https://stellar.org/install-cli | bash
```

2. Install Rust and Soroban target:
```bash
rustup target add wasm32v1-none
```

3. Build the contract:
```bash
cargo build -p tipjar --target wasm32v1-none --release
```

### Deploy to Testnet

```bash
stellar contract deploy \
  --network testnet \
  --wasm contracts/tipjar/target/wasm32v1-none/release/tipjar.wasm \
  --source <your-account-secret>
```

### Deploy to Mainnet

```bash
stellar contract deploy \
  --network mainnet \
  --wasm contracts/tipjar/target/wasm32v1-none/release/tipjar.wasm \
  --source <your-account-secret>
```

### Deploy to All Networks

Use the provided script:

```bash
bash scripts/deploy_all_networks.sh
```

## Network Switching

### In Code

```rust
use tipjar_sdk::Network;

// Detect current network
let network = Network::from_str("testnet")?;

// Get network-specific configuration
let rpc_url = network.rpc_url();
let passphrase = network.passphrase();
let contract_addr = get_contract_address(&network);
```

### In CLI

```bash
# Set network for stellar CLI
export STELLAR_NETWORK_PASSPHRASE="Test SDF Network ; September 2015"

# Or use network flag
stellar contract invoke --network testnet ...
```

## Testing

### Cross-Network Testing

Run tests across all networks:

```bash
cargo test --test multi_network_tests
```

### Network-Specific Tests

```bash
# Testnet only
cargo test --test multi_network_tests -- testnet

# Mainnet only
cargo test --test multi_network_tests -- mainnet

# Futurenet only
cargo test --test multi_network_tests -- futurenet
```

## Network Status Monitoring

### Health Checks

Monitor network health before operations:

```bash
# Check testnet status
curl https://soroban-testnet.stellar.org/health

# Check mainnet status
curl https://soroban.stellar.org/health

# Check futurenet status
curl https://rpc-futurenet.stellar.org/health
```

### RPC Connectivity

Verify RPC endpoint connectivity:

```bash
stellar rpc server info --network testnet
stellar rpc server info --network mainnet
stellar rpc server info --network futurenet
```

## Best Practices

### 1. Network Isolation

- Keep testnet and mainnet credentials separate
- Use different admin accounts for each network
- Never use mainnet keys in development

### 2. Staged Deployment

1. Deploy to testnet first
2. Run full test suite
3. Deploy to futurenet for experimental features
4. Deploy to mainnet only after validation

### 3. Contract Address Management

- Store contract addresses in configuration files
- Use environment variables for sensitive deployments
- Document contract addresses for each network

### 4. Token Whitelisting

Maintain separate token whitelists per network:

```bash
# Testnet tokens
TESTNET_USDC="CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4"

# Mainnet tokens
MAINNET_USDC="CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBSC4"
```

### 5. Monitoring

- Monitor contract events on each network
- Set up alerts for unusual activity
- Track gas costs per network
- Monitor balance changes

## Troubleshooting

### Network Connection Issues

```bash
# Test RPC connectivity
curl -X POST https://soroban-testnet.stellar.org \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getNetwork","params":[]}'
```

### Contract Not Found

Verify contract address:
```bash
stellar contract info --network testnet --id <contract-address>
```

### Transaction Failures

Check network status and account balance:
```bash
stellar account info --network testnet --account <account-id>
```

## Network Differences

| Feature | Testnet | Mainnet | Futurenet |
|---------|---------|---------|-----------|
| Real Value | No | Yes | No |
| Faucet | Yes | No | Yes |
| Stability | Medium | High | Low |
| Reset Frequency | Periodic | Never | Periodic |
| Experimental Features | Limited | No | Yes |
| Production Use | No | Yes | No |

## Migration Guide

### From Testnet to Mainnet

1. Verify contract on testnet
2. Deploy to mainnet
3. Update contract addresses in configuration
4. Update token whitelists for mainnet tokens
5. Migrate user data if needed
6. Update frontend to use mainnet contract

### Rollback Procedure

If issues occur on mainnet:

1. Pause contract using emergency pause
2. Investigate issue
3. Deploy fix to testnet
4. Test thoroughly
5. Deploy to mainnet
6. Resume contract

## Support

For network-related issues:
- Check [Stellar documentation](https://developers.stellar.org)
- Review [Soroban documentation](https://soroban.stellar.org)
- Open an issue on the repository
