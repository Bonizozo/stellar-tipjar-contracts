# TipJar Integration Guide

## Prerequisites

- Stellar CLI (`stellar`) ≥ 21.0
- A funded Stellar account on the target network
- The deployed contract ID (see `docs/DEPLOYMENT.md`)

## TypeScript / JavaScript

A generated frontend client package is available at `packages/contract-client/`.
The package exports contract constants and a `Client` wrapper for `TipJar` calls.

```typescript
import { Client, CONTRACT_IDS } from '@tipjar/contract-client';

const client = new Client({
  contractId: CONTRACT_IDS.testnet,
  network: 'testnet',
});
```

> **Note:** the hand-written client under `sdk/typescript/` is **not**
> spec-generated and has drifted from the current contract interface. Prefer
> `@tipjar/contract-client` above for new integrations. See
> [`docs/SDK_DRIFT_AUDIT.md`](./SDK_DRIFT_AUDIT.md) for details.

Send a tip with the generated client (simulate, then sign and send):

```typescript
// Send a tip
await client.tip({
  sender: keypair.publicKey(),
  creator: 'GCREATOR...',
  token: 'CTOKEN...',
  amount: BigInt(10_000_000), // 1 XLM in stroops
});

// Send a tip with memo
await client.tipWithMemo({
  sender: keypair.publicKey(),
  creator: 'GCREATOR...',
  token: 'CTOKEN...',
  amount: BigInt(5_000_000),
  memo: 'Great video! 🎉',
});

// Query recent memo-tips
const tips = await client.getTipsWithMemos({
  creator: 'GCREATOR...',
  limit: 10,
});

// Withdraw as creator
await client.withdraw({
  creator: keypair.publicKey(),
  token: 'CTOKEN...',
});
```

## Stellar CLI

```bash
# Tip a creator
stellar contract invoke \
  --id $CONTRACT_ID \
  --source $SENDER_SECRET \
  --network testnet \
  -- tip \
  --sender $SENDER_ADDR \
  --creator $CREATOR_ADDR \
  --token $TOKEN_ADDR \
  --amount 10000000

# Tip with memo
stellar contract invoke \
  --id $CONTRACT_ID \
  --source $SENDER_SECRET \
  --network testnet \
  -- tip_with_memo \
  --sender $SENDER_ADDR \
  --creator $CREATOR_ADDR \
  --token $TOKEN_ADDR \
  --amount 5000000 \
  --memo '"Great content! 🎉"'

# Query memo-tips
stellar contract invoke \
  --id $CONTRACT_ID \
  --network testnet \
  -- get_tips_with_memos \
  --creator $CREATOR_ADDR \
  --limit 10

# Withdraw
stellar contract invoke \
  --id $CONTRACT_ID \
  --source $CREATOR_SECRET \
  --network testnet \
  -- withdraw \
  --creator $CREATOR_ADDR \
  --token $TOKEN_ADDR
```

## Subscribing to Events

Listen for on-chain events using the Stellar RPC `getEvents` endpoint:

```typescript
const events = await server.getEvents({
  startLedger: fromLedger,
  filters: [{
    type: 'contract',
    contractIds: [CONTRACT_ID],
    topics: [['tip', '*']],   // all tip events
  }],
});

for (const event of events.events) {
  const [creator] = event.topic;
  const [sender, amount] = event.value;
  console.log(`${sender} tipped ${creator} ${amount} stroops`);
}
```

## Recurring Subscriptions

```typescript
// Create a weekly subscription of 1 XLM
await client.createSubscription({
  subscriber: keypair.publicKey(),
  creator: 'GCREATOR...',
  token: 'CTOKEN...',
  amount: BigInt(10_000_000),
  intervalSeconds: BigInt(604_800), // 7 days
});

// Execute a due payment (anyone can call)
await client.executeSubscriptionPayment({
  subscriber: 'GSUB...',
  creator: 'GCREATOR...',
});
```

## Split Tips

```typescript
// Split 10 XLM: 70% to creator A, 30% to creator B
await client.tipSplit({
  sender: keypair.publicKey(),
  token: 'CTOKEN...',
  amount: BigInt(100_000_000),
  recipients: [
    { creator: 'GCREATOR_A...', percentage: 7000 },
    { creator: 'GCREATOR_B...', percentage: 3000 },
  ],
});
```

## Error Handling

All contract errors are returned as `u32` codes. Map them to `TipJarError` variants:

```typescript
try {
  await client.tip({ ... });
} catch (e) {
  if (e.code === 3) console.error('Invalid amount');
  if (e.code === 2) console.error('Token not whitelisted');
  if (e.code === 32) console.error('Memo too long (max 200 chars)');
}
```

See `docs/CONTRACT_SPEC.md` for the full error code table.

## Amount Units

All amounts are in **stroops** (the smallest Stellar unit). 1 XLM = 10 000 000 stroops.

```typescript
const ONE_XLM = BigInt(10_000_000);
```

## Security Notes

- Always call `is_paused()` before building a transaction in production UIs.
- Validate memo length client-side (≤ 200 chars) before submitting to avoid wasted fees.
- Token addresses must be whitelisted by the admin before use.
- The contract holds funds in escrow; creators must call `withdraw` to receive them.
