#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env,
};
use tipjar::{TipJarContract, TipJarContractClient, TipJarError};

// ── helpers ───────────────────────────────────────────────────────────────────

const PRECISION: i128 = 10_000_000;

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
    let base_token = env.register_stellar_asset_contract(token_admin.clone());
    let quote_token = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin, &0u32, &0u64);

    let updater = Address::generate(&env);
    (env, client, admin, updater, base_token, quote_token)
}

fn advance_time(env: &Env, seconds: u64) {
    let current = env.ledger().timestamp();
    env.ledger().set(LedgerInfo {
        timestamp: current + seconds,
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 6_312_000,
    });
}

// ── twap_create_oracle ────────────────────────────────────────────────────────

#[test]
fn test_create_oracle_returns_id() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater,
        &updater,
        &base,
        &quote,
        &1_800u64,
        &10u32,
        &(PRECISION),
    );
    assert_eq!(id, 1);
}

#[test]
fn test_create_oracle_stores_config() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater,
        &updater,
        &base,
        &quote,
        &3_600u64,
        &20u32,
        &(2 * PRECISION),
    );

    let oracle = client.twap_get_oracle(&id);
    assert_eq!(oracle.window_seconds, 3_600);
    assert_eq!(oracle.max_observations, 20);
    assert_eq!(oracle.last_price, 2 * PRECISION);
    assert!(oracle.active);
    assert_eq!(oracle.observation_count, 1);
}

#[test]
fn test_create_oracle_increments_id() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id1 = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );
    let id2 = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

#[test]
fn test_create_oracle_invalid_price_fails() {
    let (env, client, _admin, updater, base, quote) = setup();

    let result =
        client.try_twap_create_oracle(&updater, &updater, &base, &quote, &1_800u64, &10u32, &0i128);
    assert_eq!(result, Err(Ok(TipJarError::TwapInvalidPrice)));
}

#[test]
fn test_create_oracle_window_too_small_fails() {
    let (env, client, _admin, updater, base, quote) = setup();

    let result = client.try_twap_create_oracle(
        &updater, &updater, &base, &quote, &10u64, &10u32, &PRECISION,
    );
    assert_eq!(result, Err(Ok(TipJarError::TwapInvalidWindow)));
}

#[test]
fn test_create_oracle_window_too_large_fails() {
    let (env, client, _admin, updater, base, quote) = setup();

    let result = client.try_twap_create_oracle(
        &updater,
        &updater,
        &base,
        &quote,
        &(8 * 24 * 3600u64), // 8 days — exceeds max
        &10u32,
        &PRECISION,
    );
    assert_eq!(result, Err(Ok(TipJarError::TwapInvalidWindow)));
}

#[test]
fn test_create_oracle_capacity_too_small_fails() {
    let (env, client, _admin, updater, base, quote) = setup();

    let result = client.try_twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &1u32, &PRECISION,
    );
    assert_eq!(result, Err(Ok(TipJarError::TwapInvalidParams)));
}

#[test]
fn test_create_oracle_capacity_too_large_fails() {
    let (env, client, _admin, updater, base, quote) = setup();

    let result = client.try_twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &300u32, &PRECISION,
    );
    assert_eq!(result, Err(Ok(TipJarError::TwapInvalidParams)));
}

// ── twap_record_price ─────────────────────────────────────────────────────────

#[test]
fn test_record_price_updates_last_price() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    advance_time(&env, 60);
    client.twap_record_price(&updater, &id, &(2 * PRECISION));

    let oracle = client.twap_get_oracle(&id);
    assert_eq!(oracle.last_price, 2 * PRECISION);
    assert_eq!(oracle.observation_count, 2);
}

#[test]
fn test_record_price_increments_observation_count() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    for i in 1..=5u64 {
        advance_time(&env, 60);
        client.twap_record_price(&updater, &id, &((i as i128 + 1) * PRECISION));
    }

    let oracle = client.twap_get_oracle(&id);
    assert_eq!(oracle.observation_count, 6); // 1 seed + 5 updates
}

#[test]
fn test_record_price_zero_fails() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    advance_time(&env, 60);
    let result = client.try_twap_record_price(&updater, &id, &0i128);
    assert_eq!(result, Err(Ok(TipJarError::TwapInvalidPrice)));
}

#[test]
fn test_record_price_wrong_updater_fails() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    let intruder = Address::generate(&env);
    advance_time(&env, 60);
    let result = client.try_twap_record_price(&intruder, &id, &(2 * PRECISION));
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}

#[test]
fn test_record_price_too_frequent_fails() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    // No time advance — same timestamp as seed
    let result = client.try_twap_record_price(&updater, &id, &(2 * PRECISION));
    assert_eq!(result, Err(Ok(TipJarError::TwapUpdateTooFrequent)));
}

