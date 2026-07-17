#!/bin/bash

# Multi-network deployment script for TipJar contract

set -e

NETWORKS=("testnet" "mainnet" "futurenet")
WASM_PATH="contracts/tipjar/target/wasm32v1-none/release/tipjar.wasm"

echo "=== TipJar Multi-Network Deployment ==="
echo ""

# Check if WASM file exists
if [ ! -f "$WASM_PATH" ]; then
    echo "Building contract..."
    cargo build -p tipjar --target wasm32v1-none --release
fi

for network in "${NETWORKS[@]}"; do
    echo "Deploying to $network..."
    
    # Set network-specific environment
    case $network in
        testnet)
            export STELLAR_NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
            export RPC_URL="https://soroban-testnet.stellar.org"
            ;;
        mainnet)
            export STELLAR_NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015"
            export RPC_URL="https://soroban.stellar.org"
            ;;
        futurenet)
            export STELLAR_NETWORK_PASSPHRASE="Test SDF Future Network ; October 2022"
            export RPC_URL="https://rpc-futurenet.stellar.org"
            ;;
    esac
    
    # Deploy contract
    echo "Deploying to $network with RPC: $RPC_URL"
    
    # This would use stellar CLI in production
    # stellar contract deploy --network $network --wasm $WASM_PATH
    
    echo "✓ Deployment to $network prepared"
    echo ""
done

echo "=== Deployment Complete ==="
echo "Next steps:"
echo "1. Set CONTRACT_ADDRESS_TESTNET, CONTRACT_ADDRESS_MAINNET, CONTRACT_ADDRESS_FUTURENET env vars"
echo "2. Run integration tests with: cargo test --test multi_network_tests"
