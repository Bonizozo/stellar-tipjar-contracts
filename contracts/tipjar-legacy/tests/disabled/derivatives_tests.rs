//! Integration tests for the Tip Derivatives Platform.
//!
//! Tests cover:
//! - Opening and matching all four derivative kinds
//! - Pricing model sanity checks
//! - Risk management (position limits, margin health)
//! - Settlement: exercise, expiry, expiry-worthless, futures P&L

#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env,
};
use tipjar::{
    derivatives::pricing::{
        black_scholes, futures_fair_value, isqrt, norm_cdf, swap_fair_value, PricingInput,
    },
    derivatives::risk::{
        check_margin_health, find_liquidatable, portfolio_health, HEALTH_PRECISION,
    },
    derivatives::settlement,
    derivatives::{self, DerivativeKind, DerivativeStatus, PRICE_PRECISION},
};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn create_token(env: &Env, admin: &Address) -> Address {
    env.register_stellar_asset_contract(admin.clone())
}

fn mint(env: &Env, token: &Address, to: &Address, amount: i128) {
    let client = token::StellarAssetClient::new(env, token);
    client.mint(to, &amount);
}

// ── Pricing unit tests ───────────────────────────────────────────────────────

#[test]
fn test_isqrt_basic() {
    assert_eq!(isqrt(0), 0);
    assert_eq!(isqrt(PRICE_PRECISION * PRICE_PRECISION), PRICE_PRECISION);
    assert_eq!(
        isqrt(4 * PRICE_PRECISION * PRICE_PRECISION),
        2 * PRICE_PRECISION
    );
}

#[test]
fn test_norm_cdf_at_zero() {
    let p = norm_cdf(0);
    // N(0) ≈ 0.5
    assert!(
        (p - 500_000).abs() < 5_000,
        "N(0) should be ~0.5, got {}",
        p
    );
}

#[test]
fn test_norm_cdf_symmetry() {
    let p1 = norm_cdf(PRICE_PRECISION);
    let p_neg1 = norm_cdf(-PRICE_PRECISION);
    assert!(
        (p1 + p_neg1 - PRICE_PRECISION).abs() < 10_000,
        "symmetry failed"
    );
}

#[test]
fn test_black_scholes_atm() {
    let input = PricingInput {
        spot: PRICE_PRECISION,
        strike: PRICE_PRECISION,
        time_to_expiry_secs: 86_400 * 30,
        volatility_bps: 5_000,
        risk_free_rate_bps: 500,
    };
    let prices = black_scholes(&input).expect("should price ATM option");
    // ATM call and put should be close
    let diff = (prices.call - prices.put).abs();
    assert!(
        diff < PRICE_PRECISION / 10,
        "ATM call/put diff too large: {}",
        diff
    );
    assert!(prices.call > 0, "call should be positive");
    assert!(prices.put > 0, "put should be positive");
}

#[test]
fn test_black_scholes_deep_itm_call() {
    let input = PricingInput {
        spot: 2 * PRICE_PRECISION, // S = 2.0
        strike: PRICE_PRECISION,   // K = 1.0
        time_to_expiry_secs: 86_400 * 30,
        volatility_bps: 3_000,
        risk_free_rate_bps: 500,
    };
    let prices = black_scholes(&input).expect("should price deep ITM call");
    // Deep ITM call ≈ S - K = 1.0
    assert!(
        prices.call > PRICE_PRECISION * 9 / 10,
        "deep ITM call should be close to intrinsic"
    );
    // Deep ITM put ≈ 0
    assert!(
        prices.put < PRICE_PRECISION / 10,
        "deep ITM put should be near zero"
    );
}

#[test]
fn test_futures_fair_value_above_spot() {
    let fair = futures_fair_value(PRICE_PRECISION, 86_400 * 365, 500);
    assert!(
        fair > PRICE_PRECISION,
        "futures should be above spot with positive rate"
    );
    assert!(
        fair < 11 * PRICE_PRECISION / 10,
        "futures should be < 1.1x spot"
    );
}

#[test]
fn test_swap_fair_value_zero_when_equal() {
    let pv = swap_fair_value(500, 500, PRICE_PRECISION, 86_400 * 365);
    assert_eq!(pv, 0, "swap PV should be zero when rates are equal");
}

#[test]
fn test_swap_fair_value_positive_when_floating_higher() {
    let pv = swap_fair_value(500, 1_000, PRICE_PRECISION, 86_400 * 365);
    assert!(pv > 0, "fixed payer profits when floating > fixed");
}

// ── Derivative lifecycle tests ───────────────────────────────────────────────

