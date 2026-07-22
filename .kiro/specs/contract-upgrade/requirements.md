# Contract Upgrade and Migration System — Requirements

> **Status: implemented.** This spec predates the shipped design and
> originally described a single-step, non-timelocked `upgrade()` entrypoint
> with a `TipJarError`/`ContractVersion` naming scheme that never matched
> `contracts/tipjar`'s actual `Error` enum or storage layout. It has been
> rewritten to describe what was actually built — see
> [`docs/UPGRADE_GUIDE.md`](../../../docs/UPGRADE_GUIDE.md) and
> [`docs/UPGRADE_RUNBOOK.md`](../../../docs/UPGRADE_RUNBOOK.md) for the
> authoritative, maintained versions. Tracked here (issue #358).

## Overview

This feature adds a timelocked, admin-gated upgrade path to the TipJar
Soroban contract (`contracts/tipjar`), enabling WASM bytecode replacement
without losing on-chain state, plus a version-gated storage migration hook.

---

## Functional Requirements

### 1. Timelocked Upgrade Pattern
- **1.1** `propose_upgrade(admin, new_wasm_hash)` MUST be admin-only and record a ledger-based unlock time computed from a timelock duration fixed at `init`.
- **1.2** `execute_upgrade()` MUST panic (`TimelockNotElapsed`) if called before the unlock ledger, and succeed at or after it.
- **1.3** `execute_upgrade()` MUST replace the executing WASM atomically via `env.deployer().update_current_contract_wasm(..)` — either the swap succeeds or the transaction reverts.
- **1.4** All instance and persistent storage entries MUST be preserved across an upgrade (a Soroban host guarantee, not something the contract implements itself).
- **1.5** `cancel_upgrade(admin)` MUST allow the admin to abort a pending proposal at any time before execution.

### 2. Storage Versioning
- **2.1** The contract MUST track a storage schema version in instance storage under `DataKey::DataVersion`, initialized at `init` to the build's `DATA_VERSION` constant.
- **2.2** A `migrate(admin)` entrypoint MUST advance `DataVersion` towards the deployed build's `DATA_VERSION`, applying any needed storage transformation.
- **2.3** `migrate()` MUST be idempotent and version-gated: a call when `DataVersion >= DATA_VERSION` MUST be a silent no-op (no panic, no event), safe to call any number of times.

### 3. Admin-Only Upgrade Authorization
- **3.1** `propose_upgrade`, `cancel_upgrade`, and `migrate` MUST all call `admin.require_auth()` and verify the caller matches stored `DataKey::Admin`.
- **3.2** Unauthorized calls MUST fail with `Error::Unauthorized`.
- **3.3** `execute_upgrade()` is intentionally **permissionless** (no caller argument) — the admin's authorization already happened at `propose_upgrade`, and the unlock ledger is public; see `docs/UPGRADE_GUIDE.md` for the rationale.

### 4. Two-Step Admin Transfer
- **4.1** `propose_admin(admin, new_admin)` MUST be admin-only and record `new_admin` in `DataKey::PendingAdmin` without changing `DataKey::Admin`.
- **4.2** `accept_admin(new_admin)` MUST require the caller to match the pending proposal exactly (`Error::NoPendingAdmin` otherwise) before updating `DataKey::Admin`.
- **4.3** The prior admin MUST retain full authority until `accept_admin` completes, so a typo'd or unreachable proposed address can never cause a permanent lockout.

### 5. Backward Compatibility
- **5.1** Adding new `DataKey` variants MUST NOT break existing storage reads — Soroban's `contracttype` derive encodes enum variants by name, not position, so this holds regardless of where a new variant is declared.
- **5.2** Removing or renaming existing `DataKey` variants, or changing the value type stored under an existing key, is PROHIBITED without a `migrate()` step.

### 6. Upgrade Testing
- **6.1** Tests MUST exercise a real WASM swap — deploy a genuinely distinct compiled binary (not a second Rust type in the same test binary) via `soroban_sdk::contractimport!`, upload it, and execute the upgrade — to prove `update_current_contract_wasm` and `migrate()` behave correctly rather than only unit-testing storage writes in isolation.
- **6.2** Tests MUST cover: balance/total preservation across the swap, the timelock boundary (unlock−1 panics, unlock succeeds), cancellation, non-admin rejection of `propose_upgrade`/`cancel_upgrade`, a second pending proposal being rejected, and `migrate()` idempotency across repeated invocations.

### 7. Rollback
- **7.1** There is no native on-chain rollback for a WASM swap. The supported procedure is: snapshot state before `execute_upgrade` (via `scripts/migrate`'s `export` subcommand), and if a rollback is needed, propose/timelock/execute the old WASM hash again through the same flow — treated as a forward action, not an undo.
- **7.2** `docs/UPGRADE_RUNBOOK.md` documents the full procedure end-to-end, including this rollback stance.

### 8. Events
- **8.1** Every state transition in the upgrade and admin-transfer lifecycle MUST emit an event: `UpgradeProposed`, `UpgradeExecuted`, `UpgradeCancelled`, `Migrated`, `AdminTransferProposed`, `AdminTransferAccepted`. See `docs/EVENTS.md` for exact topic/data shapes.

### 9. One Canonical Path
- **9.1** No other upgrade mechanism may compete with this one for `contracts/tipjar`. `migrations/upgrade_v1_to_v2.rs` (an orphaned, unreferenced fixture describing a schema that never matched any real contract) has been deleted. `contracts/tipjar-legacy`'s own `upgrade()`/`get_version()`/`migrate_state()` — a separate, non-timelocked mechanism on an unrelated frozen reference contract kept alive only for `simulator`/`tools/gas-estimator` tooling — are explicitly documented as retired in favor of this one; see the module doc at the top of `contracts/tipjar-legacy/src/lib.rs`.

---

## Non-Functional Requirements

- `DataKey::DataVersion` and the upgrade-lifecycle counters fit in `u32`.
- The timelock duration is fixed at `init` (no entrypoint to change it later) — a fresh instance is the only way to run under a different delay.
