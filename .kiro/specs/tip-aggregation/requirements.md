# Requirements Document

## Introduction

This feature introduces a **Tip Aggregation Protocol** for the TipJar Soroban smart contract. Unlike the existing `batch_tip_v2` function — which executes a fixed set of tip operations atomically in a single call — the aggregation protocol allows tips to be **queued on-chain** by one or more senders, accumulated over time, and then **settled in a single optimised transaction** by any authorised settler. The protocol calculates optimal batch sizes to maximise gas efficiency, enforces queue lifecycle rules (expiry, cancellation), applies the platform fee system consistently, and emits aggregation-specific events for off-chain indexing.

---

## Glossary

- **Aggregation_Protocol**: The on-chain subsystem described in this document that manages tip queues, batch-size calculation, and settlement.
- **Queue**: An ordered, on-chain list of pending `QueuedTip` entries associated with a specific token.
- **QueuedTip**: A single pending tip entry in a Queue, containing sender, creator, token, amount, and expiry metadata.
- **Settler**: An address authorised to trigger settlement of a Queue.
- **Settlement**: The act of executing all eligible `QueuedTip` entries in a Queue in a single transaction, transferring funds to creators and emitting events.
- **Optimal_Batch_Size**: The calculated maximum number of `QueuedTip` entries that can be settled in one transaction while remaining within Soroban's per-transaction resource limits.
- **Platform_Fee**: The existing fee in basis points deducted from each tip amount before crediting the creator, stored under `DataKey::FeeBasisPoints`.
- **Queue_ID**: A monotonically increasing `u64` identifier assigned to each Queue at creation time.
- **Expiry**: A ledger timestamp after which a `QueuedTip` is considered stale and must not be settled.
- **Circuit_Breaker**: The existing contract-level safety mechanism that halts operations when volume thresholds are exceeded.

---

## Requirements

### Requirement 1: Queue Creation

**User Story:** As a tipper, I want to create a tip queue for a specific token so that I can accumulate multiple tips before they are settled on-chain.

#### Acceptance Criteria

1. WHEN a tipper calls `create_queue` with a valid whitelisted token address, THE Aggregation_Protocol SHALL create a new Queue and assign it a unique, monotonically increasing Queue_ID.
2. WHEN a tipper calls `create_queue` with a token address that is not whitelisted, THE Aggregation_Protocol SHALL return an error and SHALL NOT create a Queue.
3. WHEN a tipper calls `create_queue` while the contract is paused, THE Aggregation_Protocol SHALL return an error and SHALL NOT create a Queue.
4. THE Aggregation_Protocol SHALL persist the Queue state in Soroban persistent storage under a key derived from the Queue_ID.
5. WHEN a Queue is created, THE Aggregation_Protocol SHALL emit a `("agg_queue_created",)` event with data `(queue_id, token, creator_address)`.

---

### Requirement 2: Queuing Tips

**User Story:** As a tipper, I want to add a tip to an existing queue so that it can be settled later in a gas-efficient batch.

#### Acceptance Criteria

1. WHEN a tipper calls `queue_tip` with a valid Queue_ID, a positive amount, a valid creator address, and an expiry timestamp in the future, THE Aggregation_Protocol SHALL append a `QueuedTip` entry to the Queue and SHALL transfer the tip amount from the tipper to the contract.
2. WHEN a tipper calls `queue_tip` with an amount less than or equal to zero, THE Aggregation_Protocol SHALL return an error and SHALL NOT modify the Queue.
3. WHEN a tipper calls `queue_tip` with an expiry timestamp that is not greater than the current ledger timestamp, THE Aggregation_Protocol SHALL return an error and SHALL NOT modify the Queue.
4. WHEN a tipper calls `queue_tip` with a Queue_ID that does not exist, THE Aggregation_Protocol SHALL return an error.
5. WHEN a tipper calls `queue_tip` and the Queue already contains 100 or more entries, THE Aggregation_Protocol SHALL return an error and SHALL NOT append the entry.
6. WHEN a tipper calls `queue_tip` while the contract is paused, THE Aggregation_Protocol SHALL return an error and SHALL NOT modify the Queue.
7. WHEN a `QueuedTip` is successfully appended, THE Aggregation_Protocol SHALL emit a `("agg_tip_queued",)` event with data `(queue_id, sender, creator, token, amount, expiry)`.

