# TipJar Contract — API Reference

## Overview

The `TipJarContract` is a Soroban smart contract on Stellar that escrows token tips for creators, tracks balances, and supports admin-controlled emergency pausing.

---

## Types

### `TipWithMessage`

A structured payload stored when a tip is sent with an attached message.

| Field | Type | Description |
|---|---|---|
| `sender` | `Address` | The tipper's address |
| `creator` | `Address` | The recipient creator's address |
| `amount` | `i128` | Token amount transferred |
| `message` | `String` | Attached message (max 280 chars) |
| `metadata` | `Map<String, String>` | Arbitrary key-value metadata |
| `timestamp` | `u64` | Ledger timestamp at tip time |

---

### `DataKey` (Storage Keys)

| Variant | Type | Scope | Description |
|---|---|---|---|
| `Token` | `Address` | Instance | Token contract used for all tips |
| `Admin` | `Address` | Instance | Contract administrator |
| `Paused` | `bool` | Instance | Emergency pause flag |
| `CreatorBalance(Address)` | `i128` | Persistent | Withdrawable escrow balance per creator |
| `CreatorTotal(Address)` | `i128` | Persistent | Cumulative historical tips per creator |
| `CreatorMessages(Address)` | `Vec<TipWithMessage>` | Persistent | Tip messages per creator |

---

### `TipJarError`

| Code | Name | Trigger |
|---|---|---|
| `1` | `AlreadyInitialized` | `init` called more than once |
| `2` | `TokenNotInitialized` | Token not set before a tip/withdraw |
| `3` | `InvalidAmount` | `amount <= 0` |
| `4` | `NothingToWithdraw` | Creator balance is zero on `withdraw` |
| `5` | `MessageTooLong` | Message exceeds 280 characters |

---

## Functions

---

### `init`

```rust
pub fn init(env: Env, token: Address, admin: Address)
```

One-time contract initialization. Sets the token contract and administrator. Panics with `AlreadyInitialized` if called again.

**Parameters**

| Name | Type | Description |
|---|---|---|
| `token` | `Address` | The Stellar token contract address to use for all tips |
| `admin` | `Address` | The address granted admin privileges (pause/unpause) |

**Authorization** — None required.

**Errors**

| Error | Condition |
|---|---|
| `AlreadyInitialized` | Contract has already been initialized |

**Side Effects** — Stores `Token`, `Admin`, and `Paused = false` in instance storage.

---

### `tip`

```rust
pub fn tip(env: Env, sender: Address, creator: Address, amount: i128)
```

Transfers `amount` tokens from `sender` into contract escrow for `creator`. Updates both the withdrawable balance and the cumulative total for the creator.

**Parameters**

| Name | Type | Description |
|---|---|---|
| `sender` | `Address` | The tipper; must authorize this call |
| `creator` | `Address` | The tip recipient |
| `amount` | `i128` | Token amount to transfer (must be > 0) |

**Authorization** — `sender.require_auth()`

**Errors**

| Error | Condition |
|---|---|
| `InvalidAmount` | `amount <= 0` |
| Contract panics | Contract is paused |

**Events Emitted**

| Topic | Data |
|---|---|
| `("tip", creator)` | `(sender, amount)` |

---

### `tip_with_message`

```rust
pub fn tip_with_message(
    env: Env,
    sender: Address,
    creator: Address,
    amount: i128,
    message: String,
    metadata: Map<String, String>,
)
```

Same escrow and balance logic as `tip`, but also stores a `TipWithMessage` record and emits a richer event including the message and metadata.

**Parameters**

| Name | Type | Description |
|---|---|---|
| `sender` | `Address` | The tipper; must authorize this call |
| `creator` | `Address` | The tip recipient |
| `amount` | `i128` | Token amount to transfer (must be > 0) |
| `message` | `String` | Attached note (max 280 characters) |
| `metadata` | `Map<String, String>` | Arbitrary key-value pairs (e.g. platform, campaign) |

**Authorization** — `sender.require_auth()`

**Errors**

| Error | Condition |
|---|---|
| `InvalidAmount` | `amount <= 0` |
| `MessageTooLong` | `message.len() > 280` |
| Contract panics | Contract is paused |

**Events Emitted**

| Topic | Data |
|---|---|
| `("tip_msg", creator)` | `(sender, amount, message, metadata)` |

---

### `get_total_tips`

```rust
pub fn get_total_tips(env: Env, creator: Address) -> i128
```

Returns the cumulative total of all tips ever received by `creator`. This value never decreases, even after withdrawals.

**Parameters**

