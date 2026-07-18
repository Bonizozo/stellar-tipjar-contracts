# Wallet Integration

## Overview

The TipJar contract requires transaction signing for all state-changing calls (`tip`, `withdraw`, `pause`, etc.). Signing can be done with:

1. **Keypair** — a raw Stellar secret key (server-side or CLI use).
2. **Freighter** — browser extension wallet (frontend use).
3. **WalletConnect** — mobile wallet support.

---

## Keypair (Server / CLI)

```typescript
import { Keypair } from '@stellar/stellar-sdk';
import { TipJarContract } from './sdk/typescript/src/TipJarContract';

const sdk = new TipJarContract({
  contractId: process.env.CONTRACT_ID!,
  network: 'testnet',
});

sdk.connect(Keypair.fromSecret(process.env.STELLAR_SECRET!));

const result = await sdk.sendTip({
  creator: 'GCREATOR...',
  amount: 10_000_000n,
  tipper: Keypair.fromSecret(process.env.STELLAR_SECRET!).publicKey(),
});
```

---

## Freighter (Browser)

Install the [Freighter](https://www.freighter.app/) browser extension.

```typescript
import {
  isConnected,
  getPublicKey,
  signTransaction,
} from '@stellar/freighter-api';

// Check connection
if (!(await isConnected())) {
  throw new Error('Freighter not installed');
}

const publicKey = await getPublicKey();

// Build the XDR transaction using the SDK, then sign with Freighter
const xdr = await sdk.buildTipTransaction({
  creator: 'GCREATOR...',
  amount: 10_000_000n,
  tipper: publicKey,
});

const signedXdr = await signTransaction(xdr, { network: 'TESTNET' });
const result = await sdk.submitSignedTransaction(signedXdr);
```

---

## Authorization Model

Every state-changing contract function calls `require_auth()` on the relevant address:

| Function | Who must authorize |
|---|---|
| `tip` | `sender` |
| `tip_with_message` | `sender` |
| `tip_batch` | `sender` |
| `tip_locked` | `tipper` |
| `withdraw` | `creator` |
| `withdraw_locked` | `creator` |
| `pause` / `unpause` | `admin` |
| `grant_role` / `revoke_role` | `admin` |
| `upgrade` | `admin` |

Soroban enforces these at the runtime level — a transaction that does not include the required auth entry will be rejected.

---

## Network Configuration

| Network | RPC URL | Network Passphrase |
|---|---|---|
| Testnet | `https://soroban-testnet.stellar.org` | `Test SDF Network ; September 2015` |
| Mainnet | `https://mainnet.stellar.validationcloud.io/v1/...` | `Public Global Stellar Network ; September 2015` |
| Futurenet | `https://rpc-futurenet.stellar.org` | `Test SDF Future Network ; October 2022` |

See `docs/NETWORKS.md` for full network details and funded account setup.

---

## Security Considerations

- Never expose secret keys in frontend code or version control.
- Use environment variables or a secrets manager for server-side keys.
- Always simulate a transaction before signing to verify the fee and outcome.
- Validate the contract ID against the known deployed address before signing.
