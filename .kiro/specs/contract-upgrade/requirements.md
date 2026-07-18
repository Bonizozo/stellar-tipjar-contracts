# Contract Upgrade and Migration System — Requirements

## Overview

This feature adds a secure, admin-controlled upgrade path to the TipJar Soroban contract, enabling WASM bytecode replacement without losing on-chain state.

---

## Functional Requirements

### 1. Upgradeable Contract Pattern
- **1.1** The contract MUST expose an `upgrade` function that replaces the executing WASM bytecode using Soroban's native upgrade mechanism.
- **1.2** The upgrade MUST be atomic: either the full WASM swap succeeds or the transaction reverts.
- **1.3** All instance and persistent storage entries MUST be preserved across an upgrade.

### 2. Version Management
- **2.1** The contract MUST track a monotonically increasing version number in instance storage under `DataKey::ContractVersion`.
- **2.2** The version MUST be incremented by 1 on every successful upgrade.
- **2.3** A `version` query function MUST return the current version (default `1` before any upgrade).

### 3. Admin-Only Upgrade Authorization
- **3.1** Only the stored contract administrator MAY invoke `upgrade`.
- **3.2** The `upgrade` function MUST call `admin.require_auth()` to enforce on-chain signature verification.
- **3.3** Unauthorized upgrade attempts MUST fail with `TipJarError::UpgradeUnauthorized`.

### 4. Backward Compatibility Checks
- **4.1** Adding new `DataKey` variants MUST NOT break existing storage reads.
- **4.2** Adding new contract functions MUST NOT affect existing function selectors.
- **4.3** Removing or renaming existing `DataKey` variants is PROHIBITED without a migration function.

### 5. Data Migration Scripts
- **5.1** A migration helper pattern MUST be documented so future upgrades can transform storage layout.
- **5.2** Migration logic SHOULD be idempotent (safe to run more than once).

### 6. Upgrade Testing Framework
- **6.1** Unit tests MUST verify that `upgrade` succeeds when called by the admin.
- **6.2** Unit tests MUST verify that `upgrade` fails when called by a non-admin.
- **6.3** Tests MUST confirm the version number increments after a successful upgrade.

### 7. Rollback Capability
- **7.1** The upgrade script MUST record the previous WASM hash before invoking `upgrade` so operators can re-upload and re-invoke with the old hash if needed.
- **7.2** Documentation MUST describe the rollback procedure.

### 8. State Preservation
- **8.1** After an upgrade, all creator balances, totals, milestones, and role assignments MUST remain intact.
- **8.2** The contract MUST emit an `upgrade` event containing the new version number.

### 9. Upgrade Documentation
- **9.1** A `docs/UPGRADE_GUIDE.md` file MUST document the end-to-end upgrade procedure.
- **9.2** The guide MUST cover: WASM upload, upgrade invocation, version verification, rollback, and access control requirements.

---

## Non-Functional Requirements

- The `upgrade` function MUST add no more than ~50 instructions of overhead to the happy path.
- The version counter MUST fit in a `u32` (supports ~4 billion upgrades).
- The upgrade script MUST be a POSIX-compatible Bash script requiring only the `stellar` CLI.
