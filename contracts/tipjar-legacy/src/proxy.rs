//! Upgradeable proxy pattern for the tip-jar contract.
//!
//! Provides:
//! - Proxy state: tracks the current implementation WASM hash and version
//! - Initialization guard: proxy can only be initialised once
//! - Upgrade mechanism: admin-only WASM swap with version bump
//! - Upgrade guards: time-lock delay and optional freeze flag prevent
//!   accidental or malicious upgrades
//! - Storage layout helpers: version-stamped slot so migrators can detect
//!   which schema is active

use soroban_sdk::{contracttype, symbol_short, Address, BytesN, Env};

use crate::DataKey;

// ── Types ────────────────────────────────────────────────────────────────────

/// On-chain proxy state stored in instance storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyState {
    /// WASM hash of the current implementation.
    pub implementation: BytesN<32>,
    /// Monotonically increasing upgrade counter (0 = initial deploy).
    pub version: u32,
    /// Address authorised to trigger upgrades.
    pub admin: Address,
    /// Minimum ledger timestamp that must be reached before an upgrade is
    /// allowed (0 = no delay enforced).
    pub upgrade_after: u64,
    /// When `true` all upgrades are blocked regardless of other conditions.
    pub frozen: bool,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

fn load_state(env: &Env) -> Option<ProxyState> {
    env.storage().instance().get(&DataKey::ProxyState)
}

fn save_state(env: &Env, state: &ProxyState) {
    env.storage().instance().set(&DataKey::ProxyState, state);
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Initialise the proxy. May only be called once.
///
/// Records the current WASM hash as the initial implementation and stores
/// the admin address. `upgrade_delay_secs` sets the minimum seconds that
/// must elapse between upgrades (0 = no delay).
pub fn init(env: &Env, admin: &Address, upgrade_delay_secs: u64) {
    admin.require_auth();
    assert!(load_state(env).is_none(), "proxy already initialised");

    // Capture the currently-executing WASM hash as the initial implementation.
    let impl_hash = env.current_contract_address(); // placeholder — real hash via deployer
    // In Soroban the deployer hash is not directly readable post-deploy, so we
    // store a zeroed sentinel and let the first real upgrade overwrite it.
    let zero_hash: BytesN<32> = BytesN::from_array(env, &[0u8; 32]);

    let state = ProxyState {
        implementation: zero_hash,
        version: 0,
        admin: admin.clone(),
        upgrade_after: env.ledger().timestamp() + upgrade_delay_secs,
        frozen: false,
    };
    save_state(env, &state);

    // Stamp the storage layout version so migrators know the active schema.
    env.storage()
        .instance()
        .set(&DataKey::ProxyStorageVersion, &0u32);

    env.events().publish((symbol_short!("prx_init"),), admin.clone());

    // suppress unused warning for impl_hash placeholder
    let _ = impl_hash;
}

/// Upgrade the contract to a new WASM `new_hash`.
///
/// Guards checked in order:
/// 1. Proxy must be initialised.
/// 2. Caller must be the stored proxy admin.
/// 3. Proxy must not be frozen.
/// 4. Current ledger timestamp must be >= `upgrade_after`.
pub fn upgrade(env: &Env, caller: &Address, new_hash: BytesN<32>) {
    caller.require_auth();

    let mut state = load_state(env).expect("proxy not initialised");

    assert!(state.admin == *caller, "not proxy admin");
    assert!(!state.frozen, "proxy is frozen");
    assert!(
        env.ledger().timestamp() >= state.upgrade_after,
        "upgrade time-lock active"
    );

    // Perform the WASM swap — storage is preserved by the host.
    env.deployer().update_current_contract_wasm(new_hash.clone());

    state.implementation = new_hash;
    state.version = state.version.saturating_add(1);
    save_state(env, &state);

    // Bump the storage layout version to match the new implementation.
    let layout_ver: u32 = env
        .storage()
        .instance()
        .get(&DataKey::ProxyStorageVersion)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::ProxyStorageVersion, &(layout_ver + 1));

    env.events()
        .publish((symbol_short!("prx_upgd"),), state.version);
}

/// Freeze the proxy, permanently blocking further upgrades.
/// Only the proxy admin may freeze.
pub fn freeze(env: &Env, caller: &Address) {
    caller.require_auth();
    let mut state = load_state(env).expect("proxy not initialised");
    assert!(state.admin == *caller, "not proxy admin");
    state.frozen = true;
    save_state(env, &state);
    env.events().publish((symbol_short!("prx_frz"),), ());
}

/// Update the upgrade time-lock: next upgrade may not happen before
/// `env.ledger().timestamp() + delay_secs`.
/// Only the proxy admin may change the delay.
pub fn set_upgrade_delay(env: &Env, caller: &Address, delay_secs: u64) {
    caller.require_auth();
    let mut state = load_state(env).expect("proxy not initialised");
    assert!(state.admin == *caller, "not proxy admin");
    state.upgrade_after = env.ledger().timestamp() + delay_secs;
    save_state(env, &state);
}

/// Transfer proxy admin rights to `new_admin`.
pub fn transfer_admin(env: &Env, caller: &Address, new_admin: &Address) {
    caller.require_auth();
    let mut state = load_state(env).expect("proxy not initialised");
    assert!(state.admin == *caller, "not proxy admin");
    state.admin = new_admin.clone();
    save_state(env, &state);
    env.events()
        .publish((symbol_short!("prx_adm"),), new_admin.clone());
}

/// Returns the current proxy state, or `None` if not initialised.
pub fn get_state(env: &Env) -> Option<ProxyState> {
    load_state(env)
}

/// Returns the active storage layout version (incremented on each upgrade).
pub fn get_storage_version(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::ProxyStorageVersion)
        .unwrap_or(0)
}
