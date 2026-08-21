# @tipjar/sdk

TypeScript SDK for the [Stellar tipjar smart contract](../../contracts/tipjar).

> **⚠️ Interface drift warning — do not use for new integrations.**
>
> This client is **hand-written**, not generated from the contract's exported
> spec, and it has already drifted from the current `contracts/tipjar` source:
> `sendTip` calls `tip(sender, creator, amount)` but the contract's `tip` now
> requires a `token` argument; `withdraw` uses an older single-argument payout
> shape. Nothing in CI catches this drift.
>
> **New integrations should use the spec-generated client at
> [`packages/contract-client`](../../packages/contract-client)**, which is
> produced by `stellar contract bindings typescript` and is compared against
> the contract WASM in CI via `scripts/check-contract-client-drift.sh`.
>
> See [`docs/SDK_DRIFT_AUDIT.md`](../../docs/SDK_DRIFT_AUDIT.md) for the full
> audit and the rationale for not maintaining a second hand-written binding
> layer.

## Installation

```bash
npm install @tipjar/sdk
```

## Quick Start

```ts
import { TipJarContract } from '@tipjar/sdk';
import { Keypair } from '@stellar/stellar-sdk';

const sdk = new TipJarContract({
  contractId: 'C...',
  network: 'testnet',
});

// Provide a keypair for signing write transactions.
sdk.connect(Keypair.fromSecret(process.env.DEPLOYER_SECRET!));

// Send a tip
const tip = await sdk.sendTip({
  tipper: 'G...',
  creator: 'G...',
  amount: 1_000_000n, // in stroops
});
console.log('Tip tx:', tip.txHash);

// Withdraw
const withdrawal = await sdk.withdraw('G...');
console.log('Withdrew:', withdrawal.amount);
```

## API

### `new TipJarContract(config: SdkConfig)`

| Field | Type | Description |
|---|---|---|
| `contractId` | `string` | Deployed contract ID |
| `network` | `'testnet' \| 'mainnet'` | Target network |
| `rpcUrl` | `string?` | Override default RPC URL |

### Methods

#### `connect(keypair: Keypair): void`
Store a keypair for signing. Required before any write operation.

#### `sendTip(params: TipParams): Promise<TipResult>`
Calls `tip(sender, creator, amount)` on-chain.

#### `withdraw(creator: string): Promise<WithdrawResult>`
Calls `withdraw(creator)` — withdraws the full escrowed balance.

#### `getTotalTips(creator: string): Promise<bigint>`
Calls `get_total_tips(creator)` — returns historical tip total.

#### `getBalance(creator: string): Promise<bigint>`
Simulates `withdraw` to read the current withdrawable balance without submitting.

#### `getTipEvents(creator: string, limit?: number): Promise<TipEvent[]>`
Fetches on-chain `("tip", creator)` events. Default limit: 20.

## Error Handling

```ts
import { InvalidAmountError, TransactionFailedError, NetworkError } from '@tipjar/sdk';

try {
  await sdk.sendTip({ ... });
} catch (err) {
  if (err instanceof InvalidAmountError) { /* bad amount */ }
  if (err instanceof TransactionFailedError) { console.log(err.txHash); }
  if (err instanceof NetworkError) { console.log('retries:', err.retries); }
}
```

All async methods retry up to 3 times with exponential backoff before throwing `NetworkError`.

## Network Configuration

| Network | RPC URL |
|---|---|
| `testnet` | `https://soroban-testnet.stellar.org` |
| `mainnet` | `https://soroban.stellar.org` |

Override with `rpcUrl` in `SdkConfig`.

## Wallet Integration

For browser wallets (e.g. Freighter), build and sign the transaction externally, then pass the signed XDR to the Soroban RPC server directly. The `connect()` method accepts any `Keypair`-compatible signer; adapt it to your wallet's signing interface as needed.
