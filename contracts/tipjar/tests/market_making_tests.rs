#![cfg(test)]

extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env};
use tipjar::{
    market_making::MakerStatus,
    TipJarContract, TipJarContractClient,
};

fn setup() -> (Env, TipJarContractClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TipJarContract);
    let client = TipJarContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let base_token = env.register_stellar_asset_contract(token_admin.clone());
    let quote_token = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin);
    client.add_token(&admin, &base_token);
    client.add_token(&admin, &quote_token);

    (env, client, admin, base_token, quote_token)
}

fn mint(env: &Env, token: &Address, to: &Address, amount: i128) {
    soroban_sdk::token::StellarAssetClient::new(env, token).mint(to, &amount);
}

fn balance(env: &Env, token: &Address, of: &Address) -> i128 {
    soroban_sdk::token::Client::new(env, token).balance(of)
}

// ── Market Making ────────────────────────────────────────────────────────────

#[test]
fn test_register_maker_and_get_quote() {
    let (env, client, _admin, base, quote) = setup();
    let maker = Address::generate(&env);
    mint(&env, &base, &maker, 1_000_000);
    mint(&env, &quote, &maker, 1_000_000);

    let maker_id = client.mm_register(
        &maker,
        &base,
        &quote,
        &100u32,       // 1% spread
        &500_000i128,  // base deposit
        &500_000i128,  // quote deposit
        &100_000i128,  // max trade size
    );

    let mm = client.mm_get_maker(&maker_id).unwrap();
    assert_eq!(mm.spread_bps, 100);
    assert_eq!(mm.base_inventory, 500_000);
    assert_eq!(mm.quote_inventory, 500_000);
    assert_eq!(mm.status, MakerStatus::Active);

    // Quote at mid_price = 1_000_000
    let q = client.mm_get_quote(&maker_id, &1_000_000i128);
    assert_eq!(q.mid_price, 1_000_000);
    // bid = 1_000_000 - 1_000_000*100/(2*10_000) = 1_000_000 - 5_000 = 995_000
    assert_eq!(q.bid_price, 995_000);
    // ask = 1_000_000 + 5_000 = 1_005_000
    assert_eq!(q.ask_price, 1_005_000);
}

#[test]
fn test_buy_trade() {
    let (env, client, _admin, base, quote) = setup();
    let maker_addr = Address::generate(&env);
    let taker = Address::generate(&env);

    mint(&env, &base, &maker_addr, 1_000_000);
    mint(&env, &quote, &maker_addr, 2_000_000);
    mint(&env, &quote, &taker, 2_000_000);

    let maker_id = client.mm_register(
        &maker_addr, &base, &quote,
        &100u32, &1_000_000i128, &2_000_000i128, &500_000i128,
    );

    let mid_price = 1_000_000i128;
    let base_amount = 100_000i128;

    // ask = 1_005_000; quote_needed = 100_000 * 1_005_000 / 1_000_000 = 100_500
    // fee = 100_500 * 10 / 10_000 = 100 (rounded down)
    // total_quote = 100_600
    let result = client.mm_buy(&taker, &maker_id, &base_amount, &mid_price, &200_000i128);

    assert_eq!(result.base_amount, base_amount);
    assert!(result.quote_amount > 0);
    assert!(result.fee_amount > 0);

    // Taker received base tokens
    assert_eq!(balance(&env, &base, &taker), base_amount);

    // Maker's base inventory decreased
    let mm = client.mm_get_maker(&maker_id).unwrap();
    assert_eq!(mm.base_inventory, 1_000_000 - base_amount);
    assert!(mm.fees_earned > 0);
    assert_eq!(mm.trade_count, 1);
}

#[test]
fn test_sell_trade() {
    let (env, client, _admin, base, quote) = setup();
    let maker_addr = Address::generate(&env);
    let taker = Address::generate(&env);

    mint(&env, &base, &maker_addr, 1_000_000);
    mint(&env, &quote, &maker_addr, 2_000_000);
    mint(&env, &base, &taker, 500_000);

    let maker_id = client.mm_register(
        &maker_addr, &base, &quote,
        &100u32, &1_000_000i128, &2_000_000i128, &500_000i128,
    );

    let mid_price = 1_000_000i128;
    let base_amount = 100_000i128;

    let result = client.mm_sell(&taker, &maker_id, &base_amount, &mid_price, &0i128);

    assert_eq!(result.base_amount, base_amount);
    assert!(result.quote_amount > 0);

    // Taker's base decreased
    assert_eq!(balance(&env, &base, &taker), 500_000 - base_amount);
    // Taker received quote
    assert!(balance(&env, &quote, &taker) > 0);

    let mm = client.mm_get_maker(&maker_id).unwrap();
    assert_eq!(mm.base_inventory, 1_000_000 + base_amount);
    assert_eq!(mm.trade_count, 1);
}

