#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env,
};
use tipjar::{TipJarContract, TipJarContractClient, TipJarError};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Returns (env, client, admin, lp_token, reward_token).
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

    let lp_token = env.register_stellar_asset_contract(token_admin.clone());
    let reward_token = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin, &0u32, &0u64);
    client.add_token(&admin, &lp_token);
    client.add_token(&admin, &reward_token);

    // Mint reward tokens to admin for program funding
    soroban_sdk::token::StellarAssetClient::new(&env, &reward_token).mint(&admin, &10_000_000i128);

    (env, client, admin, lp_token, reward_token)
}

/// Mints `amount` LP tokens to `provider`.
fn mint_lp(env: &Env, lp_token: &Address, provider: &Address, amount: i128) {
    soroban_sdk::token::StellarAssetClient::new(env, lp_token).mint(provider, &amount);
}

/// Advances ledger time by `seconds`.
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

// ── lm_create_program ─────────────────────────────────────────────────────────

#[test]
fn test_create_program_returns_id() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &2_000u32,      // 20% APY
        &0u64,          // no cliff
        &31_536_000u64, // 1 year vesting
        &0u64,          // no end
    );

    assert_eq!(program_id, 1);
}

#[test]
fn test_create_program_stores_config() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &500_000i128,
        &1_000u32,
        &86_400u64,    // 1 day cliff
        &2_592_000u64, // 30 day vesting
        &0u64,
    );

    let program = client.lm_get_program(&program_id);
    assert_eq!(program.total_rewards, 500_000);
    assert_eq!(program.reward_rate_bps, 1_000);
    assert_eq!(program.vesting_cliff, 86_400);
    assert_eq!(program.vesting_duration, 2_592_000);
    assert!(program.active);
    assert_eq!(program.total_staked, 0);
}

#[test]
fn test_create_program_zero_rewards_fails() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let result = client.try_lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &0i128,
        &1_000u32,
        &0u64,
        &86_400u64,
        &0u64,
    );
    assert_eq!(result, Err(Ok(TipJarError::InvalidAmount)));
}

#[test]
fn test_create_program_zero_rate_fails() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let result = client.try_lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &0u32,
        &0u64,
        &86_400u64,
        &0u64,
    );
    assert_eq!(result, Err(Ok(TipJarError::LmInvalidRate)));
}

#[test]
fn test_create_program_cliff_exceeds_duration_fails() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let result = client.try_lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &1_000u32,
        &2_000u64, // cliff > duration
        &1_000u64,
        &0u64,
    );
    assert_eq!(result, Err(Ok(TipJarError::LmInvalidVesting)));
}

// ── lm_stake ──────────────────────────────────────────────────────────────────

#[test]
fn test_stake_updates_position_and_program() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &2_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    let provider = Address::generate(&env);
    mint_lp(&env, &lp_token, &provider, 50_000i128);

    client.lm_stake(&provider, &program_id, &50_000i128);

    let position = client.lm_get_position(&provider, &program_id);
    assert_eq!(position.staked_amount, 50_000);

    let program = client.lm_get_program(&program_id);
    assert_eq!(program.total_staked, 50_000);
}

#[test]
fn test_stake_zero_amount_fails() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &2_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    let provider = Address::generate(&env);
    let result = client.try_lm_stake(&provider, &program_id, &0i128);
    assert_eq!(result, Err(Ok(TipJarError::InvalidAmount)));
}

#[test]
fn test_stake_inactive_program_fails() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &2_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    client.lm_deactivate_program(&admin, &program_id);

    let provider = Address::generate(&env);
    mint_lp(&env, &lp_token, &provider, 1_000i128);

    let result = client.try_lm_stake(&provider, &program_id, &1_000i128);
    assert_eq!(result, Err(Ok(TipJarError::LmProgramInactive)));
}

#[test]
fn test_stake_multiple_providers() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &2_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    let p1 = Address::generate(&env);
    let p2 = Address::generate(&env);
    mint_lp(&env, &lp_token, &p1, 30_000i128);
    mint_lp(&env, &lp_token, &p2, 20_000i128);

    client.lm_stake(&p1, &program_id, &30_000i128);
    client.lm_stake(&p2, &program_id, &20_000i128);

    let program = client.lm_get_program(&program_id);
    assert_eq!(program.total_staked, 50_000);
}

