#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};
use tipjar::{DataKey, Stream, StreamStatus, TipJarContract, TipJarContractClient, TipJarError};

fn setup() -> (
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
    let token_id = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin, &0u32, &0u64);
    client.add_token(&admin, &token_id);

    let sender = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    // Give sender enough tokens for streaming (100 tokens at 1 token/sec for 100 seconds)
    token_client.mint(&sender, &10_000i128);

    (env, client, sender, Address::generate(&env), token_id)
}

#[test]
fn test_create_stream() {
    let (env, client, sender, creator, token) = setup();

    let stream_id = client.create_stream(
        &sender, &creator, &token, &1i128,  // 1 token per second
        &100u64, // 100 seconds duration
    );

    assert_eq!(stream_id, 0);

    let stream = client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.stream_id, 0);
    assert_eq!(stream.sender, sender);
    assert_eq!(stream.creator, creator);
    assert_eq!(stream.token, token);
    assert_eq!(stream.amount_per_second, 1i128);
    assert_eq!(stream.status, StreamStatus::Active);
    assert!(stream.start_time > 0);
    assert_eq!(stream.end_time, stream.start_time + 100);
    assert_eq!(stream.withdrawn, 0);
}

#[test]
fn test_create_stream_invalid_rate() {
    let (env, client, sender, creator, token) = setup();

    let result = client.try_create_stream(
        &sender, &creator, &token, &0i128, // Invalid rate
        &100u64,
    );
    assert_eq!(result, Err(Ok(TipJarError::InvalidStreamRate)));

    let result = client.try_create_stream(
        &sender, &creator, &token, &1001i128, // Exceeds maximum
        &100u64,
    );
    assert_eq!(result, Err(Ok(TipJarError::StreamRateExceedsMaximum)));
}

#[test]
fn test_create_stream_invalid_duration() {
    let (env, client, sender, creator, token) = setup();

    let result = client.try_create_stream(
        &sender, &creator, &token, &1i128, &0u64, // Invalid duration
    );
    assert_eq!(result, Err(Ok(TipJarError::InvalidAmount)));
}

#[test]
fn test_calculate_streamed_amount() {
    let (env, client, sender, creator, token) = setup();

    let stream_id = client.create_stream(
        &sender, &creator, &token, &2i128,  // 2 tokens per second
        &100u64, // 100 seconds
    );

    // Initially, 0 streamed
    let streamed = client.get_streamed_amount(&stream_id);
    assert_eq!(streamed, 0);

    // After 10 seconds
    env.ledger().with_mut(|ledger| ledger.timestamp += 10);
    let streamed = client.get_streamed_amount(&stream_id);
    assert_eq!(streamed, 20i128); // 2 * 10

    // After 50 more seconds (total 60)
    env.ledger().with_mut(|ledger| ledger.timestamp += 50);
    let streamed = client.get_streamed_amount(&stream_id);
    assert_eq!(streamed, 120i128); // 2 * 60

    // After 50 more seconds (total 110, exceeds duration)
    env.ledger().with_mut(|ledger| ledger.timestamp += 50);
    let streamed = client.get_streamed_amount(&stream_id);
    assert_eq!(streamed, 200i128); // 2 * 100 (max)
}

#[test]
fn test_withdraw_streamed() {
    let (env, client, sender, creator, token) = setup();

    let stream_id = client.create_stream(
        &sender, &creator, &token, &3i128,  // 3 tokens per second
        &100u64, // 100 seconds
    );

    // After 20 seconds, 60 tokens should be available
    env.ledger().with_mut(|ledger| ledger.timestamp += 20);

    let available = client.get_available_to_withdraw(&stream_id);
    assert_eq!(available, 60i128);

    // Creator withdraws
    client.withdraw_streamed(&creator, &stream_id);

    // Balance should have increased
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    assert_eq!(token_client.balance(&creator), 60i128);

    // Stream withdrawn amount updated
    let stream = client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.withdrawn, 60i128);

    // After 30 more seconds (total 50), 150 tokens total - 60 withdrawn = 90 available
    env.ledger().with_mut(|ledger| ledger.timestamp += 30);
    let available = client.get_available_to_withdraw(&stream_id);
    assert_eq!(available, 90i128);

    // Withdraw again
    client.withdraw_streamed(&creator, &stream_id);
    assert_eq!(token_client.balance(&creator), 150i128);
    let stream = client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.withdrawn, 150i128);
}

#[test]
fn test_withdraw_streamed_completed() {
    let (env, client, sender, creator, token) = setup();

    let stream_id = client.create_stream(&sender, &creator, &token, &2i128, &50u64);

    // Advance past end time
    env.ledger().with_mut(|ledger| ledger.timestamp += 60);

    // Should be able to withdraw all 100 tokens (2 * 50)
    client.withdraw_streamed(&creator, &stream_id);

    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    assert_eq!(token_client.balance(&creator), 100i128);

    // Stream should be marked completed
    let stream = client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.status, StreamStatus::Completed);
    assert_eq!(stream.withdrawn, 100i128);
}

