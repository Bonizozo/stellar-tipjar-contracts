# Storage Recovery & TTL Management

## Overview

The Soroban ledger automatically archives ("evicts") storage entries that are not accessed or bumped within a specified TTL (Time To Live). This document describes the archival behavior of TipJar entries, restoration procedures, and preventive measures.

## Ledger TTL Model

### Basics

Every persistent storage entry in Soroban has:

- **`live_until`** — ledger sequence number after which the entry is archived
- **TTL threshold** — threshold for bumping (default: 100K ledgers)
- **TTL bump** — how far to extend (default: 120K ledgers)

**Timeline:**
- Entry created at ledger `L`
- Automatically set to `live_until = L + 120_960` (~7 days at 5s/ledger)
- If accessed and bumped before `live_until`, TTL resets to `now + 120_960`
- After `live_until`, entry is archived and inaccessible

### Current TipJar Configuration

```rust
const LEDGER_THRESHOLD: u32 = 100_000;  // Bump threshold
const LEDGER_BUMP: u32 = 120_960;       // ~7 days
```

Every operation that writes to storage bumps affected keys:

| Operation | Keys Bumped | Read Access | Problem |
|-----------|-------------|-------------|---------|
| `init` | instance | ✅ | ✅ None (one-time) |
| `tip` | balance, total, instance | ✅ Yes (2 reads) | ✅ Both keys bumped on every tip |
| `get_total_tips` | total | ✅ Yes (1 read) | ❌ **Read-only; no TTL bump** |
| `withdraw` | balance, instance | ✅ Yes (1 read) | ✅ Both bumped; total untouched |

## Storage Entry Archival Matrix

### Instance Storage: Token Address

**Key:** `DataKey::Token`

**Lifecycle:**
1. Set once in `init`
2. Bumped on every operation (read + write pattern)
3. Never expires (constantly accessed)

**Archival Scenario:** Impossible — contract is always initialized and accessed on first use per transaction.

**Recovery:** N/A.

---

### Persistent Storage: CreatorBalance

**Key:** `DataKey::CreatorBalance(creator_address)`

**Lifecycle:**
1. Created on first `tip` for creator
2. Bumped on every `tip` (write) and `withdraw` (write)
3. Reset to 0 on `withdraw`, TTL still bumped

