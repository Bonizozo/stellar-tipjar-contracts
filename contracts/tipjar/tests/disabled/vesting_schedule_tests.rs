#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};
use tipjar::{DataKey, TipJarContract, TipJarContractClient, TipJarError, VestingSchedule};

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

    let tipper = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&tipper, &10_000_000i128);

    (env, client, tipper, Address::generate(&env), token_id)
}

#[test]
fn test_create_vesting_schedule() {
    let (env, client, tipper, creator, token) = setup();

    let schedule_id = client.create_vesting_schedule(
        &tipper,
        &creator,
        &token,
        &1000i128,
        &86400u64,   // 1 day cliff
        &2592000u64, // 30 days total vesting
    );

    assert_eq!(schedule_id, 0);

    let schedule = client.get_vesting_schedule(&schedule_id).unwrap();
    assert_eq!(schedule.id, 0);
    assert_eq!(schedule.creator, creator);
    assert_eq!(schedule.tipper, tipper);
    assert_eq!(schedule.total_amount, 1000i128);
    assert_eq!(schedule.cliff_duration, 86400u64);
    assert_eq!(schedule.vesting_duration, 2592000u64);
    assert_eq!(schedule.withdrawn, 0);
    assert!(schedule.start_time > 0);
}

#[test]
fn test_vested_amount_before_cliff() {
    let (env, client, tipper, creator, token) = setup();

    let schedule_id = client.create_vesting_schedule(
        &tipper,
        &creator,
        &token,
        &1000i128,
        &86400u64,
        &2592000u64,
    );

    // Before cliff: 0 vested
    let vested = client.get_vested_amount(&schedule_id);
    assert_eq!(vested, 0);
}

#[test]
fn test_vested_amount_after_cliff() {
    let (env, client, tipper, creator, token) = setup();

    let schedule_id = client.create_vesting_schedule(
        &tipper,
        &creator,
        &token,
        &1000i128,
        &86400u64,   // 1 day cliff
        &2592000u64, // 30 days total
    );

    // Advance time past cliff (1 day + 1 second)
    env.ledger().with_mut(|ledger| ledger.timestamp += 86401);

    let vested = client.get_vested_amount(&schedule_id);
    // At cliff + 1 second, approximately 1/30 of tokens should be vested
    // (86401 / 2592000) * 1000 ≈ 33.33
    assert!(vested > 0);
    assert!(vested <= 1000i128);
}

#[test]
fn test_vested_amount_at_full_vesting() {
    let (env, client, tipper, creator, token) = setup();

    let schedule_id = client.create_vesting_schedule(
        &tipper,
        &creator,
        &token,
        &1000i128,
        &86400u64,
        &2592000u64,
    );

    // Advance time past total vesting duration
    env.ledger().with_mut(|ledger| ledger.timestamp += 2592001);

    let vested = client.get_vested_amount(&schedule_id);
    assert_eq!(vested, 1000i128);
}

#[test]
fn test_linear_vesting_progression() {
    let (env, client, tipper, creator, token) = setup();

    let schedule_id = client.create_vesting_schedule(
        &tipper, &creator, &token, &3000i128, &0u64,    // No cliff
        &3000u64, // 3000 seconds total
    );

    // At 50% of vesting period (1500 seconds)
    env.ledger().with_mut(|ledger| ledger.timestamp += 1500);
    let vested_50 = client.get_vested_amount(&schedule_id);
    assert_eq!(vested_50, 1500i128); // 50% of 3000

    // At 75% of vesting period (2250 seconds)
    env.ledger().with_mut(|ledger| ledger.timestamp += 750);
    let vested_75 = client.get_vested_amount(&schedule_id);
    assert_eq!(vested_75, 2250i128); // 75% of 3000
}

#[test]
fn test_withdraw_vested_after_cliff() {
    let (env, client, tipper, creator, token) = setup();

    let schedule_id = client.create_vesting_schedule(
        &tipper, &creator, &token, &3000i128, &1000u64, // 1000 second cliff
        &3000u64, // 3000 second total vesting
    );

    // Advance past cliff
    env.ledger().with_mut(|ledger| ledger.timestamp += 1500);

    let amount_withdrawn = client.withdraw_vested(&creator, &schedule_id);
    assert!(amount_withdrawn > 0);

    let schedule = client.get_vesting_schedule(&schedule_id).unwrap();
    assert_eq!(schedule.withdrawn, amount_withdrawn);
}

#[test]
fn test_withdraw_vested_partial_then_full() {
    let (env, client, tipper, creator, token) = setup();

    let schedule_id = client.create_vesting_schedule(
        &tipper, &creator, &token, &1000i128, &0u64,    // No cliff
        &2000u64, // 2000 seconds total
    );

    // First withdrawal at 25% vesting (500 seconds in)
    env.ledger().with_mut(|ledger| ledger.timestamp += 500);
    let first_withdrawal = client.withdraw_vested(&creator, &schedule_id);
    assert_eq!(first_withdrawal, 250i128);

    // Second withdrawal at 100% vesting
    env.ledger().with_mut(|ledger| ledger.timestamp += 1500);
    let second_withdrawal = client.withdraw_vested(&creator, &schedule_id);
    assert_eq!(second_withdrawal, 750i128);

    // Total withdrawn should equal initial amount
    let schedule = client.get_vesting_schedule(&schedule_id).unwrap();
    assert_eq!(schedule.withdrawn, 1000i128);
}

