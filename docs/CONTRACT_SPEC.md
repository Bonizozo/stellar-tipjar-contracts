# TipJar Contract Specification

## Overview

The TipJar contract lets supporters tip creators using whitelisted Stellar tokens. Tips are held in escrow and can be withdrawn by the creator at any time. The contract supports optional memos, recurring subscriptions, split tips, conditional execution, and a leaderboard.

## Public Functions

### Initialization

#### `init(admin: Address, fee_basis_points: u32, refund_window_seconds: u64)`
One-time setup. Stores the admin address, platform fee, and refund window.

- Panics: `AlreadyInitialized` if called more than once.
- Panics: `FeeExceedsMaximum` if `fee_basis_points > 500` (5%).

#### `get_refund_window() -> u64`
Returns the current tip refund/expiration window in seconds.

#### `set_refund_window(admin: Address, refund_window_seconds: u64)`
Updates the refund window used by time-locked tips. Admin only.

#### `process_expired_tips() -> u32`
Scans active time-locked tips and refunds those whose expiry window has passed. Returns the number of refunds processed.

- Emits: `("tip_expired", creator)` for each refunded tip.

#### `get_expired_time_locks() -> Vec<TipWithExpiry>`
Returns the list of active, expired time-locked tips with expiry metadata.

#### `add_token(admin: Address, token: Address)`
Whitelists a token for use in tips. Admin only.

- Panics: `Unauthorized` if caller is not the stored admin.

### Delegation

#### `delegate_withdrawal(creator: Address, delegate: Address, max_amount: i128, duration: u64)`
Authorizes a delegate to withdraw up to `max_amount` on behalf of `creator` for `duration` seconds.

- Requires auth from `creator`.
- Panics: `InvalidAmount` if `max_amount <= 0`.
- Panics: `InvalidDuration` if `duration == 0`.
- Emits: `("delegate",)` → `(creator, delegate, max_amount, expires_at)`.

#### `withdraw_as_delegate(delegate: Address, creator: Address, token: Address, amount: i128)`
Lets an authorized delegate withdraw `amount` from the creator's balance.

- Requires auth from `delegate`.
- Panics: `DelegationNotFound` if no delegation exists.
- Panics: `DelegationInactive` if the delegation was revoked or already used.
- Panics: `DelegationExpired` if the delegation has passed its expiration time.
- Panics: `DelegationLimitExceeded` if the withdrawal exceeds the authorized cap.
- Panics: `NothingToWithdraw` if the creator has no available balance.
- Emits: `("delegate_withdraw",)` → `(creator, delegate, amount, token)`.

#### `revoke_delegation(creator: Address, delegate: Address)`
Revokes an active delegation. Only the creator may revoke.

- Requires auth from `creator`.
- Panics: `DelegationNotFound` if no delegation exists.
- Panics: `DelegationInactive` if the delegation is already inactive.
- Emits: `("delegate_revoked",)` → `(creator, delegate)`.

#### `get_delegation(creator: Address, delegate: Address) -> Option<Delegation>`
Returns the active delegation record.

#### `get_delegates(creator: Address) -> Vec<Address>`
Returns the currently active delegate addresses.

#### `get_delegation_history(creator: Address) -> Vec<Delegation>`
Returns all delegation state snapshots for the creator.

### Tipping

#### `tip(sender: Address, creator: Address, token: Address, amount: i128) -> u64`
Transfers `amount` of `token` from `sender` into escrow for `creator`. Returns the tip ID.

- Requires auth from `sender`.
- Panics: `InvalidAmount` if `amount <= 0`.
- Panics: `TokenNotWhitelisted` if the token is not approved.
- Emits: `("tip", creator)` → `(sender, amount)`.

#### `tip_with_memo(sender, creator, token, amount, memo: Option<String>)`
Like `tip` but stores an optional memo (max 200 UTF-8 characters) on-chain alongside a timestamp. Uses the `CreatorStats` optimization (single storage read/write per call).

- Panics: `MemoTooLong` if memo exceeds 200 characters.
- Emits: `("tip_memo", creator)` → `(sender, amount)`.

#### `tip_with_fee(sender, creator, token, amount, congestion: u32)`
Deducts a dynamic platform fee before crediting the creator. `congestion`: 0=Low, 1=Normal, 2=High.

- Emits: `("tip", creator)` → `(sender, net_amount)` and `("fee", creator)` → `(fee_amount, fee_bps)`.

#### `tip_split(sender, token, recipients: Vec<TipRecipient>, amount)`
Splits a tip among 2–10 recipients. Each recipient's `percentage` is in basis points; all must sum to 10 000.