---

### Requirement 3: Optimal Batch Size Calculation

**User Story:** As a settler, I want the protocol to calculate the optimal number of tips to settle per transaction so that I can maximise gas efficiency without exceeding Soroban resource limits.

#### Acceptance Criteria

1. THE Aggregation_Protocol SHALL expose a `calculate_optimal_batch_size` query function that accepts a Queue_ID and returns a `u32` representing the recommended number of entries to settle in one call.
2. WHEN `calculate_optimal_batch_size` is called, THE Aggregation_Protocol SHALL return a value between 1 and the configured `max_batch_size` (inclusive).
3. WHEN `calculate_optimal_batch_size` is called on a Queue with zero eligible (non-expired) entries, THE Aggregation_Protocol SHALL return 0.
4. THE Aggregation_Protocol SHALL base the optimal batch size calculation on the number of distinct tokens in the eligible entries, the current queue depth, and the configured `max_batch_size` parameter.
5. THE Aggregation_Protocol SHALL store a configurable `max_batch_size` parameter (default: 50, maximum: 100) in persistent storage, settable only by the contract admin.

---

### Requirement 4: Settlement Execution

**User Story:** As a settler, I want to trigger settlement of a queue so that accumulated tips are transferred to creators in a single gas-efficient transaction.

#### Acceptance Criteria

1. WHEN a Settler calls `settle_queue` with a valid Queue_ID and a `batch_size` between 1 and `max_batch_size` (inclusive), THE Aggregation_Protocol SHALL process up to `batch_size` eligible (non-expired) `QueuedTip` entries from the front of the Queue.
2. WHEN settling, THE Aggregation_Protocol SHALL skip any `QueuedTip` entries whose expiry timestamp is less than or equal to the current ledger timestamp and SHALL NOT transfer funds for those entries.
3. WHEN settling, THE Aggregation_Protocol SHALL deduct the Platform_Fee from each tip amount before crediting the creator's balance, consistent with the existing fee mechanism.
4. WHEN settling, THE Aggregation_Protocol SHALL accumulate the Platform_Fee portion into `DataKey::PlatformFeeBalance` for the relevant token.
5. WHEN settling, THE Aggregation_Protocol SHALL remove all processed entries (both settled and expired) from the Queue.
6. WHEN `settle_queue` is called while the contract is paused, THE Aggregation_Protocol SHALL return an error and SHALL NOT modify any state.
7. WHEN `settle_queue` is called with a `batch_size` of zero or greater than `max_batch_size`, THE Aggregation_Protocol SHALL return an error.
8. WHEN `settle_queue` is called with a Queue_ID that does not exist, THE Aggregation_Protocol SHALL return an error.
9. WHEN settlement completes, THE Aggregation_Protocol SHALL emit a `("agg_settled",)` event with data `(queue_id, settled_count, expired_count, total_amount, token)`.
10. WHEN settlement completes, THE Aggregation_Protocol SHALL emit one `("agg_tip_settled",)` event per successfully settled entry with data `(queue_id, sender, creator, token, net_amount, fee_amount)`.

---

### Requirement 5: Expired Tip Refunds

**User Story:** As a tipper, I want to reclaim funds from my expired queued tips so that my tokens are not locked indefinitely.

#### Acceptance Criteria

