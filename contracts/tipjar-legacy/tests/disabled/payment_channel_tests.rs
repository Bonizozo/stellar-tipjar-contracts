#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};
use tipjar::{
    payment_channel::{ChannelStatus, PaymentChannel},
    ChannelError, DataKey, TipJarContract, TipJarContractClient, TipJarError,
};

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

    let party_a = Address::generate(&env);
    let party_b = Address::generate(&env);

    let tok = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    tok.mint(&party_a, &1_000i128);
    tok.mint(&party_b, &1_000i128);

    (env, client, admin, party_a, party_b, token_id)
}

#[test]
fn test_open_channel() {
    let (env, client, _admin, party_a, party_b, token) = setup();

    client.open_channel(&party_a, &party_b, &token, &500i128, &500i128, &3600u64);

    let ch = client.get_channel(&party_a, &party_b, &token).unwrap();
    assert_eq!(ch.total_deposit, 1_000);
    assert_eq!(ch.balance_a, 500);
    assert_eq!(ch.nonce, 0);
    assert_eq!(ch.status, ChannelStatus::Open);
    assert_eq!(ch.dispute_window, 3600);
}

#[test]
fn test_update_channel_state() {
    let (env, client, _admin, party_a, party_b, token) = setup();

    client.open_channel(&party_a, &party_b, &token, &500i128, &500i128, &3600u64);
    client.update_channel_state(&party_a, &party_b, &token, &700i128, &1u64);

    let ch = client.get_channel(&party_a, &party_b, &token).unwrap();
    assert_eq!(ch.balance_a, 700);
    assert_eq!(ch.nonce, 1);
}

#[test]
fn test_update_stale_nonce_rejected() {
    let (env, client, _admin, party_a, party_b, token) = setup();

    client.open_channel(&party_a, &party_b, &token, &500i128, &500i128, &3600u64);
    client.update_channel_state(&party_a, &party_b, &token, &700i128, &5u64);

    let result = client.try_update_channel_state(&party_a, &party_b, &token, &600i128, &3u64);
    assert_eq!(result, Err(Ok(ChannelError::StaleNonce)));
}

#[test]
fn test_cooperative_close() {
    let (env, client, _admin, party_a, party_b, token) = setup();

    client.open_channel(&party_a, &party_b, &token, &500i128, &500i128, &3600u64);
    client.update_channel_state(&party_a, &party_b, &token, &300i128, &1u64);
    client.cooperative_close(&party_a, &party_b, &token);

    let ch = client.get_channel(&party_a, &party_b, &token).unwrap();
    assert_eq!(ch.status, ChannelStatus::Closed);

    // Verify token balances: party_a gets 300, party_b gets 700
    let tok = soroban_sdk::token::Client::new(&env, &token);
    assert_eq!(tok.balance(&party_a), 800); // started with 1000, deposited 500, got back 300
    assert_eq!(tok.balance(&party_b), 1200); // started with 1000, deposited 500, got back 700
}

#[test]
fn test_cooperative_close_already_closed() {
    let (env, client, _admin, party_a, party_b, token) = setup();

    client.open_channel(&party_a, &party_b, &token, &500i128, &500i128, &3600u64);
    client.cooperative_close(&party_a, &party_b, &token);

    let result = client.try_cooperative_close(&party_a, &party_b, &token);
    assert_eq!(result, Err(Ok(ChannelError::ChannelNotOpen)));
}

#[test]
fn test_dispute_close_initiate_and_finalise() {
    let (env, client, _admin, party_a, party_b, token) = setup();

    client.open_channel(&party_a, &party_b, &token, &500i128, &500i128, &3600u64);
    client.update_channel_state(&party_a, &party_b, &token, &800i128, &1u64);

    // party_a initiates dispute with latest state
    client.dispute_close(&party_a, &party_a, &party_b, &token, &800i128, &1u64);

    let ch = client.get_channel(&party_a, &party_b, &token).unwrap();
    assert_eq!(ch.status, ChannelStatus::Disputed);

    // Advance time past dispute window
    env.ledger().with_mut(|l| l.timestamp += 3601);

    // Anyone can finalise after window
    client.dispute_close(&party_b, &party_a, &party_b, &token, &800i128, &1u64);

    let ch = client.get_channel(&party_a, &party_b, &token).unwrap();
    assert_eq!(ch.status, ChannelStatus::Closed);

    let tok = soroban_sdk::token::Client::new(&env, &token);
    assert_eq!(tok.balance(&party_a), 1300); // 1000 - 500 + 800
    assert_eq!(tok.balance(&party_b), 700); // 1000 - 500 + 200
}

#[test]
fn test_dispute_counterparty_submits_newer_state() {
    let (env, client, _admin, party_a, party_b, token) = setup();

    client.open_channel(&party_a, &party_b, &token, &500i128, &500i128, &3600u64);
    client.update_channel_state(&party_a, &party_b, &token, &800i128, &2u64);

    // party_a tries to cheat with an old state (nonce=1, balance_a=900)
    client.dispute_close(&party_a, &party_a, &party_b, &token, &900i128, &1u64);

    // party_b counters with the real latest state (nonce=2)
    client.dispute_close(&party_b, &party_a, &party_b, &token, &800i128, &2u64);

    let ch = client.get_channel(&party_a, &party_b, &token).unwrap();
    assert_eq!(ch.balance_a, 800);
    assert_eq!(ch.nonce, 2);
}

#[test]
fn test_non_party_cannot_dispute() {
    let (env, client, _admin, party_a, party_b, token) = setup();
    let stranger = Address::generate(&env);

    client.open_channel(&party_a, &party_b, &token, &500i128, &500i128, &3600u64);

    let result = client.try_dispute_close(&stranger, &party_a, &party_b, &token, &500i128, &0u64);
    assert_eq!(result, Err(Ok(ChannelError::NotChannelParty)));
}

#[test]
fn test_get_channel_not_found() {
    let (_env, client, _admin, party_a, party_b, token) = setup();
    assert!(client.get_channel(&party_a, &party_b, &token).is_none());
}