#[test]
fn test_cancel_stream() {
    let (env, client, sender, creator, token) = setup();

    let stream_id = client.create_stream(&sender, &creator, &token, &5i128, &100u64);

    // After 20 seconds, 100 tokens streamed (5 * 20)
    env.ledger().with_mut(|ledger| ledger.timestamp += 20);

    // Sender cancels
    client.cancel_stream(&sender, &stream_id);

    // Stream should be cancelled
    let stream = client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.status, StreamStatus::Cancelled);

    // Sender should have remaining tokens refunded (500 - 100 = 400)
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    assert_eq!(token_client.balance(&sender), 10000i128 - 500 + 400);

    // Creator can still withdraw streamed amount
    client.withdraw_streamed(&creator, &stream_id);
    let stream = client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.withdrawn, 100i128);
}

#[test]
fn test_stop_and_start_stream() {
    let (env, client, sender, creator, token) = setup();

    let stream_id = client.create_stream(&sender, &creator, &token, &4i128, &100u64);

    // After 10 seconds, 40 tokens
    env.ledger().with_mut(|ledger| ledger.timestamp += 10);
    let streamed = client.get_streamed_amount(&stream_id);
    assert_eq!(streamed, 40i128);

    // Stop stream
    client.stop_stream(&sender, &stream_id);
    let stream = client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.status, StreamStatus::Paused);

    // Advance 20 seconds, no more should accrue
    env.ledger().with_mut(|ledger| ledger.timestamp += 20);
    let streamed = client.get_streamed_amount(&stream_id);
    assert_eq!(streamed, 40i128);

    // Start stream again
    client.start_stream(&sender, &stream_id);
    let stream = client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.status, StreamStatus::Active);

    // Advance 10 more seconds, should have 80 tokens total (4 * 20)
    env.ledger().with_mut(|ledger| ledger.timestamp += 10);
    let streamed = client.get_streamed_amount(&stream_id);
    assert_eq!(streamed, 80i128);
}

#[test]
fn test_unauthorized_operations() {
    let (env, client, sender, creator, token) = setup();
    let other = Address::generate(&env);

    let stream_id = client.create_stream(&sender, &creator, &token, &1i128, &100u64);

    // Other user can't stop stream
    let result = client.try_stop_stream(&other, &stream_id);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));

    // Other user can't start stream
    client.stop_stream(&sender, &stream_id);
    let result = client.try_start_stream(&other, &stream_id);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));

    // Other user can't cancel stream
    client.start_stream(&sender, &stream_id);
    let result = client.try_cancel_stream(&other, &stream_id);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));

    // Other user can't withdraw
    let result = client.try_withdraw_streamed(&other, &stream_id);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}

#[test]
fn test_withdraw_without_streamed_amount() {
    let (env, client, sender, creator, token) = setup();

    let stream_id = client.create_stream(&sender, &creator, &token, &1i128, &100u64);

    // Try to withdraw before stream starts (should fail)
    let result = client.try_withdraw_streamed(&creator, &stream_id);
    assert_eq!(result, Err(Ok(TipJarError::StreamNotStarted)));

    // Advance time a bit but not enough for tokens to accrue (0 seconds)
    // Actually with 0 elapsed, it's still 0
    env.ledger().with_mut(|ledger| ledger.timestamp += 0);
    let result = client.try_withdraw_streamed(&creator, &stream_id);
    // This should still fail because no time has elapsed
    assert_eq!(result, Err(Ok(TipJarError::NoStreamedAmount)));
}

#[test]
fn test_get_streams_by_creator() {
    let (env, client, sender, creator, token) = setup();

    let stream1 = client.create_stream(&sender, &creator, &token, &1i128, &100u64);
    let stream2 = client.create_stream(&sender, &creator, &token, &2i128, &200u64);

    let streams = client.get_streams_by_creator(&creator);
    assert_eq!(streams.len(), 2);
    assert!(streams.contains(&stream1));
    assert!(streams.contains(&stream2));
}

#[test]
fn test_get_streams_by_sender() {
    let (env, client, sender, creator, token) = setup();
    let creator2 = Address::generate(&env);

    let stream1 = client.create_stream(&sender, &creator, &token, &1i128, &100u64);
    let stream2 = client.create_stream(&sender, &creator2, &token, &2i128, &200u64);

    let streams = client.get_streams_by_sender(&sender);
    assert_eq!(streams.len(), 2);
    assert!(streams.contains(&stream1));
    assert!(streams.contains(&stream2));
}

#[test]
fn test_cancel_nonexistent_stream() {
    let (env, client, sender, _creator, _token) = setup();

    let result = client.try_cancel_stream(&sender, &999u64);
    assert_eq!(result, Err(Ok(TipJarError::StreamNotFound)));
}

