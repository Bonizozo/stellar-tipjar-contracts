use soroban_sdk::{symbol_short, BytesN, Env};

use crate::{storage, SystemError};

/// Performs an admin-authorized WASM upgrade and bumps the on-chain version.
///
/// # Authorization
/// Requires auth from the stored `DataKey::Admin` address.
///
/// # Upgrade flow
/// 1. Upload new WASM and note the returned hash:
///    ```bash
///    stellar contract upload \
///        --wasm target/wasm32v1-none/release/tipjar.wasm \
///        --source <admin-key> --network testnet
///    ```
/// 2. Invoke this function:
///    ```bash
///    stellar contract invoke --id <contract_id> \
///        --source <admin-key> --network testnet \
///        -- upgrade --new_wasm_hash <hash>
///    ```
/// 3. Confirm the new version:
///    ```bash
///    stellar contract invoke --id <contract_id> --network testnet -- get_version
///    ```
///
/// # Backward compatibility
/// - Adding new `DataKey` variants or contract functions is always safe.
/// - Changing the *type* of an existing key requires a migration function in
///   the new WASM (see `migrations/upgrade_v1_to_v2.rs` for an example).
/// - Removing a key variant orphans stored data; prefer deprecation.
///
/// # Rollback
/// Re-upload the previous WASM and call `upgrade` again with its hash.
/// The version counter will continue incrementing — it records history,
/// not a semantic version.
pub fn upgrade(env: &Env, new_wasm_hash: BytesN<32>) {
    // Admin-only gate.
    let admin: soroban_sdk::Address = env
        .storage()
        .instance()
        .get(&crate::DataKey::Admin)
        .unwrap_or_else(|| soroban_sdk::panic_with_error!(env, SystemError::UpgradeUnauthorized));
    admin.require_auth();

    let new_version = storage::get_contract_version(env).saturating_add(1);

    // Atomically swap the executing WASM. Storage is preserved by the host.
    env.deployer().update_current_contract_wasm(new_wasm_hash);

    // Persist the new version *after* the WASM swap so the new code sees it.
    storage::set_contract_version(env, new_version);

    env.events()
        .publish((symbol_short!("upgraded"),), (new_version,));
}

/// Returns the current contract version (0 before the first upgrade).
pub fn get_version(env: &Env) -> u32 {
    storage::get_contract_version(env)
}