#[test]
fn test_record_price_inactive_oracle_fails() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    client.twap_deactivate(&updater, &id);

    advance_time(&env, 60);
    let result = client.try_twap_record_price(&updater, &id, &(2 * PRECISION));
    assert_eq!(result, Err(Ok(TipJarError::TwapOracleInactive)));
}

// ── twap_get_latest_price ─────────────────────────────────────────────────────

#[test]
fn test_get_latest_price_returns_seed() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater,
        &updater,
        &base,
        &quote,
        &1_800u64,
        &10u32,
        &(5 * PRECISION),
    );

    let price = client.twap_get_latest_price(&id);
    assert_eq!(price, 5 * PRECISION);
}

#[test]
fn test_get_latest_price_reflects_update() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    advance_time(&env, 300);
    client.twap_record_price(&updater, &id, &(7 * PRECISION));

    let price = client.twap_get_latest_price(&id);
    assert_eq!(price, 7 * PRECISION);
}

// ── twap_get_twap ─────────────────────────────────────────────────────────────

#[test]
fn test_twap_with_single_observation_returns_spot() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater,
        &updater,
        &base,
        &quote,
        &1_800u64,
        &10u32,
        &(3 * PRECISION),
    );

    // Only seed observation — TWAP falls back to spot price
    let result = client.twap_get_twap(&id);
    assert_eq!(result.twap, 3 * PRECISION);
    assert_eq!(result.observations_used, 1);
}

#[test]
fn test_twap_constant_price_equals_spot() {
    let (env, client, _admin, updater, base, quote) = setup();

    let price = 4 * PRECISION;
    let id =
        client.twap_create_oracle(&updater, &updater, &base, &quote, &1_800u64, &10u32, &price);

    // Record the same price multiple times
    for _ in 0..5 {
        advance_time(&env, 300);
        client.twap_record_price(&updater, &id, &price);
    }

    let result = client.twap_get_twap(&id);
    // TWAP of a constant price should equal that price
    assert_eq!(result.twap, price);
    assert!(result.observations_used >= 2);
}

#[test]
fn test_twap_averages_rising_prices() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater,
        &updater,
        &base,
        &quote,
        &3_600u64,
        &20u32,
        &(1 * PRECISION),
    );

    // Record prices 1, 2, 3, 4, 5 at equal intervals
    for i in 2..=5i128 {
        advance_time(&env, 600);
        client.twap_record_price(&updater, &id, &(i * PRECISION));
    }

    let result = client.twap_get_twap(&id);
    // TWAP should be between min (1×) and max (5×) price
    assert!(result.twap > PRECISION);
    assert!(result.twap < 5 * PRECISION);
    assert!(result.observations_used >= 2);
}

#[test]
fn test_twap_custom_window() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &3_600u64, &20u32, &PRECISION,
    );

    // Record several prices over 30 minutes
    for i in 1..=6u64 {
        advance_time(&env, 300);
        client.twap_record_price(&updater, &id, &((i as i128) * PRECISION));
    }

    // Short window (5 min) should use fewer observations than long window (30 min)
    let short = client.twap_get_twap_window(&id, &300u64);
    let long = client.twap_get_twap_window(&id, &1_800u64);

    assert!(long.observations_used >= short.observations_used);
}

// ── twap_get_observations ─────────────────────────────────────────────────────

#[test]
fn test_get_observations_returns_seed() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater,
        &updater,
        &base,
        &quote,
        &1_800u64,
        &10u32,
        &(2 * PRECISION),
    );

    let obs = client.twap_get_observations(&id, &5u32);
    assert_eq!(obs.len(), 1);
    assert_eq!(obs.get(0).unwrap().price, 2 * PRECISION);
}

#[test]
fn test_get_observations_chronological_order() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    advance_time(&env, 60);
    client.twap_record_price(&updater, &id, &(2 * PRECISION));
    advance_time(&env, 60);
    client.twap_record_price(&updater, &id, &(3 * PRECISION));

    let obs = client.twap_get_observations(&id, &10u32);
    assert_eq!(obs.len(), 3);

    // Verify chronological order: timestamps should be non-decreasing
    for i in 1..obs.len() {
        assert!(obs.get(i).unwrap().timestamp >= obs.get(i - 1).unwrap().timestamp);
    }
}

#[test]
fn test_get_observations_limit_respected() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    for _ in 0..8 {
        advance_time(&env, 60);
        client.twap_record_price(&updater, &id, &(2 * PRECISION));
    }

    let obs = client.twap_get_observations(&id, &3u32);
    assert_eq!(obs.len(), 3);
}

// ── ring buffer wrap-around ───────────────────────────────────────────────────

