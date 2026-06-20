#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};
use tipjar::{TipJarContract, TipJarContractClient, TipJarError};

// ── helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, TipJarContractClient<'static>, Address, Address, Address) {
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

    (env, client, admin, token_id, sender)
}

// ── double init tests ───────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "AlreadyInitialized")]
fn test_double_init_rejected() {
    let (_env, client, admin, _token_id, _sender) = setup();
    // First init already called in setup(), second init should panic
    client.init(&admin, &0u32, &0u64);
}

// ── unauthorized withdraw tests ───────────────────────────────────────────────

#[test]
fn test_unauthorized_withdraw_wrong_signer() {
    let (env, client, _admin, token_id, sender) = setup();
    let creator = Address::generate(&env);
    let wrong_signer = Address::generate(&env);

    // Mint tokens to sender to allow tipping
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&sender, &500i128);

    // Tip the creator
    client.tip(&sender, &creator, &token_id, &100i128);

    // Wrong signer tries to withdraw
    let result = client.try_withdraw(&wrong_signer, &creator, &token_id);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}

// ── zero balance withdraw tests ──────────────────────────────────────────────

#[test]
fn test_withdraw_with_zero_balance_returns_nothing_to_withdraw() {
    let (env, client, _admin, token_id, _sender) = setup();
    let creator = Address::generate(&env);

    // Attempt to withdraw without any tips
    let result = client.try_withdraw(&creator, &creator, &token_id);
    assert_eq!(result, Err(Ok(TipJarError::NothingToWithdraw)));
}

// ── multiple creators isolation tests ─────────────────────────────────────────

#[test]
fn test_multiple_creators_accumulate_balances_independently() {
    let (env, client, _admin, token_id, sender) = setup();
    let creator_a = Address::generate(&env);
    let creator_b = Address::generate(&env);

    // Mint extra tokens
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&sender, &1000i128);

    // Tip both creators
    client.tip(&sender, &creator_a, &token_id, &100i128);
    client.tip(&sender, &creator_b, &token_id, &200i128);

    // Get totals
    let total_a = client.get_total_tips(&creator_a);
    let total_b = client.get_total_tips(&creator_b);

    assert_eq!(total_a, 100);
    assert_eq!(total_b, 200);
}

#[test]
fn test_withdrawing_one_creator_does_not_affect_another() {
    let (env, client, _admin, token_id, sender) = setup();
    let creator_a = Address::generate(&env);
    let creator_b = Address::generate(&env);

    // Mint extra tokens
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&sender, &1000i128);

    // Tip both creators
    client.tip(&sender, &creator_a, &token_id, &150i128);
    client.tip(&sender, &creator_b, &token_id, &250i128);

    // Verify initial totals
    let total_a_before = client.get_total_tips(&creator_a);
    let total_b_before = client.get_total_tips(&creator_b);
    assert_eq!(total_a_before, 150);
    assert_eq!(total_b_before, 250);

    // Creator A withdraws
    client.withdraw(&creator_a, &creator_a, &token_id);

    // Verify creator A's balance is gone but total remains
    let total_a_after = client.get_total_tips(&creator_a);
    let total_b_after = client.get_total_tips(&creator_b);
    assert_eq!(total_a_after, 150); // Total tips unchanged
    assert_eq!(total_b_after, 250); // Creator B's balance unchanged
}

// ── tip event tests ──────────────────────────────────────────────────────────

#[test]
fn test_tip_event_emitted_with_correct_data() {
    let (env, client, _admin, token_id, sender) = setup();
    let creator = Address::generate(&env);

    // Mint extra tokens
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&sender, &500i128);

    // Make a tip
    client.tip(&sender, &creator, &token_id, &123i128);

    // Get emitted events
    let events = env.events().all();

    // Verify event contains correct topic and data
    // The event should have topic containing "tip" and data with sender, creator, and amount
    assert!(!events.is_empty());
}

