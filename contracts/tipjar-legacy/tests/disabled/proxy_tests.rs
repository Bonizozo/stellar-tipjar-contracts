#![cfg(test)]

extern crate std;

use soroban_sdk::{Address, BytesN, Env};
use tipjar::{TipJarContract, TipJarContractClient};

fn setup() -> (Env, TipJarContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TipJarContract);
    let client = TipJarContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract(token_admin);
    client.init(&admin, &0u32, &0u64);
    client.add_token(&admin, &token_id);

    (env, client, admin)
}

// ── initialisation ────────────────────────────────────────────────────────────

#[test]
fn test_proxy_init_stores_state() {
    let (env, client, admin) = setup();

    client.proxy_init(&admin, &0u64);

    let state = client.proxy_get_state().expect("state should exist");
    assert_eq!(state.admin, admin);
    assert_eq!(state.version, 0);
    assert!(!state.frozen);
}

#[test]
fn test_proxy_init_sets_storage_version_zero() {
    let (env, client, admin) = setup();

    client.proxy_init(&admin, &0u64);

    assert_eq!(client.proxy_get_storage_version(), 0);
}

#[test]
fn test_proxy_not_initialised_returns_none() {
    let (_env, client, _admin) = setup();
    assert!(client.proxy_get_state().is_none());
}

#[test]
#[should_panic(expected = "proxy already initialised")]
fn test_proxy_double_init_panics() {
    let (env, client, admin) = setup();
    client.proxy_init(&admin, &0u64);
    client.proxy_init(&admin, &0u64);
}

// ── upgrade guards ────────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "proxy is frozen")]
fn test_upgrade_blocked_when_frozen() {
    let (env, client, admin) = setup();
    client.proxy_init(&admin, &0u64);
    client.proxy_freeze(&admin);

    let hash = BytesN::from_array(&env, &[1u8; 32]);
    client.proxy_upgrade(&admin, &hash);
}

#[test]
#[should_panic(expected = "upgrade time-lock active")]
fn test_upgrade_blocked_by_time_lock() {
    let (env, client, admin) = setup();
    // Set a 1-hour delay.
    client.proxy_init(&admin, &3600u64);

    let hash = BytesN::from_array(&env, &[1u8; 32]);
    client.proxy_upgrade(&admin, &hash);
}

#[test]
#[should_panic(expected = "not proxy admin")]
fn test_upgrade_rejected_for_non_admin() {
    let (env, client, admin) = setup();
    client.proxy_init(&admin, &0u64);

    let stranger = Address::generate(&env);
    let hash = BytesN::from_array(&env, &[1u8; 32]);
    client.proxy_upgrade(&stranger, &hash);
}

// ── freeze ────────────────────────────────────────────────────────────────────

#[test]
fn test_freeze_sets_frozen_flag() {
    let (env, client, admin) = setup();
    client.proxy_init(&admin, &0u64);
    client.proxy_freeze(&admin);

    let state = client.proxy_get_state().unwrap();
    assert!(state.frozen);
}

#[test]
#[should_panic(expected = "not proxy admin")]
fn test_freeze_rejected_for_non_admin() {
    let (env, client, admin) = setup();
    client.proxy_init(&admin, &0u64);

    let stranger = Address::generate(&env);
    client.proxy_freeze(&stranger);
}

// ── set_upgrade_delay ─────────────────────────────────────────────────────────

#[test]
fn test_set_upgrade_delay_updates_upgrade_after() {
    let (env, client, admin) = setup();
    client.proxy_init(&admin, &0u64);

    // Advance ledger time.
    env.ledger().with_mut(|l| l.timestamp = 1_000);
    client.proxy_set_upgrade_delay(&admin, &500u64);

    let state = client.proxy_get_state().unwrap();
    assert_eq!(state.upgrade_after, 1_500);
}

// ── transfer_admin ────────────────────────────────────────────────────────────

#[test]
fn test_transfer_admin_changes_admin() {
    let (env, client, admin) = setup();
    client.proxy_init(&admin, &0u64);

    let new_admin = Address::generate(&env);
    client.proxy_transfer_admin(&admin, &new_admin);

    let state = client.proxy_get_state().unwrap();
    assert_eq!(state.admin, new_admin);
}

#[test]
#[should_panic(expected = "not proxy admin")]
fn test_transfer_admin_rejected_for_non_admin() {
    let (env, client, admin) = setup();
    client.proxy_init(&admin, &0u64);

    let stranger = Address::generate(&env);
    client.proxy_transfer_admin(&stranger, &stranger);
}