#[test]
fn test_no_vested_before_cliff() {
    let (env, client, tipper, creator, token) = setup();

    let schedule_id = client.create_vesting_schedule(
        &tipper,
        &creator,
        &token,
        &1000i128,
        &86400u64,
        &2592000u64,
    );

    // Try to withdraw before cliff
    let result = client.try_withdraw_vested(&creator, &schedule_id);
    assert_eq!(result, Err(Ok(TipJarError::NoVestedAmount)));
}

#[test]
fn test_cliff_duration_cannot_exceed_vesting() {
    let (env, client, tipper, creator, token) = setup();

    let result = client.try_create_vesting_schedule(
        &tipper, &creator, &token, &1000i128, &3000u64, // Cliff 3000 seconds
        &2000u64, // But vesting only 2000 seconds
    );

    assert_eq!(result, Err(Ok(TipJarError::CliffExceedsVesting)));
}

#[test]
fn test_invalid_amount() {
    let (env, client, tipper, creator, token) = setup();

    let result = client.try_create_vesting_schedule(
        &tipper,
        &creator,
        &token,
        &0i128, // Invalid amount
        &86400u64,
        &2592000u64,
    );

    assert_eq!(result, Err(Ok(TipJarError::InvalidAmount)));
}

#[test]
fn test_invalid_vesting_duration() {
    let (env, client, tipper, creator, token) = setup();

    let result = client.try_create_vesting_schedule(
        &tipper, &creator, &token, &1000i128, &0u64, &0u64, // Invalid duration
    );

    assert_eq!(result, Err(Ok(TipJarError::InvalidVestingDuration)));
}

#[test]
fn test_get_creator_vesting_schedules() {
    let (env, client, tipper, creator, token) = setup();

    // Create multiple schedules
    let id1 = client.create_vesting_schedule(&tipper, &creator, &token, &1000i128, &0u64, &1000u64);

    let id2 = client.create_vesting_schedule(&tipper, &creator, &token, &2000i128, &0u64, &2000u64);

    let schedules = client.get_creator_vesting_schedules(&creator);
    assert_eq!(schedules.len(), 2);
    assert_eq!(schedules.get(0).unwrap(), id1);
    assert_eq!(schedules.get(1).unwrap(), id2);
}

#[test]
fn test_unauthorized_withdrawal() {
    let (env, client, tipper, creator, token) = setup();
    let other_creator = Address::generate(&env);

    let schedule_id =
        client.create_vesting_schedule(&tipper, &creator, &token, &1000i128, &0u64, &1000u64);

    env.ledger().with_mut(|ledger| ledger.timestamp += 500);

    // Try to withdraw with different creator
    let result = client.try_withdraw_vested(&other_creator, &schedule_id);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}

#[test]
fn test_invalid_schedule_id() {
    let (env, client, _, _, _) = setup();

    let result = client.try_get_vesting_schedule(&0u64);
    assert_eq!(result, None);
}

#[test]
fn test_nonexistent_schedule() {
    let (env, client, _, creator, _) = setup();

    let result = client.try_withdraw_vested(&creator, &999u64);
    assert_eq!(result, Err(Ok(TipJarError::VestingScheduleNotFound)));
}

#[test]
fn test_available_vested_amount() {
    let (env, client, tipper, creator, token) = setup();

    let schedule_id =
        client.create_vesting_schedule(&tipper, &creator, &token, &1000i128, &0u64, &2000u64);

    // At 50% vesting
    env.ledger().with_mut(|ledger| ledger.timestamp += 1000);

    let available = client.get_available_vested_amount(&schedule_id);
    assert_eq!(available, 500i128);

    // Withdraw 300
    client.withdraw_vested(&creator, &schedule_id);

    // Remaining should be adjusted
    let remaining = client.get_available_vested_amount(&schedule_id);
    assert!(remaining > 0);
    assert!(remaining < available);
}

#[test]
fn test_no_cliff_vesting() {
    let (env, client, tipper, creator, token) = setup();

    let schedule_id = client.create_vesting_schedule(
        &tipper, &creator, &token, &2000i128, &0u64, // No cliff
        &2000u64,
    );

    // At 1% of duration, should have vested
    env.ledger().with_mut(|ledger| ledger.timestamp += 20);
    let vested = client.get_vested_amount(&schedule_id);
    assert_eq!(vested, 20i128); // 1% of 2000

    // Available should equal vested (since nothing withdrawn)
    let available = client.get_available_vested_amount(&schedule_id);
    assert_eq!(available, 20i128);
}