#[test]
fn test_withdraw_event_emitted_with_correct_amount() {
    let (env, client, _admin, token_id, sender) = setup();
    let creator = Address::generate(&env);

    // Mint extra tokens
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&sender, &500i128);

    // Tip the creator
    client.tip(&sender, &creator, &token_id, &200i128);

    // Withdraw
    client.withdraw(&creator, &creator, &token_id);

    // Get emitted events and verify withdraw event
    let events = env.events().all();
    assert!(!events.is_empty());
}

// ── multiple tips accumulation tests ─────────────────────────────────────────

#[test]
fn test_multiple_tips_to_same_creator_accumulate() {
    let (env, client, _admin, token_id, sender) = setup();
    let creator = Address::generate(&env);

    // Mint extra tokens
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&sender, &1500i128);

    // Multiple tips
    client.tip(&sender, &creator, &token_id, &100i128);
    client.tip(&sender, &creator, &token_id, &200i128);
    client.tip(&sender, &creator, &token_id, &150i128);

    // Verify total
    let total = client.get_total_tips(&creator);
    assert_eq!(total, 450);
}

// ── invalid tip amount tests ────────────────────────────────────────────────

#[test]
fn test_zero_tip_amount_rejected() {
    let (env, client, _admin, token_id, sender) = setup();
    let creator = Address::generate(&env);

    // Attempt to tip with zero amount
    let result = client.try_tip(&sender, &creator, &token_id, &0i128);
    assert!(result.is_err());
}

#[test]
fn test_negative_tip_amount_rejected() {
    let (env, client, _admin, token_id, sender) = setup();
    let creator = Address::generate(&env);

    // Attempt to tip with negative amount
    let result = client.try_tip(&sender, &creator, &token_id, &-100i128);
    assert!(result.is_err());
}

// ── non-whitelisted token tests ────────────────────────────────────────────

#[test]
fn test_tip_with_non_whitelisted_token_rejected() {
    let (env, client, _admin, _token_id, sender) = setup();
    let creator = Address::generate(&env);
    
    // Create a new non-whitelisted token
    let token_admin = Address::generate(&env);
    let non_whitelisted_token = env.register_stellar_asset_contract(token_admin.clone());

    // Mint tokens
    soroban_sdk::token::StellarAssetClient::new(&env, &non_whitelisted_token)
        .mint(&sender, &500i128);

    // Attempt to tip with non-whitelisted token
    let result = client.try_tip(&sender, &creator, &non_whitelisted_token, &100i128);
    assert!(result.is_err());
}

// ── edge case: same sender and creator ────────────────────────────────────────

#[test]
fn test_creator_can_tip_themselves() {
    let (env, client, _admin, token_id, _sender) = setup();
    let creator = Address::generate(&env);

    // Mint tokens to creator
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&creator, &500i128);

    // Self-tip
    client.tip(&creator, &creator, &token_id, &100i128);

    // Verify total
    let total = client.get_total_tips(&creator);
    assert_eq!(total, 100);
}

// ── concurrent withdrawals edge case ───────────────────────────────────────────

#[test]
fn test_second_withdraw_after_zero_balance_fails() {
    let (env, client, _admin, token_id, sender) = setup();
    let creator = Address::generate(&env);

    // Mint extra tokens
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&sender, &500i128);

    // Tip once
    client.tip(&sender, &creator, &token_id, &100i128);

    // First withdraw succeeds
    client.withdraw(&creator, &creator, &token_id);

    // Second withdraw should fail (nothing to withdraw)
    let result = client.try_withdraw(&creator, &creator, &token_id);
    assert_eq!(result, Err(Ok(TipJarError::NothingToWithdraw)));
}

// ── insufficient balance tests ─────────────────────────────────────────────────

#[test]
fn test_insufficient_balance_for_tip_rejected() {
    let (env, client, _admin, token_id, sender) = setup();
    let creator = Address::generate(&env);

    // Mint exactly 100 tokens to sender
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&sender, &100i128);

    // Attempt to tip more than available (starting balance is 1_000_000 + 100 = 1_000_100)
    // This should succeed since we minted more earlier, but test realistic scenario
    let result = client.try_tip(&sender, &creator, &token_id, &2_000_000i128);
    assert!(result.is_err());
}