- Panics: `InvalidRecipientCount`, `InvalidPercentage`, `InvalidPercentageSum`.
- Emits: `("tip_splt", creator)` → `(sender, share, percentage)` per recipient.

#### `execute_conditional_tip(sender, creator, token, amount, conditions) -> bool`
Executes a tip only if all conditions evaluate to true. Returns `false` (no transfer) when conditions fail.

### Querying

#### `get_withdrawable_balance(creator: Address, token: Address) -> i128`
Returns the creator's current escrowed balance.

#### `get_total_tips(creator: Address, token: Address) -> i128`
Returns the historical total tips received by the creator.

#### `get_tips_with_memos(creator: Address, limit: u32) -> Vec<TipWithMemo>`
Returns the most recent `limit` memo-tips (capped at 50) for the creator, oldest first.

#### `get_leaderboard(period: TimePeriod, kind: ParticipantKind, limit: u32) -> Vec<LeaderboardEntry>`
Returns the top `limit` (max 100) tippers or creators sorted by total amount descending.

### Withdrawal

#### `withdraw(creator: Address, token: Address)`
Transfers the full escrowed balance to the creator.

- Requires auth from `creator`.
- Panics: `NothingToWithdraw` if balance is zero.
- Emits: `("withdraw", creator)` → `amount`.

### Subscriptions

#### `create_subscription(subscriber, creator, token, amount, interval_seconds)`
Creates a recurring tip. Minimum interval: 86 400 s (1 day).

#### `execute_subscription_payment(subscriber, creator)`
Executes a due payment. Anyone may call this.

#### `pause_subscription / resume_subscription / cancel_subscription`
Subscriber-only lifecycle management.

#### `get_subscription(subscriber, creator) -> Option<Subscription>`

### Streaming


#### `create_stream(sender, creator, token, rate_per_second, duration_seconds) -> u64`
Creates a continuous tip stream where tokens are released over time. Tokens are escrowed upfront.

- Requires auth from `sender`.
- Emits: `("stream_created",)` -> `(stream_id, sender, creator, amount, rate)`.

#### `start_stream(sender, stream_id)` / `stop_stream(sender, stream_id)`
Resumes or pauses an existing stream. Only the sender can control the stream state.

#### `withdraw_streamed(creator, stream_id)`
Withdraws all currently unlocked tokens from the stream.

- Requires auth from `creator`.
- Emits: `("stream_withdrawn",)` -> `(stream_id, amount)`.

#### `cancel_stream(sender, stream_id)`
Permanently stops a stream and refunds remaining escrowed tokens to the sender.

- Requires auth from `sender`.
- Emits: `("stream_cancelled",)` -> `(stream_id, refunded_amount)`.

#### `get_streamed_amount(stream_id) -> i128`
Returns total amount released by the stream so far.

#### `get_available_to_withdraw(stream_id) -> i128`
Returns amount available for the creator to withdraw.

### Insurance

#### `insurance_set_config(admin, min_contrib, max_contrib, premium_rate, payout_ratio, cooldown, admin_fee, tip_premium)`
Sets global insurance parameters. Admin only.

#### `insurance_contribute(creator, token, amount)`
Allows a creator to contribute tokens to the insurance pool to gain coverage.

- Requires auth from `creator`.
- Emits: `("insurance_contribution",)` -> `(creator, token, amount, pool_reserves)`.

#### `insurance_submit_claim(creator, token, amount, tx_hash) -> u64`
Submits a claim for a failed transaction. Requires existing coverage.

- Requires auth from `creator`.
- Panics: `NoCoverage` if creator has no active insurance.
- Panics: `ClaimCooldownActive` if submitted too soon after last claim.
- Emits: `("claim_submitted",)` -> `(claim_id, creator, token, amount)`.

#### `insurance_approve_claim(admin, claim_id)` / `insurance_reject_claim(admin, claim_id)`
Admin review of submitted claims.

#### `insurance_pay_claim(admin, claim_id)`
Payout of an approved claim from the insurance pool to the creator.

- Emits: `("claim_paid",)` -> `(claim_id, amount, creator)`.

#### `insurance_process_claims_batch(admin, claim_ids, action)`
Batch process multiple claims (approve or pay) for efficiency.

#### `insurance_get_coverage(creator, token) -> i128`
Returns current maximum payout available for a creator based on contributions and tips received.

### Administration

#### `pause(admin, reason: String)` / `unpause(admin)`
Halts / resumes all state-changing operations.

#### `is_paused() -> bool`

