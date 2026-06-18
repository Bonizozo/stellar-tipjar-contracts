#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};
use tipjar::{TipJarContract, TipJarContractClient, TipJarError};

// ── helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (
    Env,
    TipJarContractClient<'static>,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TipJarContract);
    let client = TipJarContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin, &0u32, &0u64);
    client.add_token(&admin, &token_id);

    let sender = Address::generate(&env);
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&sender, &1_000_000i128);

    (
        env,
        client,
        admin,
        sender,
        Address::generate(&env),
        token_id,
    )
}

// ── get_withdrawal_limits ─────────────────────────────────────────────────────

#[test]
fn test_get_withdrawal_limits_defaults_to_zero() {
    let (_env, client, _admin, _sender, creator, _token) = setup();
    let limits = client.get_withdrawal_limits(&creator);
    assert_eq!(limits.daily_limit, 0);
    assert_eq!(limits.cooldown_seconds, 0);
}

// ── set_withdrawal_limits ─────────────────────────────────────────────────────

#[test]
fn test_set_withdrawal_limits_persists() {
    let (_env, client, admin, _sender, creator, _token) = setup();
    client.set_withdrawal_limits(&admin, &creator, &500i128, &3600u64);
    let limits = client.get_withdrawal_limits(&creator);
    assert_eq!(limits.daily_limit, 500);
    assert_eq!(limits.cooldown_seconds, 3600);
}

#[test]
fn test_set_withdrawal_limits_unauthorized() {
    let (env, client, _admin, _sender, creator, _token) = setup();
    let non_admin = Address::generate(&env);
    let result = client.try_set_withdrawal_limits(&non_admin, &creator, &500i128, &0u64);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}

// ── set_default_withdrawal_limits ─────────────────────────────────────────────

#[test]
fn test_default_limits_applied_when_no_per_creator_config() {
    let (_env, client, admin, _sender, creator, _token) = setup();
    client.set_default_withdrawal_limits(&admin, &200i128, &0u64);
    let limits = client.get_withdrawal_limits(&creator);
    assert_eq!(limits.daily_limit, 200);
}

#[test]
fn test_per_creator_limits_override_defaults() {
    let (_env, client, admin, _sender, creator, _token) = setup();
    client.set_default_withdrawal_limits(&admin, &200i128, &0u64);
    client.set_withdrawal_limits(&admin, &creator, &999i128, &0u64);
    let limits = client.get_withdrawal_limits(&creator);
    assert_eq!(limits.daily_limit, 999);
}

// ── daily limit enforcement ───────────────────────────────────────────────────

#[test]
fn test_withdraw_within_daily_limit_succeeds() {
    let (_env, client, admin, sender, creator, token) = setup();
    client.tip(&sender, &creator, &token, &300i128);
    client.set_withdrawal_limits(&admin, &creator, &300i128, &0u64);
    client.withdraw(&creator, &token); // should not panic
}

#[test]
fn test_withdraw_exceeds_daily_limit_fails() {
    let (_env, client, admin, sender, creator, token) = setup();
    client.tip(&sender, &creator, &token, &500i128);
    client.set_withdrawal_limits(&admin, &creator, &100i128, &0u64);
    let result = client.try_withdraw(&creator, &token);
    assert_eq!(result, Err(Ok(TipJarError::DailyLimitExceeded)));
}

#[test]
fn test_daily_limit_resets_after_24_hours() {
    let (env, client, admin, sender, creator, token) = setup();

    // Fund two separate withdrawals.
    soroban_sdk::token::StellarAssetClient::new(&env, &token).mint(&sender, &1_000_000i128);

    env.ledger().with_mut(|l| l.timestamp = 1_000);
    client.tip(&sender, &creator, &token, &100i128);
    client.set_withdrawal_limits(&admin, &creator, &100i128, &0u64);
    client.withdraw(&creator, &token);

    // Tip again and advance past 24 h.
    client.tip(&sender, &creator, &token, &100i128);
    env.ledger().with_mut(|l| l.timestamp = 1_000 + 86_401);
    client.withdraw(&creator, &token); // window reset — should succeed
}

// ── cooldown enforcement ──────────────────────────────────────────────────────

#[test]
fn test_withdraw_before_cooldown_fails() {
    let (env, client, admin, sender, creator, token) = setup();

    env.ledger().with_mut(|l| l.timestamp = 1_000);
    client.tip(&sender, &creator, &token, &100i128);
    client.set_withdrawal_limits(&admin, &creator, &0i128, &3600u64);
    client.withdraw(&creator, &token);

    // Tip again and try to withdraw immediately.
    soroban_sdk::token::StellarAssetClient::new(&env, &token).mint(&sender, &100i128);
    client.tip(&sender, &creator, &token, &100i128);
    let result = client.try_withdraw(&creator, &token);
    assert_eq!(result, Err(Ok(TipJarError::WithdrawalCooldown)));
}

#[test]
fn test_withdraw_after_cooldown_succeeds() {
    let (env, client, admin, sender, creator, token) = setup();

    env.ledger().with_mut(|l| l.timestamp = 1_000);
    client.tip(&sender, &creator, &token, &100i128);
    client.set_withdrawal_limits(&admin, &creator, &0i128, &3600u64);
    client.withdraw(&creator, &token);

    soroban_sdk::token::StellarAssetClient::new(&env, &token).mint(&sender, &100i128);
    client.tip(&sender, &creator, &token, &100i128);
    env.ledger().with_mut(|l| l.timestamp = 1_000 + 3_601);
    client.withdraw(&creator, &token); // cooldown elapsed — should succeed
}

// ── emergency_withdraw ────────────────────────────────────────────────────────

#[test]
fn test_emergency_withdraw_bypasses_limits() {
    let (_env, client, admin, sender, creator, token) = setup();
    client.tip(&sender, &creator, &token, &1_000i128);
    // Set a very restrictive daily limit.
    client.set_withdrawal_limits(&admin, &creator, &1i128, &0u64);
    // Emergency withdraw should succeed despite the limit.
    client.emergency_withdraw(&admin, &creator, &token);
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 0);
}

#[test]
fn test_emergency_withdraw_nothing_to_withdraw() {
    let (_env, client, admin, _sender, creator, token) = setup();
    let result = client.try_emergency_withdraw(&admin, &creator, &token);
    assert_eq!(result, Err(Ok(TipJarError::NothingToWithdraw)));
}

#[test]
fn test_emergency_withdraw_unauthorized() {
    let (env, client, _admin, sender, creator, token) = setup();
    client.tip(&sender, &creator, &token, &100i128);
    let non_admin = Address::generate(&env);
    let result = client.try_emergency_withdraw(&non_admin, &creator, &token);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}