#[test]
fn test_ring_buffer_wraps_around() {
    let (env, client, _admin, updater, base, quote) = setup();

    // Small capacity of 4 to force wrap-around quickly
    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &4u32, &PRECISION,
    );

    // Write 6 observations — more than capacity
    for i in 1..=6u64 {
        advance_time(&env, 60);
        client.twap_record_price(&updater, &id, &((i as i128 + 1) * PRECISION));
    }

    let oracle = client.twap_get_oracle(&id);
    assert_eq!(oracle.observation_count, 7); // 1 seed + 6 updates

    // TWAP should still work after wrap-around
    let result = client.twap_get_twap(&id);
    assert!(result.twap > 0);
}

// ── twap_update_config ────────────────────────────────────────────────────────

#[test]
fn test_update_config_changes_window() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    client.twap_update_config(&updater, &id, &3_600u64, &updater);

    let oracle = client.twap_get_oracle(&id);
    assert_eq!(oracle.window_seconds, 3_600);
}

#[test]
fn test_update_config_changes_updater() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    let new_updater = Address::generate(&env);
    client.twap_update_config(&updater, &id, &1_800u64, &new_updater);

    // New updater can now record prices
    advance_time(&env, 60);
    client.twap_record_price(&new_updater, &id, &(2 * PRECISION));

    let oracle = client.twap_get_oracle(&id);
    assert_eq!(oracle.last_price, 2 * PRECISION);
}

#[test]
fn test_update_config_wrong_updater_fails() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    let intruder = Address::generate(&env);
    let result = client.try_twap_update_config(&intruder, &id, &3_600u64, &intruder);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}

#[test]
fn test_update_config_invalid_window_fails() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    let result = client.try_twap_update_config(&updater, &id, &5u64, &updater);
    assert_eq!(result, Err(Ok(TipJarError::TwapInvalidWindow)));
}

// ── twap_deactivate ───────────────────────────────────────────────────────────

#[test]
fn test_deactivate_oracle() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    client.twap_deactivate(&updater, &id);

    let oracle = client.twap_get_oracle(&id);
    assert!(!oracle.active);
}

#[test]
fn test_deactivate_already_inactive_fails() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    client.twap_deactivate(&updater, &id);

    let result = client.try_twap_deactivate(&updater, &id);
    assert_eq!(result, Err(Ok(TipJarError::TwapOracleInactive)));
}

#[test]
fn test_deactivate_wrong_updater_fails() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    let intruder = Address::generate(&env);
    let result = client.try_twap_deactivate(&intruder, &id);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}

// ── not found ─────────────────────────────────────────────────────────────────

#[test]
fn test_get_oracle_not_found_fails() {
    let (_env, client, ..) = setup();

    let result = client.try_twap_get_oracle(&999u64);
    assert_eq!(result, Err(Ok(TipJarError::TwapOracleNotFound)));
}

// ── pause guard ───────────────────────────────────────────────────────────────

#[test]
fn test_record_price_paused_fails() {
    let (env, client, admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    let reason = soroban_sdk::String::from_str(&env, "maintenance");
    client.pause(&admin, &reason);

    advance_time(&env, 60);
    let result = client.try_twap_record_price(&updater, &id, &(2 * PRECISION));
    assert_eq!(result, Err(Ok(TipJarError::ContractPaused)));
}

#[test]
fn test_create_oracle_paused_fails() {
    let (env, client, admin, updater, base, quote) = setup();

    let reason = soroban_sdk::String::from_str(&env, "maintenance");
    client.pause(&admin, &reason);

    let result = client.try_twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );
    assert_eq!(result, Err(Ok(TipJarError::ContractPaused)));
}

// ── price accumulator correctness ─────────────────────────────────────────────

#[test]
fn test_accumulator_grows_with_time() {
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &1_800u64, &10u32, &PRECISION,
    );

    advance_time(&env, 300);
    client.twap_record_price(&updater, &id, &PRECISION);

    let obs = client.twap_get_observations(&id, &10u32);
    // Second observation should have a positive accumulator
    assert!(obs.len() >= 2);
    let second = obs.get(1).unwrap();
    assert!(second.price_cumulative > 0);
}

#[test]
fn test_twap_manipulation_resistance() {
    // Simulate a spike: price goes 1 → 100 → 1 over a short window.
    // The TWAP should be much closer to 1 than to 100.
    let (env, client, _admin, updater, base, quote) = setup();

    let id = client.twap_create_oracle(
        &updater, &updater, &base, &quote, &3_600u64, &20u32, &PRECISION,
    );

    // Normal price for 25 minutes
    for _ in 0..5 {
        advance_time(&env, 300);
        client.twap_record_price(&updater, &id, &PRECISION);
    }

    // Spike for 1 minute
    advance_time(&env, 60);
    client.twap_record_price(&updater, &id, &(100 * PRECISION));

    // Back to normal for 5 minutes
    advance_time(&env, 300);
    client.twap_record_price(&updater, &id, &PRECISION);

    let result = client.twap_get_twap(&id);
    // TWAP should be much less than 100× — spike is diluted by time
    assert!(result.twap < 10 * PRECISION);
    assert!(result.twap >= PRECISION);
}
