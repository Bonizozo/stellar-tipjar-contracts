# Programmable Royalty System

## Overview

The programmable royalty system enables creators to define complex, rule-based royalty distributions with dynamic splits and conditional logic. It extends the existing royalty/split system with powerful automation for collaborative content, derivative works, and conditional payouts.

## Features

### 1. Rule Definitions

Define multiple royalty rules per creator, each with:
- **Conditions**: Gate when a rule applies
- **Recipients**: Dynamic recipients with base + bonus shares
- **Priority**: Evaluation order
- **Enable/Disable**: Toggle rules without deletion

```rust
pub fn create_rule(
    env: &Env,
    creator: &Address,
    owner: &Address,
    conditions: Vec<Condition>,
    recipients: Vec<DynamicRecipient>,
    priority: u32,
) -> u64
```

### 2. Dynamic Splits

Splits adjust based on context through two mechanisms:

- **Base Share** (`base_bps`): Fixed percentage
- **Bonus Share** (`bonus_bps`): Applied on top when conditions met

Example: A producer receives 30% base, plus 10% bonus when tips exceed 1000 tokens.

```rust
pub struct DynamicRecipient {
    pub recipient: Address,
    pub base_bps: u32,      // Base share in basis points
    pub bonus_bps: u32,     // Bonus when conditions met
}
```

### 3. Conditional Logic

Eight condition types for flexible rule gatekeeping:

```rust
pub enum Condition {
    Always,                              // Always applies
    MinAmount { threshold: i128 },       // Tip >= threshold
    MaxAmount { threshold: i128 },       // Tip <= threshold
    FromSender { sender: Address },      // Specific tipper
    FromList { senders: Vec<Address> },  // Whitelist of tippers
    TimeAfter { start_ts: u64 },        // After timestamp
    TimeBefore { end_ts: u64 },         // Before timestamp
    MinTipperCount { threshold: u32 },  // >= unique tippers
    MinTotalTips { threshold: i128 },   // Total tips received >= threshold
}
```

**Evaluation**: All conditions in a rule must be true for the rule to apply.

### 4. Nested Royalties

Rules are evaluated recursively up to `MAX_NEST_DEPTH` (5 levels):

```
Creator A receives tip
  ├─ Rule triggers → Distribute to Creator B (50%)
  │   └─ Creator B has rules → Evaluate recursively
  │       └─ Distribute to Creator C (40% of Creator B's share)
  └─ Remaining goes to Creator A's direct accounts
```

Each nested level reduces the distribution amount proportionally.

### 5. Payment Tracking

Every distribution is recorded with full context:

```rust
pub struct RoyaltyPayment {
    pub recipient: Address,      // Who received
    pub amount: i128,            // How much
    pub timestamp: u64,          // When
    pub rule_id: u64,            // Which rule applied
    pub source_amount: i128,     // Original tip amount
}
```

Retrieve payment history by creator:
```rust
pub fn get_payment(env: &Env, creator: &Address, index: u64) -> Option<RoyaltyPayment>
pub fn get_payment_count(env: &Env, creator: &Address) -> u64
```

## API Reference

### Rule Management

```rust
// Create a new rule
pub fn create_rule(
    env: &Env,
    creator: &Address,
    owner: &Address,
    conditions: Vec<Condition>,
    recipients: Vec<DynamicRecipient>,
    priority: u32,
) -> u64

// Update an existing rule
pub fn update_rule(
    env: &Env,
    creator: &Address,
    rule_id: u64,
    owner: &Address,
    conditions: Vec<Condition>,
    recipients: Vec<DynamicRecipient>,
    priority: u32,
)

// Enable or disable a rule
pub fn toggle_rule(
    env: &Env,
    rule_id: u64,
    owner: &Address,
    enabled: bool,
)

// Retrieve a rule
pub fn get_rule(env: &Env, rule_id: u64) -> Option<RoyaltyRule>

// Get all rules for a creator
pub fn get_creator_rules(env: &Env, creator: &Address) -> Vec<RoyaltyRule>
```

### Distribution

```rust
// Apply all applicable rules to a tip
pub fn distribute_programmable_royalties(
    env: &Env,
    creator: &Address,
    tip_amount: i128,
    sender: &Address,
) -> i128  // Returns remaining amount
```

Called automatically from the main `tip()` contract function.

### Withdrawal & Queries

