# State Management

## On-Chain State

All contract state is stored in Soroban's key-value store. The `DataKey` enum defines every storage key used by the contract.

### Instance Storage (shared, low-cost reads)

| Key | Type | Description |
|---|---|---|
| `DataKey::Token` | `Address` | Whitelisted token contract address |
| `DataKey::Admin` | `Address` | Contract administrator |
| `DataKey::Paused` | `bool` | Emergency pause flag |

### Persistent Storage (per-entity, TTL-extended on access)

| Key | Type | Description |
|---|---|---|
| `DataKey::CreatorBalance(Address)` | `i128` | Withdrawable escrow balance |
| `DataKey::CreatorTotal(Address)` | `i128` | Cumulative historical tips |
| `DataKey::CreatorMessages(Address)` | `Vec<TipWithMessage>` | Tip messages |
| `DataKey::LockedTip(u64)` | `TipWithExpiry` | Time-locked tip by ID |
| `DataKey::Subscription(Address, Address)` | `Subscription` | Recurring tip subscription |
| `DataKey::LeaderboardEntry(Address)` | `LeaderboardEntry` | Leaderboard aggregate |
| `DataKey::MatchingProgram(u64)` | `MatchingProgram` | Tip matching program |
| `DataKey::Milestone(u64)` | `Milestone` | Creator milestone |
| `DataKey::Role(Address)` | `Role` | RBAC role assignment |

## State Transitions

### Tip Flow

```
Initial: CreatorBalance = B, CreatorTotal = T
After tip(amount):
  CreatorBalance = B + amount
  CreatorTotal   = T + amount
```

### Withdraw Flow

```
Initial: CreatorBalance = B (B > 0)
After withdraw():
  CreatorBalance = 0
  Token transferred: B tokens → creator address
```

### Locked Tip Flow

```
tip_locked(amount, unlock_time) → stores TipWithExpiry{claimed: false}
withdraw_locked(tip_id)         → requires ledger_time >= unlock_time
                                  sets claimed = true, transfers amount
```

### Pause / Unpause

```
pause()   → Paused = true  (blocks tip, withdraw, tip_with_message)
unpause() → Paused = false (restores normal operation)
```

## Off-Chain State (Indexer)

The indexer maintains a relational mirror of on-chain events:

```sql
-- events table (see indexer/migrations/0003_create_events.sql)
CREATE TABLE events (
  id          SERIAL PRIMARY KEY,
  ledger      BIGINT NOT NULL,
  tx_hash     TEXT NOT NULL,
  event_type  TEXT NOT NULL,   -- 'tip' | 'withdraw' | 'tip_msg' | ...
  creator     TEXT NOT NULL,
  sender      TEXT,
  amount      BIGINT NOT NULL,
  timestamp   TIMESTAMPTZ NOT NULL
);
```

The indexer processes events in order and is idempotent — re-processing the same ledger range produces the same DB state.

## SDK State

The TypeScript SDK is stateless between calls. The only mutable state it holds is:

- `keypair` — set via `connect(keypair)`, used to sign transactions.
- `server` — `SorobanRpc.Server` instance (connection pool managed by the SDK).

No caching of on-chain values is performed; every read call issues a fresh `simulateTransaction`.
