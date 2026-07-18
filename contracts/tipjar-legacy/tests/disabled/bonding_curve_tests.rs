#![cfg(test)]

extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env};
use tipjar::{
    bonding_curve::{CurveParams, CurveType},
    TipJarContract, TipJarContractClient, TipJarError,
};

// ── helpers ───────────────────────────────────────────────────────────────────

const PRECISION: i128 = 10_000_000;

/// Returns (env, client, admin, tip_token, reserve_token, creator).
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

    let tip_token = env.register_stellar_asset_contract(token_admin.clone());
    let reserve_token = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin, &0u32, &0u64);
    client.add_token(&admin, &tip_token);
    client.add_token(&admin, &reserve_token);

    let creator = Address::generate(&env);

    // Mint reserve tokens to creator and a buyer
    soroban_sdk::token::StellarAssetClient::new(&env, &reserve_token)
        .mint(&creator, &10_000_000i128);

    // Mint tip tokens to contract so it can transfer on buy
    soroban_sdk::token::StellarAssetClient::new(&env, &tip_token)
        .mint(&contract_id, &100_000_000i128);

    (env, client, admin, tip_token, reserve_token, creator)
}

fn linear_params() -> CurveParams {
    CurveParams {
        curve_type: CurveType::Linear,
        base_price: PRECISION, // 1.0
        slope: PRECISION / 10, // 0.1 per token
        k_param: 0,
        midpoint: 0,
        max_price: 0,
        buy_fee_bps: 100,  // 1%
        sell_fee_bps: 100, // 1%
    }
}

fn exponential_params() -> CurveParams {
    CurveParams {
        curve_type: CurveType::Exponential,
        base_price: PRECISION,
        slope: PRECISION / 100, // 0.01 growth rate
        k_param: 0,
        midpoint: 0,
        max_price: 0,
        buy_fee_bps: 50,
        sell_fee_bps: 50,
    }
}

fn sigmoid_params() -> CurveParams {
    CurveParams {
        curve_type: CurveType::Sigmoid,
        base_price: PRECISION,
        slope: 0,
        k_param: PRECISION / 10,   // k = 0.1
        midpoint: 50 * PRECISION,  // inflection at supply = 50
        max_price: 10 * PRECISION, // max price = 10
        buy_fee_bps: 200,
        sell_fee_bps: 200,
    }
}

fn mint_reserve(env: &Env, reserve_token: &Address, to: &Address, amount: i128) {
    soroban_sdk::token::StellarAssetClient::new(env, reserve_token).mint(to, &amount);
}

// ── bc_create_curve ───────────────────────────────────────────────────────────

#[test]
fn test_create_linear_curve() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    assert_eq!(curve_id, 1);
    let curve = client.bc_get_curve(&curve_id);
    assert!(curve.active);
    assert_eq!(curve.supply, 0);
    assert_eq!(curve.reserve, 0);
    assert_eq!(curve.buy_fee_bps, 100);
}

#[test]
fn test_create_exponential_curve() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &exponential_params(),
        &0i128,
    );

    let curve = client.bc_get_curve(&curve_id);
    assert!(curve.active);
}

#[test]
fn test_create_sigmoid_curve() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &sigmoid_params(),
        &0i128,
    );

    let curve = client.bc_get_curve(&curve_id);
    assert!(curve.active);
}

#[test]
fn test_create_curve_with_initial_reserve() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &500_000i128,
    );

    let curve = client.bc_get_curve(&curve_id);
    assert_eq!(curve.reserve, 500_000);
}

#[test]
fn test_create_curve_zero_base_price_fails() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let mut params = linear_params();
    params.base_price = 0;

    let result = client.try_bc_create_curve(&creator, &tip_token, &reserve_token, &params, &0i128);
    assert_eq!(result, Err(Ok(TipJarError::BcInvalidParams)));
}

#[test]
fn test_create_curve_fee_too_high_fails() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let mut params = linear_params();
    params.buy_fee_bps = 2_000; // 20% — exceeds MAX_FEE_BPS

    let result = client.try_bc_create_curve(&creator, &tip_token, &reserve_token, &params, &0i128);
    assert_eq!(result, Err(Ok(TipJarError::BcFeeTooHigh)));
}

#[test]
fn test_create_curve_increments_id() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let id1 = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );
    let id2 = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

// ── bc_get_spot_price ─────────────────────────────────────────────────────────

#[test]
fn test_spot_price_at_zero_supply_equals_base_price() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let price = client.bc_get_spot_price(&curve_id);
    assert_eq!(price, PRECISION); // base_price = 1.0
}

#[test]
fn test_spot_price_increases_after_buy() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let price_before = client.bc_get_spot_price(&curve_id);

    let buyer = Address::generate(&env);
    mint_reserve(&env, &reserve_token, &buyer, 1_000_000i128);
    client.bc_buy(&buyer, &curve_id, &(10 * PRECISION), &1_000_000i128);

    let price_after = client.bc_get_spot_price(&curve_id);
    assert!(price_after > price_before);
}

