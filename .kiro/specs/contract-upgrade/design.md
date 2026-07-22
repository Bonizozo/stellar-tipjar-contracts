# Contract Upgrade and Migration System — Design

> **Status: implemented.** Rewritten to match what actually shipped — see
> [`docs/UPGRADE_GUIDE.md`](../../../docs/UPGRADE_GUIDE.md) for the
> authoritative, maintained version of this document and
> [`docs/UPGRADE_RUNBOOK.md`](../../../docs/UPGRADE_RUNBOOK.md) for the
> operational procedure. Tracked here (issue #358).

## 1. Soroban Native Upgrade Mechanism

```rust
env.deployer().update_current_contract_wasm(new_wasm_hash);
```

`new_wasm_hash` is a `BytesN<32>` identifying WASM already uploaded to the
network (`env.deployer().upload_contract_wasm(..)`). The host atomically
swaps the bytecode; all storage (instance + persistent) is untouched. The
call succeeds or the entire transaction reverts.

## 2. Timelock

`propose_upgrade` computes `unlock_ledger = env.ledger().sequence() +
UpgradeTimelockLedgers`, where the timelock duration is fixed once at
`init(token, admin, upgrade_timelock_ledgers)` — there is no entrypoint to
change it afterwards. `execute_upgrade()` panics with
`Error::TimelockNotElapsed` while `env.ledger().sequence() < unlock_ledger`.

```rust
pub fn propose_upgrade(env: Env, admin: Address, new_wasm_hash: BytesN<32>) {
    admin.require_auth();
    Self::require_admin(&env, &admin);
    if env.storage().instance().has(&DataKey::PendingUpgrade) {
        panic_with_error!(&env, Error::UpgradeAlreadyPending);
    }
    let timelock: u32 = env.storage().instance().get(&DataKey::UpgradeTimelockLedgers).unwrap();
    let unlock_ledger = env.ledger().sequence() + timelock;
    env.storage().instance().set(&DataKey::PendingUpgrade, &(new_wasm_hash.clone(), unlock_ledger));
    UpgradeProposed { hash: new_wasm_hash, unlock_ledger }.publish(&env);
}

pub fn execute_upgrade(env: Env) {
    let (hash, unlock_ledger): (BytesN<32>, u32) = env
        .storage().instance().get(&DataKey::PendingUpgrade)
        .unwrap_or_else(|| panic_with_error!(&env, Error::NoPendingUpgrade));
    if env.ledger().sequence() < unlock_ledger {
        panic_with_error!(&env, Error::TimelockNotElapsed);
    }
    env.storage().instance().remove(&DataKey::PendingUpgrade);
    env.deployer().update_current_contract_wasm(hash.clone());
    UpgradeExecuted { hash }.publish(&env);
}
```

`execute_upgrade` is deliberately **permissionless** — no caller argument, no
`require_auth`. The admin already authorized the change at `propose_upgrade`,
and `unlock_ledger` is public on-chain state; gating *who* triggers the
mechanical swap once the timelock has elapsed adds no real security and only
introduces a liveness dependency on one key.

## 3. Admin-Only Proposal/Cancellation, Admin-Only Migration

`propose_upgrade`, `cancel_upgrade`, and `migrate` all follow the same
pattern: `admin.require_auth()`, then compare against stored
`DataKey::Admin`, panicking `Error::Unauthorized` on mismatch.

## 4. Two-Step Admin Transfer

```rust
pub fn propose_admin(env: Env, admin: Address, new_admin: Address) {
    admin.require_auth();
    Self::require_admin(&env, &admin);
    env.storage().instance().set(&DataKey::PendingAdmin, &new_admin);
    AdminTransferProposed { current_admin: admin, new_admin }.publish(&env);
}

pub fn accept_admin(env: Env, new_admin: Address) {
    new_admin.require_auth();
    let pending: Address = env.storage().instance().get(&DataKey::PendingAdmin)
        .unwrap_or_else(|| panic_with_error!(&env, Error::NoPendingAdmin));
    if pending != new_admin {
        panic_with_error!(&env, Error::NoPendingAdmin);
    }
    env.storage().instance().set(&DataKey::Admin, &new_admin);
    env.storage().instance().remove(&DataKey::PendingAdmin);
    AdminTransferAccepted { new_admin }.publish(&env);
}
```

The old admin keeps full authority until the new address actively claims it
via `accept_admin` — a typo'd or unreachable proposed address can never
permanently lock out administration.

## 5. Storage Versioning and `migrate()`

`DataKey::DataVersion` is set to the build's `DATA_VERSION` constant at
`init`. `migrate(admin)` is idempotent and version-gated:

```rust
pub fn migrate(env: Env, admin: Address) {
    admin.require_auth();
    Self::require_admin(&env, &admin);
    let current: u32 = env.storage().instance().get(&DataKey::DataVersion).unwrap_or(1);
    if current >= DATA_VERSION {
        return; // no-op: already at or past this build's target version
    }
    // storage transformation for this version step would run here
    env.storage().instance().set(&DataKey::DataVersion, &DATA_VERSION);
    Migrated { from_version: current, to_version: DATA_VERSION }.publish(&env);
}
```

Every build defines its own `DATA_VERSION`; a release with no storage change
still ships a `migrate()` that reads as a no-op under this rule, which is
what keeps the double-invocation guarantee meaningful across every release.

## 6. Testing Strategy

Soroban's fast test mode runs `#[contract]` types natively rather than
through WASM, so a WASM swap can't be exercised by registering a second Rust
type in the same test binary — `upload_contract_wasm` needs real compiled
bytes. `contracts/tipjar-v2-fixture` is a separate crate, built to
`wasm32v1-none` ahead of the test run and pulled in via
`soroban_sdk::contractimport!` in `contracts/tipjar/src/test_upgrade.rs`.
It deliberately does not depend on `contracts/tipjar` as a library:
`#[contractimpl]` exports every function as a forced WASM symbol, so linking
one contract's compiled code into another's cdylib collides on those
exports. It instead declares its own storage-compatible `DataKey`/`Error`
copies — legitimate because Soroban's `contracttype` derive encodes enum
variants by name, so an independently-declared enum with matching variant
names reads and writes the same storage entries.

## 7. Backward Compatibility Rules

| Change | Safe? | Notes |
|---|---|---|
| Add new `DataKey` variant | ✅ | Variants are encoded by name, not position — order never matters. |
| Add new contract function | ✅ | No selector collision. |
| Remove a `DataKey` variant | ❌ | Orphans stored data. |
| Rename a `DataKey` variant | ❌ | Requires a `migrate()` step. |
| Change value type of an existing key | ❌ | Requires a `migrate()` step. |
| Remove explicit `= N` from an `Error` discriminant | ❌ | Breaks client error-code handling. |

## 8. Correctness Properties

- **P1 (Authorization):** For any caller `c ≠ stored_admin`, `propose_upgrade(c, _)`, `cancel_upgrade(c)`, and `migrate(c)` always panic with `Unauthorized`.
- **P2 (Timelock):** `execute_upgrade()` panics with `TimelockNotElapsed` for every ledger `< unlock_ledger` and succeeds at every ledger `>= unlock_ledger`.
- **P3 (State preservation):** Creator balances and totals are byte-identical immediately before and after `execute_upgrade`.
- **P4 (Migration idempotency):** Any number of consecutive `migrate()` calls after `DataVersion` reaches `DATA_VERSION` produce no further state change and no additional `Migrated` event.
- **P5 (Event emission):** Every successful state transition emits exactly the event documented in `docs/EVENTS.md`.

All five are covered by tests in `contracts/tipjar/src/test_upgrade.rs`.