#### `upgrade(new_wasm_hash: BytesN<32>)`
Upgrades the contract WASM. Admin only. Increments the on-chain version.

#### `get_version() -> u32`

## Storage Layout

| Key | Type | Description |
|-----|------|-------------|
| `Admin` | `Address` | Contract administrator |
| `TokenWhitelist(token)` | `bool` | Whether a token is approved |
| `CreatorBalance(creator, token)` | `i128` | Withdrawable escrow balance |
| `CreatorTotal(creator, token)` | `i128` | Historical total tips |
| `CreatorStats(creator, token)` | `CreatorStats` | Combined balance+total (optimized) |
| `TipCount(creator)` | `u64` | Number of memo-tips stored |
| `TipData(creator, index)` | `TipWithMemo` | Individual memo-tip record |
| `Subscription(subscriber, creator)` | `Subscription` | Recurring tip state |
| `Paused` | `bool` | Emergency pause flag |
| `PauseReason` | `String` | Human-readable pause reason |
| `Delegation(creator, delegate)` | `Delegation` | Active delegation record |
| `Delegates(creator)` | `Vec<Address>` | Active delegate list |
| `DelegationHistory(creator)` | `Vec<Delegation>` | Delegation history snapshots |
| `ContractVersion` | `u32` | Incremented on each upgrade |

## Error Codes

| Code | Name | Description |
|------|------|-------------|
| 1 | `AlreadyInitialized` | `init` called more than once |
| 2 | `TokenNotWhitelisted` | Token not approved for tips |
| 3 | `InvalidAmount` | Amount must be positive |
| 4 | `NothingToWithdraw` | Creator balance is zero |
| 5 | `MessageTooLong` | Tip message exceeds limit |
| 9 | `Unauthorized` | Caller is not the admin |
| 24 | `SubscriptionNotFound` | No subscription exists |
| 25 | `SubscriptionNotActive` | Subscription is paused or cancelled |
| 26 | `PaymentNotDue` | Interval has not elapsed |
| 27 | `InvalidInterval` | Interval below 1-day minimum |
| 28 | `InvalidRecipientCount` | Must have 2–10 recipients |
| 29 | `InvalidPercentageSum` | Shares must sum to 10 000 bps |
| 30 | `InvalidPercentage` | Individual share is zero |
| 31 | `ContractPaused` | Contract is paused |
| 32 | `MemoTooLong` | Memo exceeds 200 characters |
| 44 | `DelegationNotFound` | No delegation exists for this creator/delegate pair |
| 45 | `DelegationExpired` | Delegation has expired |
| 46 | `DelegationInactive` | Delegation has been revoked or deactivated |
| 47 | `DelegationLimitExceeded` | Requested delegate withdrawal exceeds allowed limit |
| 48 | `InvalidDuration` | Delegation duration must be greater than zero |

## Data Structures

```rust
pub struct TipWithMemo {
    pub sender: Address,
    pub amount: i128,
    pub memo: Option<String>,  // max 200 UTF-8 chars
    pub timestamp: u64,        // ledger timestamp
}

pub struct Delegation {
    pub creator: Address,
    pub delegate: Address,
    pub max_amount: i128,
    pub used_amount: i128,
    pub expires_at: u64,
    pub active: bool,
}

pub struct CreatorStats {
    pub balance: i128,  // withdrawable
    pub total: i128,    // historical
}

pub struct Subscription {
    pub subscriber: Address,
    pub creator: Address,
    pub token: Address,
    pub amount: i128,
    pub interval_seconds: u64,
    pub last_payment: u64,
    pub next_payment: u64,
    pub status: SubscriptionStatus,  // Active | Paused | Cancelled
}

pub struct TipRecipient {
    pub creator: Address,
    pub percentage: u32,  // basis points, must be > 0
}
```

## Events

| Topic | Data | Emitted by |
|-------|------|-----------|
| `("tip", creator)` | `(sender, amount)` | `tip` |
| `("tip_memo", creator)` | `(sender, amount)` | `tip_with_memo` |
| `("tip_splt", creator)` | `(sender, share, pct)` | `tip_split` |
| `("withdraw", creator)` | `amount` | `withdraw` |
| `("sub_new", creator)` | `(subscriber, amount, interval)` | `create_subscription` |
| `("sub_pay", creator)` | `(subscriber, amount)` | `execute_subscription_payment` |
| `("paused",)` | `(admin, reason)` | `pause` |
| `("unpaused",)` | `admin` | `unpause` |
| `("upgraded",)` | `version` | `upgrade` |
