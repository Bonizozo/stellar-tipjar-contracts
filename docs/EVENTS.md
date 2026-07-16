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
- **Data (Vec)**: `[amount: i128]`
- **Semantics**: Emitted when a creator withdraws their tips. `amount` is the total withdrawn balance.

*(Additional legacy events exist in `tipjar-legacy` and follow similar patterns.)*

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
