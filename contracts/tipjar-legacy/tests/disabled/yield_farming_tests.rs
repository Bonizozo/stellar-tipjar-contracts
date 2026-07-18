extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
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

    let lp_token = env.register_stellar_asset_contract(token_admin.clone());
    let reward_token = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin);
    client.add_token(&admin, &lp_token);
    client.add_token(&admin, &reward_token);

    (env, client, admin, lp_token, reward_token)
}

fn mint(env: &Env, token: &Address, to: &Address, amount: i128) {
    soroban_sdk::token::StellarAssetClient::new(env, token).mint(to, &amount);
}

fn advance_time(env: &Env, seconds: u64) {
    let ts = env.ledger().timestamp();
    env.ledger().set(LedgerInfo {
        timestamp: ts + seconds,
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 6_312_000,
    });
}

// ── farm_create_pool ──────────────────────────────────────────────────────────

#[test]
fn test_create_pool_returns_id() {
    let (_, client, admin, lp_token, reward_token) = setup();

    let pool_id = client.farm_create_pool(
        &admin,
        &lp_token,
        &reward_token,
        &2_000u32, // 20% APY
        &0u64,     // no lock
    );

    assert_eq!(pool_id, 1);
}

#[test]
fn test_create_pool_increments_id() {
    let (_, client, admin, lp_token, reward_token) = setup();

    let id1 = client.farm_create_pool(&admin, &lp_token, &reward_token, &1_000u32, &0u64);
    let id2 = client.farm_create_pool(&admin, &lp_token, &reward_token, &1_000u32, &0u64);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

#[test]
fn test_create_pool_unauthorized() {
    let (env, client, _, lp_token, reward_token) = setup();
    let attacker = Address::generate(&env);

    let result = client.try_farm_create_pool(&attacker, &lp_token, &reward_token, &1_000u32, &0u64);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}

#[test]
fn test_create_pool_zero_rate_rejected() {
    let (_, client, admin, lp_token, reward_token) = setup();

    let result = client.try_farm_create_pool(&admin, &lp_token, &reward_token, &0u32, &0u64);
    assert_eq!(result, Err(Ok(TipJarError::InvalidAmount)));
}

// ── farm_get_pool ─────────────────────────────────────────────────────────────

#[test]
fn test_get_pool_returns_config() {
    let (_, client, admin, lp_token, reward_token) = setup();

    let pool_id = client.farm_create_pool(&admin, &lp_token, &reward_token, &2_000u32, &3600u64);
    let pool = client.farm_get_pool(&pool_id).unwrap();

    assert_eq!(pool.id, pool_id);
    assert_eq!(pool.lp_token, lp_token);
    assert_eq!(pool.reward_token, reward_token);
    assert_eq!(pool.reward_rate_bps, 2_000);
    assert_eq!(pool.lock_period, 3600);
    assert_eq!(pool.total_staked, 0);
}

#[test]
fn test_get_pool_nonexistent_returns_none() {
    let (_, client, _, _, _) = setup();
    assert!(client.farm_get_pool(&999u64).is_none());
}

// ── farm_stake ────────────────────────────────────────────────────────────────

#[test]
fn test_stake_updates_pool_and_position() {
    let (env, client, admin, lp_token, reward_token) = setup();
    let staker = Address::generate(&env);
    mint(&env, &lp_token, &staker, 1_000_000);

    let pool_id = client.farm_create_pool(&admin, &lp_token, &reward_token, &2_000u32, &0u64);
    client.farm_stake(&staker, &pool_id, &500_000i128);

    let pool = client.farm_get_pool(&pool_id).unwrap();
    assert_eq!(pool.total_staked, 500_000);

    let pos = client.farm_get_position(&pool_id, &staker).unwrap();
    assert_eq!(pos.amount, 500_000);
    assert_eq!(pos.staker, staker);
}

#[test]
fn test_stake_zero_rejected() {
    let (env, client, admin, lp_token, reward_token) = setup();
    let staker = Address::generate(&env);

    let pool_id = client.farm_create_pool(&admin, &lp_token, &reward_token, &2_000u32, &0u64);
    let result = client.try_farm_stake(&staker, &pool_id, &0i128);
    assert!(result.is_err());
}

#[test]
fn test_stake_accumulates() {
    let (env, client, admin, lp_token, reward_token) = setup();
    let staker = Address::generate(&env);
    mint(&env, &lp_token, &staker, 2_000_000);

    let pool_id = client.farm_create_pool(&admin, &lp_token, &reward_token, &2_000u32, &0u64);
    client.farm_stake(&staker, &pool_id, &500_000i128);
    client.farm_stake(&staker, &pool_id, &300_000i128);

    let pos = client.farm_get_position(&pool_id, &staker).unwrap();
    assert_eq!(pos.amount, 800_000);
}

// ── farm_unstake ──────────────────────────────────────────────────────────────

#[test]
fn test_unstake_after_lock_succeeds() {
    let (env, client, admin, lp_token, reward_token) = setup();
    let staker = Address::generate(&env);
    mint(&env, &lp_token, &staker, 1_000_000);

    let lock_period = 3600u64;
    let pool_id = client.farm_create_pool(&admin, &lp_token, &reward_token, &2_000u32, &lock_period);
    client.farm_stake(&staker, &pool_id, &1_000_000i128);

    advance_time(&env, lock_period);

    client.farm_unstake(&staker, &pool_id, &1_000_000i128);

    let pool = client.farm_get_pool(&pool_id).unwrap();
    assert_eq!(pool.total_staked, 0);
}

#[test]
fn test_unstake_before_lock_fails() {
    let (env, client, admin, lp_token, reward_token) = setup();
    let staker = Address::generate(&env);
    mint(&env, &lp_token, &staker, 1_000_000);

    let pool_id = client.farm_create_pool(&admin, &lp_token, &reward_token, &2_000u32, &3600u64);
    client.farm_stake(&staker, &pool_id, &1_000_000i128);

    let result = client.try_farm_unstake(&staker, &pool_id, &1_000_000i128);
    assert_eq!(result, Err(Ok(TipJarError::FarmingLockNotExpired)));
}

// ── farm_harvest ──────────────────────────────────────────────────────────────

#[test]
fn test_harvest_accrues_rewards_over_time() {
    let (env, client, admin, lp_token, reward_token) = setup();
    let staker = Address::generate(&env);
    mint(&env, &lp_token, &staker, 1_000_000);
    // Fund contract with reward tokens so it can pay out
    mint(&env, &reward_token, &env.current_contract_address(), &10_000_000);

    let pool_id = client.farm_create_pool(&admin, &lp_token, &reward_token, &2_000u32, &0u64);
    client.farm_stake(&staker, &pool_id, &1_000_000i128);

    // Advance half a year
    advance_time(&env, 31_536_000 / 2);

    let harvested = client.farm_harvest(&staker, &pool_id);
    // 20% APY on 1_000_000 for half a year ≈ 100_000
    assert!(harvested > 0);
}

#[test]
fn test_harvest_no_stake_fails() {
    let (env, client, admin, lp_token, reward_token) = setup();
    let staker = Address::generate(&env);

    let pool_id = client.farm_create_pool(&admin, &lp_token, &reward_token, &2_000u32, &0u64);
    let result = client.try_farm_harvest(&staker, &pool_id);
    assert!(result.is_err());
}

// ── farm_get_position ─────────────────────────────────────────────────────────

#[test]
fn test_get_position_nonexistent_returns_none() {
    let (env, client, admin, lp_token, reward_token) = setup();
    let staker = Address::generate(&env);

    let pool_id = client.farm_create_pool(&admin, &lp_token, &reward_token, &2_000u32, &0u64);
    assert!(client.farm_get_position(&pool_id, &staker).is_none());
}
