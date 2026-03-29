# Stellar TipJar SDK

## Install
npm install stellar-tipjar-sdk

## Usage

```ts
import { TipJarContract } from 'stellar-tipjar-sdk';

const sdk = new TipJarContract(CONTRACT_ID, 'testnet');

// Send Tip
await sdk.sendTip({
  creator: 'G...',
  amount: BigInt(1000),
  tipper: 'G...',
});

// Get Balance
const balance = await sdk.getBalance('G...');

// Withdraw
await sdk.withdraw('G...', BigInt(500));