#![cfg(test)]

extern crate std;

use soroban_sdk::{Address, Env};
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

#[test]
fn test_mint_fractions_creates_pool() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);

    client.mint_fractions(&creator, &1_000u64, &10i128);

    let pool = client
        .get_fraction_pool(&creator)
        .expect("pool should exist");
    assert_eq!(pool.total_supply, 1_000);
    assert_eq!(pool.buyout_price_per_fraction, 10);
    assert_eq!(pool.pending_revenue, 0);
}

#[test]
fn test_creator_holds_full_supply_after_mint() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);

    client.mint_fractions(&creator, &500u64, &0i128);

    let pos = client.get_fraction_position(&creator, &creator);
    assert_eq!(pos.amount, 500);
}

#[test]
fn test_accrue_and_claim_revenue() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);

    client.mint_fractions(&creator, &100u64, &0i128);
    client.accrue_revenue(&creator, &1_000i128);

    let owed = client.claim_fraction_revenue(&creator, &creator);
    // 1000 revenue / 100 fractions = 10 per fraction; creator holds 100 → 1000
    assert_eq!(owed, 1_000);
}

#[test]
fn test_claim_revenue_zero_when_nothing_accrued() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);

    client.mint_fractions(&creator, &100u64, &0i128);

    let owed = client.claim_fraction_revenue(&creator, &creator);
    assert_eq!(owed, 0);
}

#[test]
fn test_transfer_fractions_splits_ownership() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let holder = Address::generate(&env);

    client.mint_fractions(&creator, &100u64, &0i128);
    client.transfer_fractions(&creator, &creator, &holder, &40u64);

    let creator_pos = client.get_fraction_position(&creator, &creator);
    let holder_pos = client.get_fraction_position(&creator, &holder);

    assert_eq!(creator_pos.amount, 60);
    assert_eq!(holder_pos.amount, 40);
}

#[test]
fn test_revenue_split_proportional_after_transfer() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let holder = Address::generate(&env);

    client.mint_fractions(&creator, &100u64, &0i128);
    client.transfer_fractions(&creator, &creator, &holder, &40u64);

    // Accrue 100 units: creator (60%) → 60, holder (40%) → 40
    client.accrue_revenue(&creator, &100i128);

    let creator_owed = client.claim_fraction_revenue(&creator, &creator);
    let holder_owed = client.claim_fraction_revenue(&creator, &holder);

    assert_eq!(creator_owed, 60);
    assert_eq!(holder_owed, 40);
}

#[test]
fn test_buyout_consolidates_ownership() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);

    client.mint_fractions(&creator, &100u64, &5i128);
    client.transfer_fractions(&creator, &creator, &buyer, &30u64);

    // buyer holds 30, creator holds 70 → buyer buys remaining 70 at 5 each
    let cost = client.buyout_fractions(&creator, &buyer);
    assert_eq!(cost, 70 * 5);

    let buyer_pos = client.get_fraction_position(&creator, &buyer);
    assert_eq!(buyer_pos.amount, 100);

    let creator_pos = client.get_fraction_position(&creator, &creator);
    assert_eq!(creator_pos.amount, 0);
}

#[test]
fn test_no_pool_returns_none() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);

    assert!(client.get_fraction_pool(&creator).is_none());
}

#[test]
fn test_double_claim_returns_zero() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);

    client.mint_fractions(&creator, &100u64, &0i128);
    client.accrue_revenue(&creator, &500i128);

    client.claim_fraction_revenue(&creator, &creator);
    let second = client.claim_fraction_revenue(&creator, &creator);
    assert_eq!(second, 0);
}