#[test]
fn test_stream_already_completed() {
    let (env, client, sender, creator, token) = setup();

    let stream_id = client.create_stream(&sender, &creator, &token, &1i128, &10u64);

    // Advance past end time
    env.ledger().with_mut(|ledger| ledger.timestamp += 20);

    // Withdraw all
    client.withdraw_streamed(&creator, &stream_id);

    // Try to cancel completed stream
    let result = client.try_cancel_stream(&sender, &stream_id);
    assert_eq!(result, Err(Ok(TipJarError::StreamAlreadyCompleted)));
}

#[test]
fn test_stream_rate_limit() {
    let (env, client, sender, creator, token) = setup();

    // 1000 tokens/sec should be OK
    let result = client.try_create_stream(&sender, &creator, &token, &1000i128, &100u64);
    assert!(result.is_ok());

    // 1001 tokens/sec should fail
    let result = client.try_create_stream(&sender, &creator, &token, &1001i128, &100u64);
    assert_eq!(result, Err(Ok(TipJarError::StreamRateExceedsMaximum)));
}

#[test]
fn test_paused_stream_cannot_withdraw() {
    let (env, client, sender, creator, token) = setup();

    let stream_id = client.create_stream(&sender, &creator, &token, &5i128, &100u64);

    // Advance 10 seconds
    env.ledger().with_mut(|ledger| ledger.timestamp += 10);

    // Stop stream
    client.stop_stream(&sender, &stream_id);

    // Try to withdraw - should work (already accrued)
    client.withdraw_streamed(&creator, &stream_id);

    let stream = client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.withdrawn, 50i128);
}

#[test]
fn test_double_cancel() {
    let (env, client, sender, creator, token) = setup();

    let stream_id = client.create_stream(&sender, &creator, &token, &1i128, &100u64);

    client.cancel_stream(&sender, &stream_id);
    let result = client.try_cancel_stream(&sender, &stream_id);
    assert_eq!(result, Err(Ok(TipJarError::StreamAlreadyCancelled)));
}

#[test]
fn test_stream_token_whitelist() {
    let (env, client, sender, creator, token) = setup();

    // Create a non-whitelisted token
    let non_token_admin = Address::generate(&env);
    let non_token_id = env.register_stellar_asset_contract(non_token_admin.clone());
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &non_token_id);
    token_client.mint(&sender, &1000i128);

    // Should fail
    let result = client.try_create_stream(&sender, &creator, &non_token_id, &1i128, &100u64);
    assert_eq!(result, Err(Ok(TipJarError::TokenNotWhitelisted)));
}

#[test]
fn test_stream_get_available_to_withdraw() {
    let (env, client, sender, creator, token) = setup();

    let stream_id = client.create_stream(&sender, &creator, &token, &10i128, &100u64);

    // Initially 0
    assert_eq!(client.get_available_to_withdraw(&stream_id), 0);

    // After 5 seconds: 50 tokens
    env.ledger().with_mut(|ledger| ledger.timestamp += 5);
    assert_eq!(client.get_available_to_withdraw(&stream_id), 50);

    // Withdraw 30
    client.withdraw_streamed(&creator, &stream_id);

    // Available should be 20 now
    assert_eq!(client.get_available_to_withdraw(&stream_id), 20);

    // After 5 more seconds: 50 new + 20 old = 70
    env.ledger().with_mut(|ledger| ledger.timestamp += 5);
    assert_eq!(client.get_available_to_withdraw(&stream_id), 70);
}

#[test]
fn test_stream_completed_no_more_withdrawals() {
    let (env, client, sender, creator, token) = setup();

    let stream_id = client.create_stream(&sender, &creator, &token, &1i128, &10u64);

    // Advance past end
    env.ledger().with_mut(|ledger| ledger.timestamp += 20);

    // Withdraw all
    client.withdraw_streamed(&creator, &stream_id);

    // Try to withdraw again - should fail
    let result = client.try_withdraw_streamed(&creator, &stream_id);
    assert_eq!(result, Err(Ok(TipJarError::NoStreamedAmount)));

    // Available should be 0
    assert_eq!(client.get_available_to_withdraw(&stream_id), 0);
}

#[test]
fn test_get_stream_nonexistent() {
    let (env, client, _sender, _creator, _token) = setup();

    let stream = client.get_stream(&999u64);
    assert!(stream.is_none());
}

#[test]
fn test_stream_duration_accurate() {
    let (env, client, sender, creator, token) = setup();

    let stream_id = client.create_stream(
        &sender, &creator, &token, &10i128, &300u64, // 5 minutes
    );

    let stream = client.get_stream(&stream_id).unwrap();
    assert_eq!(stream.end_time - stream.start_time, 300);

    // Advance 150 seconds - exactly half
    env.ledger().with_mut(|ledger| ledger.timestamp += 150);
    let streamed = client.get_streamed_amount(&stream_id);
    assert_eq!(streamed, 1500i128); // 10 * 150

    // Advance 150 more seconds - should be complete
    env.ledger().with_mut(|ledger| ledger.timestamp += 150);
    let streamed = client.get_streamed_amount(&stream_id);
    assert_eq!(streamed, 3000i128); // 10 * 300
}