| Name | Type | Description |
|---|---|---|
| `creator` | `Address` | The creator to query |

**Returns** — `i128` — Total historical tips. Returns `0` if the creator has never received a tip.

---

### `get_withdrawable_balance`

```rust
pub fn get_withdrawable_balance(env: Env, creator: Address) -> i128
```

Returns the current escrowed balance available for the creator to withdraw.

**Parameters**

| Name | Type | Description |
|---|---|---|
| `creator` | `Address` | The creator to query |

**Returns** — `i128` — Withdrawable balance. Returns `0` if nothing is available.

---

### `get_messages`

```rust
pub fn get_messages(env: Env, creator: Address) -> Vec<TipWithMessage>
```

Returns all stored `TipWithMessage` records for a creator, in insertion order.

**Parameters**

| Name | Type | Description |
|---|---|---|
| `creator` | `Address` | The creator to query |

**Returns** — `Vec<TipWithMessage>` — All messages. Returns an empty vector if none exist.

---

### `withdraw`

```rust
pub fn withdraw(env: Env, creator: Address)
```

Transfers the creator's full escrowed balance from the contract to their address, then resets the balance to zero.

**Parameters**

| Name | Type | Description |
|---|---|---|
| `creator` | `Address` | The creator withdrawing funds; must authorize this call |

**Authorization** — `creator.require_auth()`

**Errors**

| Error | Condition |
|---|---|
| `NothingToWithdraw` | Creator's withdrawable balance is `0` |
| Contract panics | Contract is paused |

**Events Emitted**

| Topic | Data |
|---|---|
| `("withdraw", creator)` | `amount` |

---

### `pause`

```rust
pub fn pause(env: Env, admin: Address)
```

Halts all state-changing operations (`tip`, `tip_with_message`, `withdraw`). Admin-only.

**Parameters**

| Name | Type | Description |
|---|---|---|
| `admin` | `Address` | Must match the stored admin address |

**Authorization** — `admin.require_auth()`

**Errors** — Panics with `"Unauthorized"` if `admin` does not match the stored admin.

---

### `unpause`

```rust
pub fn unpause(env: Env, admin: Address)
```

Resumes normal contract operations after a pause. Admin-only.

**Parameters**

| Name | Type | Description |
|---|---|---|
| `admin` | `Address` | Must match the stored admin address |

**Authorization** — `admin.require_auth()`

**Errors** — Panics with `"Unauthorized"` if `admin` does not match the stored admin.

---

## Streaming Functions

### `create_stream`

```rust
pub fn create_stream(
    env: Env,
    sender: Address,
    creator: Address,
    token: Address,
    amount_per_second: i128,
    duration_seconds: u64,
) -> u64
```

Creates a new continuous tip stream. The total amount (`amount_per_second * duration_seconds`) is transferred from `sender` to the contract escrow immediately.

**Authorization** — `sender.require_auth()`

**Events Emitted**
- `("strm_new", stream_id)` with data `(sender, creator, token, total_amount, rate)`

---

### `withdraw_streamed`

```rust
pub fn withdraw_streamed(env: Env, creator: Address, stream_id: u64)
```

Withdraws all tokens that have been "unlocked" by the stream's progression since the last withdrawal.

**Authorization** — `creator.require_auth()`

---

## Insurance Functions

### `insurance_contribute`

```rust
pub fn insurance_contribute(env: Env, creator: Address, token: Address, amount: i128)
```

Creator contributes tokens to the insurance pool to gain coverage for potential failed transactions.

**Parameters**
- `amount`: Must be between `min_contribution` and `max_contribution` configured in the pool.

**Authorization** — `creator.require_auth()`

---

### `insurance_submit_claim`

```rust
pub fn insurance_submit_claim(
    env: Env,
    creator: Address,
    token: Address,
    amount: i128,
    tx_hash: BytesN<32>,
) -> u64
```

Submits a claim for insurance payout. Requires that the creator has sufficient coverage.

**Authorization** — `creator.require_auth()`

**Errors**
| Error | Condition |
|---|---|
| `NoCoverage` | Creator has no active insurance contribution or earned coverage |
| `ClaimCooldownActive` | Less than `claim_cooldown` seconds since last claim |
| `TooManyActiveClaims` | Creator already has the maximum allowed number of pending claims |

---

### `insurance_process_claims_batch`

```rust
pub fn insurance_process_claims_batch(
    env: Env,
    admin: Address,
    claim_ids: Vec<u64>,
    action: String,
)
```

Allows an admin to approve or pay multiple claims in a single transaction.

**Parameters**
- `action`: Either `"approve"` or `"pay"`.
