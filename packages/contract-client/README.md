# @tipjar/contract-client

Frontend contract client for the Stellar TipJar contract.

## Generation

Run the bindings generator after contract deployment:

```bash
bash scripts/generate-bindings.sh [network]
```

The script reads the deployed contract ID from `deployment/config.json` for the chosen network or from `CONTRACT_ID` if set.

## Installation

```bash
npm install @tipjar/contract-client
```

## Usage

```ts
import { Client, CONTRACT_IDS, Network } from '@tipjar/contract-client';

const client = new Client({
  contractId: CONTRACT_IDS.testnet,
  network: 'testnet',
});

client.connect(keypair);
await client.tip({
  sender: 'G...'
  creator: 'G...'
  amount: 1_000_000n,
});
```

## Network Constants

The package exports `TESTNET_CONTRACT_ID` and `MAINNET_CONTRACT_ID` as top-level constants.
Update them after deployment or use the generated values from `deployment/config.json` as part of your build.
