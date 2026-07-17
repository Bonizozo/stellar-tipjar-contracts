# TipJar Contract — Events Reference

All events are emitted via `env.events().publish(topics, data)` and are queryable from Stellar's event streaming API.

---

## `tip`

Emitted when a plain tip is successfully transferred into escrow.

**Topics**

| Position | Value | Type |
|---|---|---|
| 0 | `"tip"` | `Symbol` |
| 1 | `creator` | `Address` |

**Data**

| Field | Type | Description |
|---|---|---|
| `sender` | `Address` | The address that sent the tip |
| `amount` | `i128` | Token amount transferred |

**Example (conceptual)**
```
topics: ["tip", "GCREATOR..."]
data:   ["GSENDER...", 250]
```

---

## `tip_msg`

Emitted when a tip is sent with an attached message via `tip_with_message`.

**Topics**

| Position | Value | Type |
|---|---|---|
| 0 | `"tip_msg"` | `Symbol` |
| 1 | `creator` | `Address` |

**Data**

| Field | Type | Description |
|---|---|---|
| `sender` | `Address` | The address that sent the tip |
| `amount` | `i128` | Token amount transferred |
| `message` | `String` | The attached message text |
| `metadata` | `Map<String, String>` | Arbitrary key-value metadata |

**Example (conceptual)**
```
topics: ["tip_msg", "GCREATOR..."]
data:   ["GSENDER...", 500, "Great content!", {"platform": "web"}]
```

---

## `withdraw`

Emitted when a creator successfully withdraws their escrowed balance.

**Topics**

| Position | Value | Type |
|---|---|---|
| 0 | `"withdraw"` | `Symbol` |
| 1 | `creator` | `Address` |

**Data**

| Field | Type | Description |
|---|---|---|
| `amount` | `i128` | Total amount withdrawn |

**Example (conceptual)**
```
topics: ["withdraw", "GCREATOR..."]
data:   400
```

---

## `delegate`

Emitted when a creator grants withdrawal authorization to a delegate.

**Topics**

| Position | Value | Type |
|---|---|---|
| 0 | `"delegate"` | `Symbol` |
| 1 | `creator` | `Address` |

**Data**

| Field | Type | Description |
|---|---|---|
| `delegate` | `Address` | The authorized delegate address |
| `max_amount` | `i128` | Lifetime withdrawal cap for the delegation |
| `expires_at` | `u64` | Expiry timestamp for the delegation |

---

## `delegate_withdraw`

Emitted when a delegate successfully withdraws on behalf of a creator.

**Topics**

| Position | Value | Type |
|---|---|---|
| 0 | `"del_wdr"` | `Symbol` |
| 1 | `creator` | `Address` |

**Data**

| Field | Type | Description |
|---|---|---|
| `delegate` | `Address` | The delegate that withdrew funds |
| `amount` | `i128` | Amount withdrawn |
| `token` | `Address` | The token asset withdrawn |

---

## `delegate_revoked`

Emitted when a creator revokes a delegation.

**Topics**

| Position | Value | Type |
|---|---|---|
| 0 | `"del_rev"` | `Symbol` |
| 1 | `creator` | `Address` |

**Data**

| Field | Type | Description |
|---|---|---|
| `delegate` | `Address` | The delegate that was revoked |

---

## `tip_expired`

Emitted when an unclaimed time-locked tip is refunded after its expiration window.

**Topics**

| Position | Value | Type |
|---|---|---|
| 0 | `"tip_expired"` | `Symbol` |
| 1 | `creator` | `Address` |

**Data**

| Field | Type | Description |
|---|---|---|
| `sender` | `Address` | The original tipper's address |
| `amount` | `i128` | Refunded token amount |
| `expires_at` | `u64` | Expiry timestamp that triggered the refund |
| `lock_id` | `u64` | Identifier for the refunded time-lock |

---

## `stream_created`

Emitted when a new continuous tip stream is initialized.

**Topics**

| Position | Value | Type |
|---|---|---|
| 0 | `"strm_new"` | `Symbol` |
| 1 | `stream_id` | `u64` |

**Data**

| Field | Type | Description |
|---|---|---|
| `sender` | `Address` | The address that created the stream |
| `creator` | `Address` | The stream recipient |
| `token` | `Address` | The token asset being streamed |
| `total` | `i128` | Total amount escrowed for the stream |
| `rate` | `i128` | Stream rate in tokens per second |

---

## `claim_submitted`

Emitted when an insurance claim is submitted.

**Topics**

| Position | Value | Type |
|---|---|---|
| 0 | `"clm_sub"` | `Symbol` |

**Data**

| Field | Type | Description |
|---|---|---|
| `claim_id` | `u64` | Global identifier for the claim |
| `creator` | `Address` | The creator submitting the claim |
| `token` | `Address` | The token the claim is for |
| `amount` | `i128` | Requested payout amount |

---

## `claim_paid`

Emitted when an insurance claim is successfully paid out.

**Topics**

| Position | Value | Type |
|---|---|---|
| 0 | `"clm_paid"` | `Symbol` |

**Data**

| Field | Type | Description |
|---|---|---|
| `claim_id` | `u64` | Global identifier for the paid claim |
| `amount` | `i128` | Total amount paid out to the creator |
| `creator` | `Address` | Recipient of the payout |

---

## Querying Events

Use the Stellar CLI to stream or query events from a deployed contract:

```bash
stellar events \
  --network testnet \
  --contract-id <CONTRACT_ID> \
  --start-ledger <LEDGER>
```