// ── lm_unstake ────────────────────────────────────────────────────────────────

#[test]
fn test_unstake_reduces_position() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &2_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    let provider = Address::generate(&env);
    mint_lp(&env, &lp_token, &provider, 10_000i128);
    client.lm_stake(&provider, &program_id, &10_000i128);

    advance_time(&env, 1_000);
    client.lm_unstake(&provider, &program_id, &4_000i128);

    let position = client.lm_get_position(&provider, &program_id);
    assert_eq!(position.staked_amount, 6_000);

    let program = client.lm_get_program(&program_id);
    assert_eq!(program.total_staked, 6_000);
}

#[test]
fn test_unstake_more_than_staked_fails() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &2_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    let provider = Address::generate(&env);
    mint_lp(&env, &lp_token, &provider, 5_000i128);
    client.lm_stake(&provider, &program_id, &5_000i128);

    let result = client.try_lm_unstake(&provider, &program_id, &6_000i128);
    assert_eq!(result, Err(Ok(TipJarError::InsufficientBalance)));
}

// ── reward accrual ────────────────────────────────────────────────────────────

#[test]
fn test_rewards_accrue_over_time() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &10_000u32, // 100% APY
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    let provider = Address::generate(&env);
    mint_lp(&env, &lp_token, &provider, 100_000i128);
    client.lm_stake(&provider, &program_id, &100_000i128);

    // Advance half a year
    advance_time(&env, 31_536_000 / 2);

    let pending = client.lm_get_pending_rewards(&provider, &program_id);
    // At 100% APY, 100_000 staked for half a year ≈ 50_000 rewards
    assert!(pending > 0);
    assert!(pending <= 50_000);
}

#[test]
fn test_no_rewards_before_staking() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &2_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    let provider = Address::generate(&env);
    let pending = client.lm_get_pending_rewards(&provider, &program_id);
    assert_eq!(pending, 0);
}

// ── vesting ───────────────────────────────────────────────────────────────────

#[test]
fn test_nothing_vested_before_cliff() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let cliff = 86_400u64; // 1 day
    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &10_000u32,
        &cliff,
        &(cliff * 30),
        &0u64,
    );

    let provider = Address::generate(&env);
    mint_lp(&env, &lp_token, &provider, 100_000i128);
    client.lm_stake(&provider, &program_id, &100_000i128);

    // Advance less than cliff
    advance_time(&env, cliff - 1);

    let info = client.lm_get_vesting_info(&provider, &program_id);
    assert_eq!(info.vested, 0);
    assert!(info.cliff_remaining > 0);
}

#[test]
fn test_rewards_vest_after_cliff() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let cliff = 86_400u64;
    let duration = cliff * 30;
    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &10_000u32,
        &cliff,
        &duration,
        &0u64,
    );

    let provider = Address::generate(&env);
    mint_lp(&env, &lp_token, &provider, 100_000i128);
    client.lm_stake(&provider, &program_id, &100_000i128);

    // Advance past cliff
    advance_time(&env, cliff + 1);

    let info = client.lm_get_vesting_info(&provider, &program_id);
    assert_eq!(info.cliff_remaining, 0);
    assert!(info.vested > 0 || info.total_earned == 0); // vested if any earned
}

#[test]
fn test_claim_before_cliff_fails() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let cliff = 86_400u64;
    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &10_000u32,
        &cliff,
        &(cliff * 30),
        &0u64,
    );

    let provider = Address::generate(&env);
    mint_lp(&env, &lp_token, &provider, 100_000i128);
    client.lm_stake(&provider, &program_id, &100_000i128);

    // Advance less than cliff — nothing vested
    advance_time(&env, cliff / 2);

    let result = client.try_lm_claim_rewards(&provider, &program_id);
    assert_eq!(result, Err(Ok(TipJarError::LmNothingToClaim)));
}

#[test]
fn test_claim_after_cliff_succeeds() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let cliff = 0u64; // no cliff for simplicity
    let duration = 31_536_000u64;
    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &10_000u32, // 100% APY
        &cliff,
        &duration,
        &0u64,
    );

    let provider = Address::generate(&env);
    mint_lp(&env, &lp_token, &provider, 100_000i128);
    client.lm_stake(&provider, &program_id, &100_000i128);

    // Advance full vesting duration
    advance_time(&env, duration);

    let claimed = client.lm_claim_rewards(&provider, &program_id);
    assert!(claimed > 0);

    let info = client.lm_get_vesting_info(&provider, &program_id);
    assert_eq!(info.vesting_remaining, 0);
}

