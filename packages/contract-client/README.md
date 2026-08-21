# @tipjar/contract-client

Frontend TypeScript client for the Stellar TipJar contract. `src/generated.ts`
is vendored output from `stellar contract bindings typescript` (regenerate it
with `scripts/generate-bindings.sh`); `src/networks.ts` and `src/index.ts` are
hand-written and re-export it alongside the deployed contract ID constants.

## Regenerating bindings

After deploying (or redeploying) the contract:

```bash
bash scripts/generate-bindings.sh [network]   # defaults to "testnet"
```

This reads the contract ID for `[network]` from `deployment/config.json` and
overwrites `src/generated.ts`.

For a local contract build, generate from the exact WASM artifact instead:

```bash
cargo build -p tipjar --target wasm32v1-none --release
TIPJAR_WASM=target/wasm32v1-none/release/tipjar.wasm \
  bash scripts/generate-bindings.sh
```

CI compares the generated `Client` declaration against this vendored output
with `scripts/check-contract-client-drift.sh`.

## Building

```bash
npm install
npm run build
```

## Usage

```ts
import { Client, TESTNET_CONTRACT_ID } from '@tipjar/contract-client';

const client = new Client({
  contractId: TESTNET_CONTRACT_ID,
  networkPassphrase: 'Test SDF Network ; September 2015',
  rpcUrl: 'https://soroban-testnet.stellar.org',
  publicKey: senderAddress,
});

// Tip a creator (simulate, then sign and send)
const tipTx = await client.tip({ sender: senderAddress, creator: creatorAddress, amount: 10_0000000n });
await tipTx.signAndSend({ signTransaction: wallet.signTransaction });

// Withdraw escrowed tips
const withdrawTx = await client.withdraw({ creator: creatorAddress });
await withdrawTx.signAndSend({ signTransaction: wallet.signTransaction });

// Read-only: total historical tips for a creator
const totalTx = await client.get_total_tips({ creator: creatorAddress });
const total = totalTx.result; // bigint
```

## Network constants

`TESTNET_CONTRACT_ID` and `MAINNET_CONTRACT_ID` are read live from
`deployment/config.json` at build time, so they always reflect the latest
values written by `scripts/deploy.sh`.
