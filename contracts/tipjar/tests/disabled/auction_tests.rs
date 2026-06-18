#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo, Address},
    Env,
};
use tipjar::{TipJarContract, TipJarContractClient};

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

    client.init(&admin);
    client.add_token(&admin, &token_id);

    (env, client, admin, token_admin, token_id)
}

#[test]
fn test_auction_bid_and_settle_success() {
    let (env, client, admin, _token_admin, token) = setup();
    let creator = Address::generate(&env);
    let bidder1 = Address::generate(&env);
    let bidder2 = Address::generate(&env);

    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_client.mint(&bidder1, &1_000_000i128);
    token_client.mint(&bidder2, &1_000_000i128);

    let auction_id = client.create_auction(&creator, &token, &100i128, &10u64);
    client.place_bid(&bidder1, &auction_id, &150i128);
    assert_eq!(token_client.balance(&bidder1), 1_000_000 - 150);

    client.place_bid(&bidder2, &auction_id, &200i128);
    assert_eq!(token_client.balance(&bidder2), 1_000_000 - 200);
    assert_eq!(token_client.balance(&bidder1), 1_000_000);

    let current_timestamp = env.ledger().timestamp();
    env.ledger().set(LedgerInfo {
        timestamp: current_timestamp + 20,
        sequence_number: env.ledger().sequence_number() + 1,
        ..Default::default()
    });

    client.settle_auction(&creator, &auction_id);
    let creator_balance = client.get_split_balance(&creator);
    assert_eq!(creator_balance, 200);

    let auction = client.get_auction(&auction_id).unwrap();
    assert!(auction.settled);
    assert_eq!(auction.highest_bid, 200);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #81)")]
fn test_auction_bid_below_reserve_fails() {
    let (env, client, _admin, _token_admin, token) = setup();
    let creator = Address::generate(&env);
    let bidder = Address::generate(&env);

    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_client.mint(&bidder, &1_000_000i128);

    let auction_id = client.create_auction(&creator, &token, &500i128, &10u64);
    client.place_bid(&bidder, &auction_id, &100i128);
}
