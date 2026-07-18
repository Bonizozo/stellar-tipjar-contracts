#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};
use tipjar::{DataKey, TipJarContract, TipJarContractClient, TipJarError, TipMetadata};

// ── helpers ──────────────────────────────────────────────────────────────────

fn setup() -> (
    Env,
    TipJarContractClient<'static>,
    Address,
    Address,
    Address,
) {
    setup_with_expiry(0)
}

fn setup_with_expiry(
    expiry_seconds: u64,
) -> (
    Env,
    TipJarContractClient<'static>,
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

    // Deploy a mock token.
    let token_id = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin, &0u32, &expiry_seconds);
    client.add_token(&admin, &token_id);

    // Fund sender with tokens via the asset contract.
    let sender = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&sender, &1_000_000i128);

    (env, client, sender, Address::generate(&env), token_id)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_tip_with_message_stores_metadata() {
    let (env, client, sender, creator, token) = setup();

    let msg = String::from_str(&env, "Great content, keep it up!");
    let tip_index =
        client.tip_with_message(&sender, &creator, &token, &500i128, &Some(msg.clone()));

    assert_eq!(tip_index, 0);

    let history = client.get_tip_history(&creator, &10u32);
    assert_eq!(history.len(), 1);

    let meta: TipMetadata = history.get(0).unwrap();
    assert_eq!(meta.sender, sender);
    assert_eq!(meta.amount, 500i128);
    assert_eq!(meta.message, Some(msg));
}

#[test]
fn test_tip_without_message_stores_none() {
    let (env, client, sender, creator, token) = setup();

    client.tip_with_message(&sender, &creator, &token, &100i128, &None);

    let history = client.get_tip_history(&creator, &10u32);
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(0).unwrap().message, None);
}

#[test]
fn test_message_exactly_200_chars_accepted() {
    let (env, client, sender, creator, token) = setup();

    // Build a 200-character ASCII string.
    let s: std::string::String = "a".repeat(200);
    let msg = String::from_str(&env, &s);

    // Should not panic.
    client.tip_with_message(&sender, &creator, &token, &100i128, &Some(msg));
    assert_eq!(client.get_tip_history(&creator, &1u32).len(), 1);
}

#[test]
#[should_panic]
fn test_message_exceeding_200_chars_rejected() {
    let (env, client, sender, creator, token) = setup();

    let s: std::string::String = "a".repeat(201);
    let msg = String::from_str(&env, &s);

    client.tip_with_message(&sender, &creator, &token, &100i128, &Some(msg));
}

#[test]
fn test_tip_history_returned_newest_first() {
    let (env, client, sender, creator, token) = setup();

    // Mint more tokens for multiple tips.
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_client.mint(&sender, &10_000i128);

    for i in 0u32..5 {
        let msg_str = std::format!("tip {}", i);
        let msg = String::from_str(&env, &msg_str);
        // Advance ledger time so timestamps differ.
        env.ledger().with_mut(|l| l.timestamp += 1);
        client.tip_with_message(&sender, &creator, &token, &100i128, &Some(msg));
    }

    let history = client.get_tip_history(&creator, &5u32);
    assert_eq!(history.len(), 5);

    // Newest first: tip 4 should be at index 0.
    let first_msg = history.get(0).unwrap().message.unwrap();
    assert_eq!(first_msg, String::from_str(&env, "tip 4"));
    let last_msg = history.get(4).unwrap().message.unwrap();
    assert_eq!(last_msg, String::from_str(&env, "tip 0"));
}

#[test]
fn test_tip_history_limit_respected() {
    let (env, client, sender, creator, token) = setup();

    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_client.mint(&sender, &10_000i128);

    for _ in 0..10 {
        client.tip_with_message(&sender, &creator, &token, &50i128, &None);
    }

    let history = client.get_tip_history(&creator, &3u32);
    assert_eq!(history.len(), 3);
}

#[test]
fn test_utf8_emoji_message_accepted() {
    let (env, client, sender, creator, token) = setup();

    // 5 emoji = 5 chars but many bytes each — must pass the 200-char limit.
    let msg = String::from_str(&env, "🎉🚀💎🌟🔥");
    client.tip_with_message(&sender, &creator, &token, &100i128, &Some(msg.clone()));

    let history = client.get_tip_history(&creator, &1u32);
    assert_eq!(history.get(0).unwrap().message.unwrap(), msg);
}

#[test]
#[should_panic]
fn test_emoji_message_exceeding_200_chars_rejected() {
    let (env, client, sender, creator, token) = setup();

    // 201 emoji = 201 chars (but many more bytes).
    let s: std::string::String = "🎉".repeat(201);
    let msg = String::from_str(&env, &s);

    client.tip_with_message(&sender, &creator, &token, &100i128, &Some(msg));
}

#[test]
fn test_storage_efficiency_tip_count_increments() {
    let (env, client, sender, creator, token) = setup();

    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_client.mint(&sender, &10_000i128);

    for i in 0u64..3 {
        client.tip_with_message(&sender, &creator, &token, &100i128, &None);
        // Verify the count key matches expected value.
        let count: u64 = env.as_contract(&client.address, || {
            env.storage()
                .persistent()
                .get::<DataKey, u64>(&DataKey::TipCount(creator.clone()))
                .unwrap_or(0)
        });
        assert_eq!(count, i + 1);
    }
}

#[test]
fn test_existing_tip_behavior_unaffected() {
    let (env, client, sender, creator, token) = setup();

    // The original `tip` function should still work independently.
    client.tip(&sender, &creator, &token, &200i128);

    // tip_with_message history should be empty (tip() doesn't write TipHistory).
    let history = client.get_tip_history(&creator, &10u32);
    assert_eq!(history.len(), 0);

    // But balance should be credited.
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 200i128);
}

#[test]
fn test_process_expired_tips_refunds_unclaimed_locked_tip() {
    let (env, client, sender, creator, token) = setup_with_expiry(100);

    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);

    let current_time = env.ledger().timestamp();
    let unlock_time = current_time + 1_000;
    let lock_id = client.tip_locked(&sender, &creator, &token, &500i128, &unlock_time);

    assert_eq!(lock_id, 0);
    assert_eq!(client.get_refund_window(), 100u64);

    // Advance time past the expiry window but before unlock time.
    env.ledger().with_mut(|ledger| ledger.timestamp += 101);

    let refunded_count = client.process_expired_tips();
    assert_eq!(refunded_count, 1);

    // Sender should be refunded and lock should be removed.
    assert_eq!(token_client.balance(&sender), 1_000i128);
    let result = client.try_tip_locked(&sender, &creator, &token, &500i128, &unlock_time);
    assert!(
        result.is_ok(),
        "locked tip should still be creatable after refund"
    );
}
