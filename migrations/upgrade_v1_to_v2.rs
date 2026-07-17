//! Migration: v1 → v2
//!
//! This file documents and implements any storage-layout changes introduced
//! between contract version 1 and version 2.
//!
//! ## When to add a migration
//!
//! Soroban preserves all storage entries across a WASM upgrade, so purely
//! additive changes (new `DataKey` variants, new functions) need no migration.
//! A migration is only required when:
//!
//! - The *type* stored under an existing key changes.
//! - A key is renamed (old key must be read and re-written under the new name).
//! - Derived/cached values need to be recomputed from existing data.
//!
//! ## How to run a migration
//!
//! 1. Include this logic in the new WASM (v2).
//! 2. After `upgrade()` succeeds, call `migrate_v1_to_v2` once:
//!    ```bash
//!    stellar contract invoke --id <contract_id> \
//!        --source <admin-key> --network testnet \
//!        -- migrate_v1_to_v2
//!    ```
//! 3. The function is idempotent — calling it twice is safe.
//!
//! ## v1 → v2 changes (example)
//!
//! In this example v2 introduces `DataKey::CreatorTipCount(Address)` which
//! must be back-filled from existing `TipRecord` entries.  Adjust the body
//! below to match the actual schema delta for your release.

use soroban_sdk::{symbol_short, Address, Env};

use crate::{DataKey, TipJarError};

/// Key used to record that this migration has already run (idempotency guard).
const MIGRATION_FLAG: &str = "mig_v1v2";

/// Back-fills any storage changes introduced in v2.
///
/// Must be called once by the admin after upgrading to v2 WASM.
/// Subsequent calls are no-ops.
pub fn migrate_v1_to_v2(env: &Env, admin: Address) {
    admin.require_auth();

    // Verify caller is the stored admin.
    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| soroban_sdk::panic_with_error!(env, TipJarError::Unauthorized));
    if admin != stored_admin {
        soroban_sdk::panic_with_error!(env, TipJarError::Unauthorized);
    }

    // Idempotency: skip if already applied.
    let flag_key = soroban_sdk::symbol_short!("migv1v2");
    if env.storage().instance().has(&flag_key) {
        return;
    }

    // -----------------------------------------------------------------------
    // Place actual migration logic here.
    //
    // Example: ensure ContractVersion is set (it defaults to 0 via
    // `get_version`, but we can explicitly record it was migrated to v2).
    // -----------------------------------------------------------------------
    env.storage()
        .instance()
        .set(&DataKey::ContractVersion, &2u32);

    // Mark migration as complete.
    env.storage().instance().set(&flag_key, &true);

    env.events()
        .publish((symbol_short!("migrated"),), (2u32,));
}