#[test]
fn test_open_and_cancel_call_option() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let writer = Address::generate(&env);
    let token = create_token(&env, &admin);
    mint(&env, &token, &writer, 10_000_000);

    let now = env.ledger().timestamp();
    let expires_at = now + 86_400;

    let id = derivatives::open(
        &env,
        &writer,
        DerivativeKind::Call,
        &token,
        1_000_000,       // notional
        PRICE_PRECISION, // strike = 1.0
        50_000,          // premium
        expires_at,
    );

    let dc = derivatives::get_contract(&env, id).unwrap();
    assert_eq!(dc.status, DerivativeStatus::Open);
    assert_eq!(dc.kind, DerivativeKind::Call);
    assert!(dc.collateral_a > 0);

    // Cancel before matching
    derivatives::cancel(&env, &writer, id);
    let dc = derivatives::get_contract(&env, id).unwrap();
    assert_eq!(dc.status, DerivativeStatus::Cancelled);
}

#[test]
fn test_open_and_match_put_option() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let writer = Address::generate(&env);
    let holder = Address::generate(&env);
    let token = create_token(&env, &admin);
    mint(&env, &token, &writer, 10_000_000);
    mint(&env, &token, &holder, 10_000_000);

    let now = env.ledger().timestamp();
    let expires_at = now + 86_400;

    let id = derivatives::open(
        &env,
        &writer,
        DerivativeKind::Put,
        &token,
        1_000_000,
        PRICE_PRECISION,
        30_000,
        expires_at,
    );

    derivatives::match_contract(&env, &holder, id);

    let dc = derivatives::get_contract(&env, id).unwrap();
    assert_eq!(dc.status, DerivativeStatus::Active);
    assert_eq!(dc.party_b, Some(holder.clone()));
    assert!(dc.collateral_b > 0);
}

#[test]
fn test_open_and_match_futures() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let long = Address::generate(&env);
    let short = Address::generate(&env);
    let token = create_token(&env, &admin);
    mint(&env, &token, &long, 10_000_000);
    mint(&env, &token, &short, 10_000_000);

    let now = env.ledger().timestamp();
    let expires_at = now + 86_400 * 30;

    let id = derivatives::open(
        &env,
        &long,
        DerivativeKind::Future,
        &token,
        1_000_000,
        PRICE_PRECISION,
        0,
        expires_at,
    );

    derivatives::match_contract(&env, &short, id);

    let dc = derivatives::get_contract(&env, id).unwrap();
    assert_eq!(dc.status, DerivativeStatus::Active);
    assert!(dc.collateral_a > 0);
    assert!(dc.collateral_b > 0);
}

#[test]
fn test_update_mark_price() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let long = Address::generate(&env);
    let short = Address::generate(&env);
    let token = create_token(&env, &admin);
    mint(&env, &token, &long, 10_000_000);
    mint(&env, &token, &short, 10_000_000);

    let now = env.ledger().timestamp();
    let id = derivatives::open(
        &env,
        &long,
        DerivativeKind::Future,
        &token,
        1_000_000,
        PRICE_PRECISION,
        0,
        now + 86_400,
    );
    derivatives::match_contract(&env, &short, id);

    let new_price = 12 * PRICE_PRECISION / 10; // 1.2
    derivatives::update_mark_price(&env, id, new_price);

    let dc = derivatives::get_contract(&env, id).unwrap();
    assert_eq!(dc.mark_price, new_price);
}

// ── Risk management tests ────────────────────────────────────────────────────

#[test]
fn test_margin_health_healthy() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let long = Address::generate(&env);
    let short = Address::generate(&env);
    let token = create_token(&env, &admin);
    mint(&env, &token, &long, 10_000_000);
    mint(&env, &token, &short, 10_000_000);

    let now = env.ledger().timestamp();
    let id = derivatives::open(
        &env,
        &long,
        DerivativeKind::Future,
        &token,
        1_000_000,
        PRICE_PRECISION,
        0,
        now + 86_400,
    );
    derivatives::match_contract(&env, &short, id);

    let dc = derivatives::get_contract(&env, id).unwrap();
    let cfg = derivatives::get_config(&env);
    let (under_a, under_b) = check_margin_health(&dc, &cfg);
    // At initial margin, both sides should be healthy
    assert!(!under_a, "party_a should not be under-margined at open");
    assert!(!under_b, "party_b should not be under-margined at open");
}

#[test]
fn test_portfolio_health_no_positions() {
    let env = Env::default();
    let account = Address::generate(&env);
    let cfg = derivatives::get_config(&env);
    let health = portfolio_health(&env, &account, &cfg);
    assert_eq!(health.active_count, 0);
    assert!(health.health_factor >= HEALTH_PRECISION);
}

#[test]
fn test_find_liquidatable_empty() {
    let env = Env::default();
    let cfg = derivatives::get_config(&env);
    let result = find_liquidatable(&env, &cfg);
    assert_eq!(result.len(), 0);
}

// ── Settlement tests ─────────────────────────────────────────────────────────