#[test]
fn test_spot_price_decreases_after_sell() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    // Buy first
    let buyer = Address::generate(&env);
    mint_reserve(&env, &reserve_token, &buyer, 5_000_000i128);
    client.bc_buy(&buyer, &curve_id, &(20 * PRECISION), &5_000_000i128);

    let price_after_buy = client.bc_get_spot_price(&curve_id);

    // Mint tip tokens to buyer so they can sell
    soroban_sdk::token::StellarAssetClient::new(&env, &tip_token).mint(&buyer, &(20 * PRECISION));
    client.bc_sell(&buyer, &curve_id, &(10 * PRECISION), &0i128);

    let price_after_sell = client.bc_get_spot_price(&curve_id);
    assert!(price_after_sell < price_after_buy);
}

// ── bc_buy ────────────────────────────────────────────────────────────────────

#[test]
fn test_buy_updates_supply_and_reserve() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let buyer = Address::generate(&env);
    mint_reserve(&env, &reserve_token, &buyer, 2_000_000i128);

    let amount = 5 * PRECISION;
    let result = client.bc_buy(&buyer, &curve_id, &amount, &2_000_000i128);

    assert_eq!(result.token_amount, amount);
    assert!(result.collateral_amount > 0);
    assert!(result.fee_amount > 0);
    assert_eq!(result.new_supply, amount);

    let curve = client.bc_get_curve(&curve_id);
    assert_eq!(curve.supply, amount);
    assert!(curve.reserve > 0);
}

#[test]
fn test_buy_zero_amount_fails() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let buyer = Address::generate(&env);
    let result = client.try_bc_buy(&buyer, &curve_id, &0i128, &1_000_000i128);
    assert_eq!(result, Err(Ok(TipJarError::InvalidAmount)));
}

#[test]
fn test_buy_slippage_exceeded_fails() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let buyer = Address::generate(&env);
    mint_reserve(&env, &reserve_token, &buyer, 10_000_000i128);

    // Set max_collateral to 1 — way too low
    let result = client.try_bc_buy(&buyer, &curve_id, &(10 * PRECISION), &1i128);
    assert_eq!(result, Err(Ok(TipJarError::BcSlippageExceeded)));
}

#[test]
fn test_buy_inactive_curve_fails() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    client.bc_deactivate(&creator, &curve_id);

    let buyer = Address::generate(&env);
    mint_reserve(&env, &reserve_token, &buyer, 1_000_000i128);

    let result = client.try_bc_buy(&buyer, &curve_id, &PRECISION, &1_000_000i128);
    assert_eq!(result, Err(Ok(TipJarError::BcInactive)));
}

// ── bc_sell ───────────────────────────────────────────────────────────────────

#[test]
fn test_sell_reduces_supply_and_reserve() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let buyer = Address::generate(&env);
    mint_reserve(&env, &reserve_token, &buyer, 5_000_000i128);
    let buy_amount = 10 * PRECISION;
    client.bc_buy(&buyer, &curve_id, &buy_amount, &5_000_000i128);

    // Give buyer tip tokens to sell
    soroban_sdk::token::StellarAssetClient::new(&env, &tip_token).mint(&buyer, &buy_amount);

    let sell_amount = 5 * PRECISION;
    let result = client.bc_sell(&buyer, &curve_id, &sell_amount, &0i128);

    assert_eq!(result.token_amount, sell_amount);
    assert!(result.collateral_amount > 0);

    let curve = client.bc_get_curve(&curve_id);
    assert_eq!(curve.supply, buy_amount - sell_amount);
}

#[test]
fn test_sell_more_than_supply_fails() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let buyer = Address::generate(&env);
    mint_reserve(&env, &reserve_token, &buyer, 2_000_000i128);
    client.bc_buy(&buyer, &curve_id, &(5 * PRECISION), &2_000_000i128);

    soroban_sdk::token::StellarAssetClient::new(&env, &tip_token).mint(&buyer, &(100 * PRECISION));

    let result = client.try_bc_sell(&buyer, &curve_id, &(100 * PRECISION), &0i128);
    assert_eq!(result, Err(Ok(TipJarError::BcInsufficientSupply)));
}

#[test]
fn test_sell_slippage_exceeded_fails() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let buyer = Address::generate(&env);
    mint_reserve(&env, &reserve_token, &buyer, 5_000_000i128);
    client.bc_buy(&buyer, &curve_id, &(10 * PRECISION), &5_000_000i128);

    soroban_sdk::token::StellarAssetClient::new(&env, &tip_token).mint(&buyer, &(10 * PRECISION));

    // min_collateral set impossibly high
    let result = client.try_bc_sell(&buyer, &curve_id, &(5 * PRECISION), &999_999_999i128);
    assert_eq!(result, Err(Ok(TipJarError::BcSlippageExceeded)));
}

// ── bc_get_quote ──────────────────────────────────────────────────────────────

#[test]
fn test_quote_buy_cost_positive() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let quote = client.bc_get_quote(&curve_id, &(10 * PRECISION));
    assert!(quote.buy_cost > 0);
    assert!(quote.buy_fee > 0);
    assert_eq!(quote.sell_return, 0); // no supply yet
}