1. WHEN a tipper calls `refund_expired_tip` with a valid Queue_ID and entry index, and the referenced `QueuedTip` has an expiry timestamp less than or equal to the current ledger timestamp, THE Aggregation_Protocol SHALL transfer the full tip amount back to the original sender and SHALL remove the entry from the Queue.
2. WHEN a tipper calls `refund_expired_tip` for a `QueuedTip` that has not yet expired, THE Aggregation_Protocol SHALL return an error and SHALL NOT transfer any funds.
3. WHEN a tipper calls `refund_expired_tip` for a `QueuedTip` whose sender does not match the caller, THE Aggregation_Protocol SHALL return an error and SHALL NOT transfer any funds.
4. WHEN a tipper calls `refund_expired_tip` with a Queue_ID or entry index that does not exist, THE Aggregation_Protocol SHALL return an error.
5. WHEN a refund is successfully processed, THE Aggregation_Protocol SHALL emit a `("agg_tip_refunded",)` event with data `(queue_id, sender, creator, token, amount)`.

---

### Requirement 6: Queue Cancellation

**User Story:** As the contract admin, I want to cancel an entire queue so that all pending tips are refunded and the queue is removed from storage.

#### Acceptance Criteria

1. WHEN the contract admin calls `cancel_queue` with a valid Queue_ID, THE Aggregation_Protocol SHALL refund the full amount of every non-expired `QueuedTip` in the Queue to its original sender and SHALL delete the Queue from storage.
2. WHEN the contract admin calls `cancel_queue` with a Queue_ID that does not exist, THE Aggregation_Protocol SHALL return an error.
3. WHEN a non-admin address calls `cancel_queue`, THE Aggregation_Protocol SHALL return an error and SHALL NOT modify any state.
4. WHEN a queue is successfully cancelled, THE Aggregation_Protocol SHALL emit a `("agg_queue_cancelled",)` event with data `(queue_id, refunded_count, total_refunded, token)`.

---

### Requirement 7: Aggregation Configuration

**User Story:** As the contract admin, I want to configure aggregation parameters so that I can tune gas efficiency and queue behaviour for the platform.

#### Acceptance Criteria

1. WHEN the contract admin calls `set_aggregation_config` with a `max_batch_size` between 1 and 100 (inclusive), THE Aggregation_Protocol SHALL persist the new value in storage.
2. WHEN the contract admin calls `set_aggregation_config` with a `max_batch_size` of zero or greater than 100, THE Aggregation_Protocol SHALL return an error and SHALL NOT update the stored value.
3. WHEN a non-admin address calls `set_aggregation_config`, THE Aggregation_Protocol SHALL return an error and SHALL NOT modify any configuration.
4. THE Aggregation_Protocol SHALL use a default `max_batch_size` of 50 when no configuration has been set by the admin.

---

### Requirement 8: Queue State Queries

**User Story:** As an off-chain indexer or settler, I want to query queue state so that I can determine when and how to trigger settlement.

#### Acceptance Criteria

1. THE Aggregation_Protocol SHALL expose a `get_queue` query function that returns the full `Queue` struct (including all `QueuedTip` entries) for a given Queue_ID.
2. WHEN `get_queue` is called with a Queue_ID that does not exist, THE Aggregation_Protocol SHALL return an error.
3. THE Aggregation_Protocol SHALL expose a `get_queue_depth` query function that returns the number of entries currently in a Queue as a `u32`.
4. THE Aggregation_Protocol SHALL expose a `get_eligible_count` query function that returns the number of non-expired entries in a Queue as a `u32`, based on the current ledger timestamp.

---

### Requirement 9: Circuit Breaker Integration

**User Story:** As a platform operator, I want the aggregation protocol to respect the existing circuit breaker so that large settlement volumes do not bypass safety controls.

#### Acceptance Criteria

1. WHEN `settle_queue` is called and the total settlement amount for a token would trigger the Circuit_Breaker threshold, THE Aggregation_Protocol SHALL halt settlement and return an error, consistent with the existing circuit breaker behaviour.
2. WHILE the Circuit_Breaker is in a halted state, THE Aggregation_Protocol SHALL reject all `settle_queue` calls and SHALL return an error.
3. WHILE the Circuit_Breaker is in a halted state, THE Aggregation_Protocol SHALL still allow `queue_tip` calls so that tips can continue to be queued for later settlement.