#[test]
fn test_settle_expired_futures_long_profit() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let long = Address::generate(&env);
    let short = Address::generate(&env);
    let token = create_token(&env, &admin);
    mint(&env, &token, &long, 10_000_000);
    mint(&env, &token, &short, 10_000_000);

    let now = env.ledger().timestamp();
    let expires_at = now + 100;

    let id = derivatives::open(
        &env,
        &long,
        DerivativeKind::Future,
        &token,
        1_000_000,
        PRICE_PRECISION,
        0,
        expires_at,
    );
    derivatives::match_contract(&env, &short, id);

    // Advance time past expiry
    env.ledger().with_mut(|l| l.timestamp = expires_at + 1);

    // Final price above strike → long profits
    let final_price = 12 * PRICE_PRECISION / 10;
    settlement::settle_expired(&env, id, final_price);

    let dc = derivatives::get_contract(&env, id).unwrap();
    assert_eq!(dc.status, DerivativeStatus::Settled);
    assert_eq!(dc.settlement_price, final_price);
}

#[test]
fn test_settle_expired_call_option_itm() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let writer = Address::generate(&env);
    let holder = Address::generate(&env);
    let token = create_token(&env, &admin);
    mint(&env, &token, &writer, 10_000_000);
    mint(&env, &token, &holder, 10_000_000);

    let now = env.ledger().timestamp();
    let expires_at = now + 100;

    let id = derivatives::open(
        &env,
        &writer,
        DerivativeKind::Call,
        &token,
        1_000_000,
        PRICE_PRECISION,
        50_000,
        expires_at,
    );
    derivatives::match_contract(&env, &holder, id);

    env.ledger().with_mut(|l| l.timestamp = expires_at + 1);

    // Final price above strike → call is ITM
    let final_price = 15 * PRICE_PRECISION / 10;
    settlement::settle_expired(&env, id, final_price);

    let dc = derivatives::get_contract(&env, id).unwrap();
    assert_eq!(dc.status, DerivativeStatus::Settled);
}

#[test]
fn test_settle_expired_call_option_otm() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let writer = Address::generate(&env);
    let holder = Address::generate(&env);
    let token = create_token(&env, &admin);
    mint(&env, &token, &writer, 10_000_000);
    mint(&env, &token, &holder, 10_000_000);

    let now = env.ledger().timestamp();
    let expires_at = now + 100;

    let id = derivatives::open(
        &env,
        &writer,
        DerivativeKind::Call,
        &token,
        1_000_000,
        PRICE_PRECISION,
        50_000,
        expires_at,
    );
    derivatives::match_contract(&env, &holder, id);

    env.ledger().with_mut(|l| l.timestamp = expires_at + 1);

    // Final price below strike → call is OTM, expires worthless
    let final_price = PRICE_PRECISION / 2;
    settlement::settle_expired(&env, id, final_price);

    let dc = derivatives::get_contract(&env, id).unwrap();
    assert_eq!(dc.status, DerivativeStatus::Expired);
}

#[test]
fn test_exercise_call_option_itm() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let writer = Address::generate(&env);
    let holder = Address::generate(&env);
    let token = create_token(&env, &admin);
    mint(&env, &token, &writer, 10_000_000);
    mint(&env, &token, &holder, 10_000_000);

    let now = env.ledger().timestamp();
    let expires_at = now + 86_400;

    let id = derivatives::open(
        &env,
        &writer,
        DerivativeKind::Call,
        &token,
        1_000_000,
        PRICE_PRECISION,
        50_000,
        expires_at,
    );
    derivatives::match_contract(&env, &holder, id);

    // Move mark price above strike
    let new_price = 15 * PRICE_PRECISION / 10;
    derivatives::update_mark_price(&env, id, new_price);

    settlement::exercise_option(&env, &holder, id);

    let dc = derivatives::get_contract(&env, id).unwrap();
    assert_eq!(dc.status, DerivativeStatus::Exercised);
}

#[test]
fn test_open_swap_and_match() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let fixed_payer = Address::generate(&env);
    let floating_payer = Address::generate(&env);
    let token = create_token(&env, &admin);
    mint(&env, &token, &fixed_payer, 10_000_000);
    mint(&env, &token, &floating_payer, 10_000_000);

    let now = env.ledger().timestamp();
    let expires_at = now + 86_400 * 90;

    let id = derivatives::open(
        &env,
        &fixed_payer,
        DerivativeKind::Swap,
        &token,
        1_000_000,
        PRICE_PRECISION,
        0,
        expires_at,
    );

    derivatives::match_contract(&env, &floating_payer, id);

    let dc = derivatives::get_contract(&env, id).unwrap();
    assert_eq!(dc.status, DerivativeStatus::Active);
    assert_eq!(dc.kind, DerivativeKind::Swap);
}