#[test]
fn test_deposit_and_withdraw_inventory() {
    let (env, client, _admin, base, quote) = setup();
    let maker_addr = Address::generate(&env);

    mint(&env, &base, &maker_addr, 2_000_000);
    mint(&env, &quote, &maker_addr, 2_000_000);

    let maker_id = client.mm_register(
        &maker_addr, &base, &quote,
        &50u32, &500_000i128, &500_000i128, &100_000i128,
    );

    // Deposit more
    client.mm_deposit(&maker_addr, &maker_id, &200_000i128, &200_000i128);
    let mm = client.mm_get_maker(&maker_id).unwrap();
    assert_eq!(mm.base_inventory, 700_000);
    assert_eq!(mm.quote_inventory, 700_000);

    // Withdraw some
    client.mm_withdraw(&maker_addr, &maker_id, &100_000i128, &100_000i128);
    let mm = client.mm_get_maker(&maker_id).unwrap();
    assert_eq!(mm.base_inventory, 600_000);
    assert_eq!(mm.quote_inventory, 600_000);
}

#[test]
fn test_update_spread() {
    let (env, client, _admin, base, quote) = setup();
    let maker_addr = Address::generate(&env);

    mint(&env, &base, &maker_addr, 1_000_000);
    mint(&env, &quote, &maker_addr, 1_000_000);

    let maker_id = client.mm_register(
        &maker_addr, &base, &quote,
        &100u32, &500_000i128, &500_000i128, &100_000i128,
    );

    client.mm_update_spread(&maker_addr, &maker_id, &200u32);
    let mm = client.mm_get_maker(&maker_id).unwrap();
    assert_eq!(mm.spread_bps, 200);
}

#[test]
fn test_pause_and_resume_maker() {
    let (env, client, _admin, base, quote) = setup();
    let maker_addr = Address::generate(&env);

    mint(&env, &base, &maker_addr, 1_000_000);
    mint(&env, &quote, &maker_addr, 1_000_000);

    let maker_id = client.mm_register(
        &maker_addr, &base, &quote,
        &100u32, &500_000i128, &500_000i128, &100_000i128,
    );

    client.mm_set_status(&maker_addr, &maker_id, &false);
    let mm = client.mm_get_maker(&maker_id).unwrap();
    assert_eq!(mm.status, MakerStatus::Paused);

    client.mm_set_status(&maker_addr, &maker_id, &true);
    let mm = client.mm_get_maker(&maker_id).unwrap();
    assert_eq!(mm.status, MakerStatus::Active);
}

#[test]
fn test_close_maker_returns_inventory() {
    let (env, client, _admin, base, quote) = setup();
    let maker_addr = Address::generate(&env);

    mint(&env, &base, &maker_addr, 1_000_000);
    mint(&env, &quote, &maker_addr, 1_000_000);

    let maker_id = client.mm_register(
        &maker_addr, &base, &quote,
        &100u32, &500_000i128, &500_000i128, &100_000i128,
    );

    client.mm_close(&maker_addr, &maker_id);

    let mm = client.mm_get_maker(&maker_id).unwrap();
    assert_eq!(mm.status, MakerStatus::Closed);
    assert_eq!(mm.base_inventory, 0);
    assert_eq!(mm.quote_inventory, 0);

    // All inventory returned
    assert_eq!(balance(&env, &base, &maker_addr), 1_000_000);
    assert_eq!(balance(&env, &quote, &maker_addr), 1_000_000);

    // No longer in active list
    let active = client.mm_get_active_makers();
    assert!(!active.contains(&maker_id));
}

#[test]
fn test_get_pair_makers() {
    let (env, client, _admin, base, quote) = setup();
    let maker1 = Address::generate(&env);
    let maker2 = Address::generate(&env);

    mint(&env, &base, &maker1, 1_000_000);
    mint(&env, &quote, &maker1, 1_000_000);
    mint(&env, &base, &maker2, 1_000_000);
    mint(&env, &quote, &maker2, 1_000_000);

    let id1 = client.mm_register(
        &maker1, &base, &quote, &100u32, &500_000i128, &500_000i128, &100_000i128,
    );
    let id2 = client.mm_register(
        &maker2, &base, &quote, &200u32, &500_000i128, &500_000i128, &100_000i128,
    );

    let pair_makers = client.mm_get_pair_makers(&base, &quote);
    assert!(pair_makers.contains(&id1));
    assert!(pair_makers.contains(&id2));
}
