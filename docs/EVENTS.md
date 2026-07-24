# Event Schema Contract and Versioning Policy

## Event Schemas

The following events are emitted by the `tipjar` contract. Off-chain consumers (like the indexer) must adhere to these schemas.

All data payloads are serialized in Soroban as arrays (using `data_format = "vec"` in the contract definition).

### 1. `Tip`
- **Topics**: `["tip", creator: Address]`
- **Data (Vec)**: `[sender: Address, amount: i128]`
- **Semantics**: Emitted when a user tips a creator. `amount` represents the amount of tokens transferred.

### 2. `Withdraw`
- **Topics**: `["withdraw", creator: Address]`
- **Data (Vec)**: `[amount: i128, to: Address]`
- **Semantics**: Emitted when a creator (or an authorized operator) withdraws. `amount` is the amount paid out this call; `to` is the payout address funds were sent to.

*(Note: The `Withdraw` event's data payload was recently normalized to use `data_format = "vec"` for trailing-field evolution. Legacy events did not use this format.)*

### 3. `PayoutChangeProposed` / `PayoutChangeApplied` / `PayoutChangeCancelled`
- **Topics**: `["payout_change_proposed" | "payout_change_applied" | "payout_change_cancelled", creator: Address]`
- **Data (Vec)**: proposed — `[new_payout: Address, effective_ledger: u32]`; applied — `[new_payout: Address]`; cancelled — `[]`
- **Semantics**: Track a creator's timelocked payout-address change (see `docs/CONTRACT_SPEC.md`).

### 4. `OperatorAuthorized` / `OperatorRevoked`
- **Topics**: `["operator_authorized" | "operator_revoked", creator: Address, operator: Address]`
- **Data (Vec)**: authorized — `[allowance: i128, expiry_ledger: u32]`; revoked — `[]`
- **Semantics**: Track delegated withdrawal authority a creator grants to an operator address.

### 5. Admin governance: `AdminTransferProposed` / `AdminTransferAccepted` / `AdminTransferCancelled`
- **Topics**: `["admin_transfer_proposed"]` / `["admin_transfer_accepted"]` / `["admin_transfer_cancelled", admin: Address]`
- **Data (Vec)**: `[current_admin: Address, new_admin: Address]` / `[new_admin: Address]` / `[]`
- **Semantics**: Two-step admin transfer (`propose_admin` → `accept_admin`), or abandonment via `cancel_admin_transfer`. Governance only takes effect once the proposed address itself calls `accept_admin`, so a typoed address can never brick the contract.

### 6. Upgrade lifecycle: `UpgradeProposed` / `UpgradeExecuted` / `UpgradeCancelled` / `Migrated`
- **Topics**: `["upgrade_proposed", hash: BytesN<32>]` / `["upgrade_executed", hash: BytesN<32>]` / `["upgrade_cancelled", hash: BytesN<32>]` / `["migrated"]`
- **Data (Vec)**: `[unlock_ledger: u32]` / `[]` / `[]` / `[from_version: u32, to_version: u32]`
- **Semantics**: The full timelocked upgrade lifecycle (`propose_upgrade` → `execute_upgrade`/`cancel_upgrade` → `migrate`). Watchers should alert on `UpgradeProposed` and treat the timelock window as a review period — see `docs/UPGRADE_RUNBOOK.md`.

### 7. `FeeCharged`
- **Topics**: `["fee_charged", creator: Address]`
- **Data (Vec)**: `[gross: i128, fee: i128, net: i128]`
- **Semantics**: Emitted from `tip()` alongside `Tip`, but only when a nonzero protocol fee rate is configured (a `fee_bps` of 0 is a true no-op: no `FeeCharged` event, no fee storage write). `gross` is the full tipped amount (equal to `Tip.amount`), `fee` is `floor(gross * fee_bps / 10_000)`, and `net` is the amount credited to the creator's withdrawable balance. `fee + net == gross` always holds, including when `fee` floors to 0 for small `gross`. Consumers can reconstruct fee accounting from this event alone, without needing to know the fee schedule that was active at tip time.

### 8. `FeeConfigured`
- **Topics**: `["fee_configured", admin: Address]`
- **Data (Vec)**: `[bps: u32, collector: Address]`
- **Semantics**: Emitted by `set_fee()` whenever the admin changes the protocol fee rate and/or collector. `bps` is hard-capped on-chain at 1,000 (10%).

### 9. `FeeWithdraw`
- **Topics**: `["fee_withdraw", collector: Address]`
- **Data (Vec)**: `[amount: i128]`
- **Semantics**: Emitted by `withdraw_fees()` when the fee collector withdraws its accrued share of `FeeBalance`.

## Legacy Events (TipJarLegacy)

The following events are emitted by the `tipjar-legacy` contract and may not use the `data_format = "vec"` serialization pattern.

### `tip_msg`
Emitted when a tip is sent with an attached message via `tip_with_message`.
- **Topics**: `["tip_msg", creator: Address]`
- **Data**: `[sender: Address, amount: i128, message: String, metadata: Map<String, String>]`

### `delegate`
Emitted when a creator grants withdrawal authorization to a delegate.
- **Topics**: `["delegate", creator: Address]`
- **Data**: `[delegate: Address, max_amount: i128, expires_at: u64]`

### `delegate_withdraw`
Emitted when a delegate successfully withdraws on behalf of a creator.
- **Topics**: `["del_wdr", creator: Address]`
- **Data**: `[delegate: Address, amount: i128, token: Address]`

### `delegate_revoked`
Emitted when a creator revokes a delegation.
- **Topics**: `["del_rev", creator: Address]`
- **Data**: `[delegate: Address]`

### `tip_expired`
Emitted when an unclaimed time-locked tip is refunded after its expiration window.
- **Topics**: `["tip_expired", creator: Address]`
- **Data**: `[sender: Address, amount: i128, expires_at: u64, lock_id: u64]`

### `stream_created`
Emitted when a new continuous tip stream is initialized.
- **Topics**: `["strm_new", stream_id: u64]`
- **Data**: `[sender: Address, creator: Address, token: Address, total: i128, rate: i128]`

### `claim_submitted`
Emitted when an insurance claim is submitted.
- **Topics**: `["clm_sub"]`
- **Data**: `[claim_id: u64, creator: Address, token: Address, amount: i128]`

### `claim_paid`
Emitted when an insurance claim is successfully paid out.
- **Topics**: `["clm_paid"]`
- **Data**: `[claim_id: u64, amount: i128, creator: Address]`

## Versioning Convention for Evolution

To ensure backwards compatibility and smooth upgrades, the following policy applies to all event schema changes:

1. **Additive Changes**: Additive changes **must** append new fields to the end of the data payload. Off-chain consumers (e.g., the indexer) **must** tolerate trailing, unknown fields in the array.
2. **Breaking Changes**: Any breaking change (modifying existing field types, removing fields, or changing topics) **requires a new event name** (e.g., `TipV2`).
3. **Deprecation Window**: For breaking changes, a dual-emission deprecation window must be documented and implemented across contract upgrades. During this window, the contract emits both the old and the new events to allow off-chain consumers time to migrate.

## PR Review Checklist

Reviewers must verify the following before approving changes to event structures:

- [ ] Does this change modify an existing event?
  - If **Yes**, does it strictly append new fields to the end of the `data_format = "vec"` structure?
    - If it's a breaking change (removal, type change, reordering), does it create a new event (e.g., `V2`) and implement dual-emission?
- [ ] Have the golden XDR fixtures been updated in this PR? (Failing CI tests will catch this if missed).
- [ ] Does the off-chain indexer tolerate the new trailing fields or correctly parse the new event version?