**Archival Scenario:**
- Creator receives tip on day 0 → `live_until = day 7`
- Creator receives no tips/withdrawals for 7 days → entry archived on day 7
- Creator tries to withdraw on day 8 → **storage miss, withdraw fails**
- Creator tries to tip someone else (using this creator's balance) on day 8 → **storage miss**

**User Impact:**
- Creator cannot withdraw
- Escrowed funds are **inaccessible but not lost** (entry exists off-chain; restoration required)
- No active tipping to this creator (since no one queries their balance)

**Recovery Procedure:**
1. Indexer detects archived entry via querying historical events
2. Keeper script calls `extend_entries(creator)` to restore TTL
3. Creator can now withdraw; historical total untouched

---

### Persistent Storage: CreatorTotal

**Key:** `DataKey::CreatorTotal(creator_address)`

**Lifecycle:**
1. Created on first `tip` for creator
2. Bumped on every `tip` (write)
3. **Not touched by `withdraw`** — total is immutable

**Archival Scenario:**
- Creator's last tip received 7 days ago → `live_until` reached
- No recent tips (inactive creator) → entry archived on day 7
- Creator queries `get_total_tips(creator)` on day 8 → **read returns 0 (entry missing)**
- This is **silent data loss** for historical reporting

**User Impact:**
- Historical stats disappear (showing 0 instead of actual total)
- Indexer out of sync (shows historical total, but contract returns 0)
- Leaderboards reset

**Recovery Procedure:**
1. Indexer detects discrepancy: has historical total but contract returns 0
2. Keeper calls `extend_entries(creator)` to restore TTL
3. Contract returns correct total again

---

### Persistent Storage: CreatorMessages (if future feature)

**Key:** `DataKey::CreatorMessages(creator_address)` (hypothetical future feature)

**Lifecycle:**
1. Created on first `tip_with_message`
2. Bumped on every `tip_with_message`
3. Not touched by `tip` (without message) or `withdraw`

**Archival Scenario:**
- Creator with rich message history receives last tipped-with-message on day 0 → `live_until = day 7`
- Regular tips (no message) do not bump this entry
- Day 7 reached → entry archived
- Query `get_messages(creator)` on day 8 → **returns empty list (entry missing)**

**User Impact:**
- Message history disappears
- Creator loses all attached notes/metadata

**Recovery Procedure:**
Same as CreatorTotal.

---

## TTL Maintenance Strategy

### Permissionless Extend Entrypoint

To prevent archival, introduce a permissionless keeper function:

```rust
pub fn extend_entries(env: Env, creator: Address, threshold: u32) {
    // Anyone can call this to keep a creator's entries alive.
    // Useful for indexers, community keepers, or the creator themselves.
    
    // Bump all keys for this creator:
    // - CreatorBalance
    // - CreatorTotal
    // - CreatorMessages (if exists)
    // - Instance storage
    
    // Require: threshold must be in [LEDGER_THRESHOLD_MIN, LEDGER_THRESHOLD_MAX]
    // to prevent DoS (very frequent bumps) or stale entries (too-rare bumps).
}

pub fn extend_entries_batch(env: Env, creators: Vec<Address>, threshold: u32) {
    // Batch variant; bounded to prevent Soroban CPU limit hits.
    // Max batch size: 50 creators.
    for creator in creators {
        extend_entries(env, creator, threshold);
    }
}
```

### Configurable TTL Parameters

Instead of hardcoded constants, make them instance-configurable:

```rust
pub struct Config {
    ledger_threshold: u32,  // Bump threshold
    ledger_bump: u32,       // Bump distance
}

pub fn set_ttl_config(env: Env, admin: Address, threshold: u32, bump: u32) {
    // Admin only
    // Enforce: THRESHOLD_MIN <= threshold <= THRESHOLD_MAX
    //          BUMP_MIN <= bump <= BUMP_MAX
}

pub fn get_ttl_config(env: Env) -> Config {
    // Read current TTL settings
}
```

**Justification for Configurability:**
- Soroban network parameters may evolve (TTL periods, archival rates)
- Different networks (testnet vs. mainnet) may have different defaults
- Emergency adjustments (e.g., rapid archival due to network lag) without code redeployment

---

## Keeper Tooling

### Keeper Script

Located at `scripts/keeper.sh`. Responsibilities:

1. **Scan recent Tip events** via off-chain indexer RPC
2. **Identify creators approaching TTL expiry** (e.g., last tip > 6 days ago)
3. **Batch call `extend_entries`** to keep entries alive
4. **Log and alert** if a creator's entry is already archived

**Usage:**

```bash
# Run keeper once (intended for cron)
bash scripts/keeper.sh

# Dry run (show what would be extended, don't submit)
bash scripts/keeper.sh --dry-run

# Extend for specific creator
bash scripts/keeper.sh --creator GBXXXXXXXX
```

**Configuration (env vars):**
```bash
KEEPER_RPC_URL         # Soroban RPC endpoint (default: testnet)
KEEPER_NETWORK         # Network passphrase
KEEPER_BATCH_SIZE      # Max creators per batch (default: 50)
KEEPER_TTL_THRESHOLD   # Ledger threshold for bumps (default: 100K)
KEEPER_EXPIRY_HORIZON  # Days before archival to trigger bump (default: 1)
```

### Indexer Integration

The keeper relies on the off-chain indexer to track:

- All `Tip` events (emitted with creator topic)
- Event timestamp → ledger sequence number (via RPC `getTransaction`)
- Creator's last activity date

**Contract Enhancement:** Emit `EntriesExtended { creator, new_live_until }` event when keeper calls `extend_entries` for observability.

---

## Archival Behavior Specification

### Per-Entry Behavior

This table defines what happens when each entry type is archived:

| Entry | Archived Behavior | User-Facing Impact | Restoration |
|-------|-------------------|-------------------|-------------|
| `Token` (instance) | Impossible | N/A | N/A |
| `CreatorBalance` | Read returns 0 | Cannot withdraw; funds inaccessible | Call `extend_entries` |
| `CreatorTotal` | Read returns 0 | Historical stats disappear | Call `extend_entries` |
| `CreatorMessages` | Read returns empty | Message history lost | Call `extend_entries` |

### Restoration Procedure

**For Creator:**
1. Run keeper: `bash scripts/keeper.sh --creator YOUR_ADDRESS`
2. Or call directly: `stellar contract invoke ... extend_entries --creator YOUR_ADDRESS`

**For Indexer:**
1. Detect mismatch: "Historical data shows total X, contract returns 0"
2. Log alert: "Creator GBXX data potentially archived"
3. Trigger keeper for affected creator
4. Resync database

**For Admin:**
1. Monitor keeper logs for repeated archival events
2. If frequent, consider raising `LEDGER_BUMP` via `set_ttl_config`

---

## Current Network Limits & Defaults

### Soroban Ledger Parameters (as of Protocol 21)

| Parameter | Value | Notes |
|-----------|-------|-------|
| `max_entry_ttl` | ~120 days (63,072,000 ledgers) | Maximum TTL any entry can have |
| `min_entry_ttl` | 1 ledger | Minimum TTL (ephemeral storage) |
| `archival_window` | ~90 days | When archived entries become truly deleted |

### TipJar Defaults

| Parameter | Value | Justification |
|-----------|-------|---|
| `LEDGER_THRESHOLD` | 100K | ~1.4 days; bumped on every write |
| `LEDGER_BUMP` | 120,960 | ~7 days; balances protection vs. rent fees |

These were chosen to:
- **Protect active creators:** 7-day window is reasonable for tipping activity patterns
- **Minimize rent:** 7 days = ~30/month per address, cost-effective
- **Enable recovery:** 1-day warning threshold before archival allows keeper to react

---

## Testing TTL Expiry

### Ledger Advancement Test Pattern

Use `env.ledger().with_mut()` to test archival scenarios:

```rust
#[test]
fn test_archival_after_ttl_expiry() {
    let ctx = setup();
    
    // Create a balance for creator
    ctx.client().tip(&sender, &creator, &500);
    assert_eq!(ctx.client().get_total_tips(&creator), 500);
    
    // Advance ledger past TTL expiry
    let current_ledger = ctx.env.ledger().sequence();
    let expiry_ledger = current_ledger + LEDGER_BUMP + 1;
    ctx.env.ledger().with_mut(|ledger| {
        ledger.set_sequence_number(expiry_ledger);
    });
    
    // Entry is archived; read returns default
    assert_eq!(ctx.client().get_total_tips(&creator), 0);
    
    // Restore via extend_entries
    ctx.client().extend_entries(&creator, LEDGER_THRESHOLD);
    
    // Total is readable again
    assert_eq!(ctx.client().get_total_tips(&creator), 500);
}
```

### Test Suite for `extend_entries`

- Entry alive before TTL → bump extends it
- Entry just-archived → restore via bump
- Multi-creator batch → all bumped atomically
- Invalid threshold → rejected (with error)
- Permissionless access → any caller accepted

---

## Monitoring & Observability

### Metrics to Track

1. **Archived entries per day** — rising trend indicates configuration needs adjustment
2. **Keeper execution time** — batch size tuning for cost/speed tradeoff
3. **Entries with <1 day to expiry** — upstream alert to creators
4. **Recovery success rate** — keeper effectiveness

### Events for Alerting

Contract emits `EntriesExtended { creator, live_until }` on every keeper call.

Dashboard queries:
- Group by creator → "most-extended creators" (potential issues?)
- Rate per hour → detect keeper overload
- Time since last extend → "entries at risk"

---

## References

- **Soroban TTL Model:** [Soroban docs - Storage Ledger Entries](https://developers.stellar.org/docs/learn/storing-data)
- **Archival & Restoration:** [Soroban SDK - Ledger TTL](https://docs.rs/soroban-sdk/latest/soroban_sdk/env/trait.Ledger.html)
- **RESOURCE_BUDGETS.md** — Related; keeper calls must respect resource budgets
- **ARCHITECTURE.md** — Storage layout and entry point design
