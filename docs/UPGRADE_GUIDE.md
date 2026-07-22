# TipJar Contract Upgrade Guide

This guide documents `contracts/tipjar`'s on-chain upgrade mechanism: a
timelocked, admin-gated WASM swap plus a version-gated `migrate()` entrypoint
for storage-layout changes. For the step-by-step operational procedure
(propose → monitor → execute → verify), see
[`docs/UPGRADE_RUNBOOK.md`](./UPGRADE_RUNBOOK.md).

> `contracts/tipjar-legacy` is a frozen reference contract kept only so
> `simulator` and `tools/gas-estimator` have a richer feature surface to
> exercise; it is not deployed anywhere. Its own `upgrade()` function is
> retired — this document and the runbook describe the only supported
> upgrade path in this workspace.

---

## Storage

Set once at `init(token, admin, upgrade_timelock_ledgers)`:

| Key | Type | Purpose |
|---|---|---|
| `DataKey::Admin` | `Address` | May call `propose_upgrade`, `cancel_upgrade`, `migrate`, `propose_admin`. |
| `DataKey::UpgradeTimelockLedgers` | `u32` | Ledger delay `execute_upgrade` must wait out after a `propose_upgrade`. Fixed at init; there is no entrypoint to change it later. |
| `DataKey::DataVersion` | `u32` | Storage schema version, initialized to `DATA_VERSION` (currently `1`). Advanced by `migrate()`. |

Set only while a proposal or transfer is pending:

| Key | Type | Purpose |
|---|---|---|
| `DataKey::PendingUpgrade` | `(BytesN<32>, u32)` | `(new_wasm_hash, unlock_ledger)`. Present only between `propose_upgrade` and `execute_upgrade`/`cancel_upgrade`. |
| `DataKey::PendingAdmin` | `Address` | Present only between `propose_admin` and `accept_admin`. |

## Functions

### `propose_upgrade(admin: Address, new_wasm_hash: BytesN<32>)`

Admin-only (`admin.require_auth()` + must match `DataKey::Admin`). Records
`new_wasm_hash` and `unlock_ledger = current_ledger + UpgradeTimelockLedgers`.
Fails with `UpgradeAlreadyPending` if a proposal is already pending — cancel
it first to replace it. Emits `UpgradeProposed { hash, unlock_ledger }`.

### `execute_upgrade()`

**Permissionless** — no caller argument, no auth check. The admin already
authorized the upgrade at `propose_upgrade`, and `unlock_ledger` is public
on-chain state, so restricting *who* triggers the mechanical swap once the
timelock has elapsed adds no real security; it does remove a liveness
dependency on any single key. Fails with `NoPendingUpgrade` if nothing is
pending, or `TimelockNotElapsed` if `env.ledger().sequence() < unlock_ledger`
(so calling at `unlock_ledger - 1` panics; calling at exactly `unlock_ledger`
succeeds). On success, clears the pending proposal, calls
`env.deployer().update_current_contract_wasm(hash)`, and emits
`UpgradeExecuted { hash }`. All instance and persistent storage survives the
swap untouched — the host guarantees this.

### `cancel_upgrade(admin: Address)`

Admin-only. Aborts a pending proposal at any time before execution, timelock
elapsed or not. Fails with `NoPendingUpgrade` if nothing is pending. Emits
`UpgradeCancelled { hash }`.

### `migrate(admin: Address)`

Admin-only, **idempotent and version-gated**. Every contract build defines
its own `DATA_VERSION` constant. `migrate()` compares it against the stored
`DataKey::DataVersion`: if the stored value already meets or exceeds
`DATA_VERSION`, it returns immediately — no panic, no event, safe to call any
number of times (including before the first upgrade, or repeatedly after
one). Otherwise it applies whatever storage transformation that version step
requires and advances `DataKey::DataVersion`, emitting
`Migrated { from_version, to_version }`.

Call it once, by hand, after `execute_upgrade` swaps in a WASM whose
`DATA_VERSION` is higher than the currently stored value. New WASM releases
that don't change storage layout can (and should) still expose a `migrate()`
that simply reads as a no-op under this rule — that's what keeps the
double-invocation guarantee meaningful across every release, not just the
ones with an actual transformation to run.

### Two-step admin transfer

`propose_admin(admin: Address, new_admin: Address)` (admin-only) records
`new_admin` as `DataKey::PendingAdmin` and emits `AdminTransferProposed`. The
current admin keeps full authority until `new_admin` itself calls
`accept_admin(new_admin: Address)`, which fails with `NoPendingAdmin` unless
it exactly matches the pending proposal, and otherwise sets `DataKey::Admin`,
clears the pending entry, and emits `AdminTransferAccepted`. A typo'd or
unreachable proposed address can never permanently lock out administration —
the old admin retains authority (including the ability to `propose_admin`
again) until the new address actively claims it.

## Backward Compatibility Rules

Soroban's `contracttype` derive encodes enum variants by name, not by
declaration order or position — so these rules follow directly from that:

| Change | Safe? | Notes |
|---|---|---|
| Add new `DataKey` variant | ✅ | Existing keys are unaffected regardless of where the new variant is declared. |
| Add new contract function | ✅ | No selector collision. |
| Remove a `DataKey` variant | ❌ | Orphans stored data — nothing will ever read it again, but it isn't reclaimed either. |
| Rename a `DataKey` variant | ❌ | Same bytes on-chain, new name — old entries become unreadable under the new name. Requires a `migrate()` step that reads the old (still-declared) variant and re-writes under the new one. |
| Change the value type stored under an existing key | ❌ | Requires a `migrate()` step to decode the old shape and re-encode the new one. |
| Reorder `Error` discriminants | ❌ | Discriminants are explicit `= N` values precisely so this is safe *unless* someone removes the explicit value — never do that. |

## Testing

`contracts/tipjar/src/test_upgrade.rs` exercises the full lifecycle against a
second, genuinely distinct compiled WASM (`contracts/tipjar-v2-fixture`,
registered via `soroban_sdk::contractimport!`) rather than only unit-testing
the storage writes in isolation — see that file's module docs for why a real
second binary is necessary. It covers: balance/total preservation across the
swap, the timelock boundary (`unlock_ledger - 1` panics, `unlock_ledger`
succeeds), cancellation, non-admin rejection of `propose_upgrade` and
`cancel_upgrade`, a second pending proposal being rejected, `migrate()`
idempotency across repeated calls, and the two-step admin transfer.

```bash
cargo build -p tipjar-v2-fixture --target wasm32v1-none --release
cargo test -p tipjar
```

The fixture must be built first — `contractimport!` embeds the compiled
WASM at compile time, so it has to already exist on disk.