// ── boosting ──────────────────────────────────────────────────────────────────

#[test]
fn test_boost_increases_multiplier() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &2_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    let provider = Address::generate(&env);
    mint_lp(&env, &lp_token, &provider, 10_000i128);
    client.lm_stake(&provider, &program_id, &10_000i128);

    // Lock for 6 months → ~2× boost
    let lock_duration = 31_536_000u64 / 2;
    client.lm_apply_boost(&provider, &program_id, &lock_duration);

    let position = client.lm_get_position(&provider, &program_id);
    // Boost should be > 1× (MIN_BOOST = 10_000_000)
    assert!(position.boost_multiplier > 10_000_000);
    // Boost should be <= 3× (MAX_BOOST = 30_000_000)
    assert!(position.boost_multiplier <= 30_000_000);
}

#[test]
fn test_full_year_lock_gives_max_boost() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &2_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    let provider = Address::generate(&env);
    mint_lp(&env, &lp_token, &provider, 10_000i128);
    client.lm_stake(&provider, &program_id, &10_000i128);

    // Lock for 1 full year → max boost (3×)
    client.lm_apply_boost(&provider, &program_id, &31_536_000u64);

    let position = client.lm_get_position(&provider, &program_id);
    assert_eq!(position.boost_multiplier, 30_000_000); // MAX_BOOST
}

#[test]
fn test_boost_zero_duration_fails() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &2_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    let provider = Address::generate(&env);
    mint_lp(&env, &lp_token, &provider, 10_000i128);
    client.lm_stake(&provider, &program_id, &10_000i128);

    let result = client.try_lm_apply_boost(&provider, &program_id, &0u64);
    assert_eq!(result, Err(Ok(TipJarError::InvalidDuration)));
}

#[test]
fn test_boost_lower_than_current_fails() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &2_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    let provider = Address::generate(&env);
    mint_lp(&env, &lp_token, &provider, 10_000i128);
    client.lm_stake(&provider, &program_id, &10_000i128);

    // Apply a high boost first
    client.lm_apply_boost(&provider, &program_id, &31_536_000u64);

    // Try to apply a lower boost — should fail
    let result = client.try_lm_apply_boost(&provider, &program_id, &1_000u64);
    assert_eq!(result, Err(Ok(TipJarError::LmBoostTooLow)));
}

// ── deactivation ──────────────────────────────────────────────────────────────

#[test]
fn test_deactivate_program() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &2_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    client.lm_deactivate_program(&admin, &program_id);

    let program = client.lm_get_program(&program_id);
    assert!(!program.active);
}

#[test]
fn test_deactivate_already_inactive_fails() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &2_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    client.lm_deactivate_program(&admin, &program_id);

    let result = client.try_lm_deactivate_program(&admin, &program_id);
    assert_eq!(result, Err(Ok(TipJarError::LmProgramInactive)));
}

// ── provider program tracking ─────────────────────────────────────────────────

#[test]
fn test_provider_programs_tracked() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let p1 = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &500_000i128,
        &2_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );
    let p2 = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &500_000i128,
        &3_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    let provider = Address::generate(&env);
    mint_lp(&env, &lp_token, &provider, 20_000i128);
    client.lm_stake(&provider, &p1, &10_000i128);
    client.lm_stake(&provider, &p2, &10_000i128);

    let programs = client.lm_get_provider_programs(&provider);
    assert_eq!(programs.len(), 2);
}

// ── pause guard ───────────────────────────────────────────────────────────────

#[test]
fn test_lm_stake_paused_fails() {
    let (env, client, admin, lp_token, reward_token) = setup();

    let program_id = client.lm_create_program(
        &admin,
        &lp_token,
        &reward_token,
        &1_000_000i128,
        &2_000u32,
        &0u64,
        &31_536_000u64,
        &0u64,
    );

    let reason = soroban_sdk::String::from_str(&env, "maintenance");
    client.pause(&admin, &reason);

    let provider = Address::generate(&env);
    mint_lp(&env, &lp_token, &provider, 1_000i128);

    let result = client.try_lm_stake(&provider, &program_id, &1_000i128);
    assert_eq!(result, Err(Ok(TipJarError::ContractPaused)));
}