```rust
// Withdraw accumulated royalties
pub fn withdraw_programmable_royalties(
    env: &Env,
    recipient: &Address,
    token_addr: &Address,
) -> i128

// Get accumulated balance
pub fn get_programmable_balance(env: &Env, recipient: &Address) -> i128

// Get total tips received by creator
pub fn get_total_tips_received(env: &Env, creator: &Address) -> i128

// Get unique tipper count
pub fn get_tipper_count(env: &Env, creator: &Address) -> u32

// Get payment history
pub fn get_payment(env: &Env, creator: &Address, index: u64) -> Option<RoyaltyPayment>
pub fn get_payment_count(env: &Env, creator: &Address) -> u64
```

## Use Cases

### 1. Collaborative Content

Multiple contributors with different roles:

```
Rule 1: Default split (all tippers)
  Producer: 30%
  Vocalist: 50%
  Musician: 20%

Rule 2: Premium bonus when tip >= 100 tokens
  Producer: 30% + 5%
  Vocalist: 50% + 10%
  Musician: 20% + 10%
```

### 2. Derivative Works

Creator receives royalties from derivative content:

```
Rule: Derivative artist receives tips
  Original Creator: 15% (royalty)
  Derivative Creator: 85%

Nested: If Original Creator has rules, apply recursively
```

### 3. Time-Limited Promotions

Special splits during campaign periods:

```
Rule 1: Before Jan 1, 2025
  Creator: 90%
  Platform: 10%

Rule 2: After Jan 1, 2025
  Creator: 95%
  Platform: 5%
```

### 4. Tiered Rewards

Increase payouts at milestones:

```
Rule 1: < 100 total tips received
  Creator: 85%
  Platform: 15%

Rule 2: >= 100 total tips received
  Creator: 90%
  Platform: 10%

Rule 3: >= 1000 total tips received
  Creator: 95%
  Platform: 5%
```

### 5. VIP Tipper Benefits

Special handling for major supporters:

```
Rule 1: From VIP whitelist
  Creator: 95%
  Platform: 5%
  VIP Bonus Pool: 0% (sent to VIP directly as thank you)

Rule 2: All other tippers
  Creator: 85%
  Platform: 15%
```

## Storage Model

| Key | Purpose |
|-----|---------|
| `ProgrammableRoyaltyRule(u64)` | Rule definition by ID |
| `ProgrammableRoyaltyCreatorRule(Address, u64)` | Creator's rule ID at index |
| `ProgrammableRoyaltyPayment(Address, u64)` | Payment record for creator at index |
| `ProgrammableRoyaltyPaymentCount(Address)` | Total payments for creator |
| `ProgrammableRoyaltyCounter` | Global rule ID counter |
| `ProgrammableRoyaltyBalance(Address)` | Accumulated balance for recipient |
| `ProgrammableRoyaltyTotalTips(Address)` | Total tips received by creator |
| `ProgrammableRoyaltyTipperCount(Address)` | Unique tipper count |

## Constraints

- **Max Rules per Creator**: 10
- **Max Conditions per Rule**: 5
- **Max Recipients per Rule**: 20
- **Max Nesting Depth**: 5 levels
- **Basis Points Denominator**: 10,000 (1 bp = 0.01%)

## Events

| Event | Data |
|-------|------|
| `"prog_rul"` | `(creator, rule_id)` |
| `"prog_upd"` | `(creator, rule_id)` |
| `"prog_dst"` | `(creator, distributed_amount)` |
| `"prog_wdw"` | `(recipient, withdrawn_amount)` |

## Integration with Main Contract

Programmable royalties are automatically applied in the `tip()` function after the main tip is recorded:

```rust
// Apply programmable royalties if configured
let _remaining = programmable_royalty::distribute_programmable_royalties(
    &env,
    &creator,
    creator_amount,
    &sender,
);
```

Creators without rules configured are unaffected (distribution returns the full amount).

## Testing

Unit tests for:
- Rule creation and modification
- Condition evaluation (all types)
- Dynamic split calculations with bonuses
- Nested royalty resolution
- Payment tracking and history
- Rule priority ordering
- Tipper count and total tips tracking
- Withdrawal functionality

See `/workspaces/stellar-tipjar-contracts/contracts/tipjar/tests/programmable_royalty_tests.rs`

## Future Enhancements

- Percentage-based conditions (e.g., "if previous month's tips > 500")
- Time-decay splits (percentage degrades over time)
- Multi-level nesting with custom aggregation
- Royalty caps (max percentage per rule)
- Conditional rule chaining (e.g., "apply rule B only if rule A matched")
- Batch distribution for efficiency
- Royalty dispute resolution integration