#[test]
fn test_quote_sell_return_after_buy() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let buyer = Address::generate(&env);
    mint_reserve(&env, &reserve_token, &buyer, 5_000_000i128);
    client.bc_buy(&buyer, &curve_id, &(20 * PRECISION), &5_000_000i128);

    let quote = client.bc_get_quote(&curve_id, &(10 * PRECISION));
    assert!(quote.sell_return > 0);
    assert!(quote.sell_fee > 0);
    // Sell return should be less than buy cost (fees + curve shape)
    assert!(quote.sell_return < quote.buy_cost);
}

#[test]
fn test_quote_zero_amount_fails() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let result = client.try_bc_get_quote(&curve_id, &0i128);
    assert_eq!(result, Err(Ok(TipJarError::InvalidAmount)));
}

// ── fee management ────────────────────────────────────────────────────────────

#[test]
fn test_fees_accumulate_on_buy() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let buyer = Address::generate(&env);
    mint_reserve(&env, &reserve_token, &buyer, 5_000_000i128);
    client.bc_buy(&buyer, &curve_id, &(10 * PRECISION), &5_000_000i128);

    let curve = client.bc_get_curve(&curve_id);
    assert!(curve.fees_collected > 0);
}

#[test]
fn test_withdraw_fees_transfers_to_creator() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let buyer = Address::generate(&env);
    mint_reserve(&env, &reserve_token, &buyer, 5_000_000i128);
    client.bc_buy(&buyer, &curve_id, &(10 * PRECISION), &5_000_000i128);

    let fees_before = client.bc_get_curve(&curve_id).fees_collected;
    assert!(fees_before > 0);

    let withdrawn = client.bc_withdraw_fees(&creator, &curve_id);
    assert_eq!(withdrawn, fees_before);

    let curve = client.bc_get_curve(&curve_id);
    assert_eq!(curve.fees_collected, 0);
}

#[test]
fn test_withdraw_fees_no_fees_fails() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let result = client.try_bc_withdraw_fees(&creator, &curve_id);
    assert_eq!(result, Err(Ok(TipJarError::BcNoFeesToWithdraw)));
}

#[test]
fn test_update_fees() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    client.bc_update_fees(&creator, &curve_id, &200u32, &300u32);

    let curve = client.bc_get_curve(&curve_id);
    assert_eq!(curve.buy_fee_bps, 200);
    assert_eq!(curve.sell_fee_bps, 300);
}

#[test]
fn test_update_fees_too_high_fails() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let result = client.try_bc_update_fees(&creator, &curve_id, &5_000u32, &0u32);
    assert_eq!(result, Err(Ok(TipJarError::BcFeeTooHigh)));
}

// ── deactivation ──────────────────────────────────────────────────────────────

#[test]
fn test_deactivate_curve() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    client.bc_deactivate(&creator, &curve_id);

    let curve = client.bc_get_curve(&curve_id);
    assert!(!curve.active);
}

#[test]
fn test_deactivate_already_inactive_fails() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    client.bc_deactivate(&creator, &curve_id);

    let result = client.try_bc_deactivate(&creator, &curve_id);
    assert_eq!(result, Err(Ok(TipJarError::BcInactive)));
}

// ── pause guard ───────────────────────────────────────────────────────────────

#[test]
fn test_bc_buy_paused_fails() {
    let (env, client, admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &linear_params(),
        &0i128,
    );

    let reason = soroban_sdk::String::from_str(&env, "maintenance");
    client.pause(&admin, &reason);

    let buyer = Address::generate(&env);
    mint_reserve(&env, &reserve_token, &buyer, 1_000_000i128);

    let result = client.try_bc_buy(&buyer, &curve_id, &PRECISION, &1_000_000i128);
    assert_eq!(result, Err(Ok(TipJarError::ContractPaused)));
}

// ── curve type pricing ────────────────────────────────────────────────────────

#[test]
fn test_exponential_curve_buy() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &exponential_params(),
        &0i128,
    );

    let buyer = Address::generate(&env);
    mint_reserve(&env, &reserve_token, &buyer, 5_000_000i128);

    let result = client.bc_buy(&buyer, &curve_id, &(5 * PRECISION), &5_000_000i128);
    assert!(result.collateral_amount > 0);
    assert_eq!(result.new_supply, 5 * PRECISION);
}

#[test]
fn test_sigmoid_curve_buy() {
    let (env, client, _admin, tip_token, reserve_token, creator) = setup();

    let curve_id = client.bc_create_curve(
        &creator,
        &tip_token,
        &reserve_token,
        &sigmoid_params(),
        &0i128,
    );

    let buyer = Address::generate(&env);
    mint_reserve(&env, &reserve_token, &buyer, 5_000_000i128);

    let result = client.bc_buy(&buyer, &curve_id, &(5 * PRECISION), &5_000_000i128);
    assert!(result.collateral_amount > 0);
}

#[test]
fn test_not_found_curve_fails() {
    let (env, client, _admin, _tip_token, _reserve_token, _creator) = setup();

    let result = client.try_bc_get_curve(&999u64);
    assert_eq!(result, Err(Ok(TipJarError::BcNotFound)));
}
