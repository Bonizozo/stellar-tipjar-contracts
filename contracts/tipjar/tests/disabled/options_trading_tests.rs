#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token, Address, Env, String,
};

mod tipjar {
    soroban_sdk::contractimport!(file = "../target/wasm32-unknown-unknown/release/tipjar.wasm");
}

use tipjar::{Client as TipJarClient, OptionStatus, OptionType};

fn create_token_contract<'a>(env: &Env, admin: &Address) -> token::StellarAssetClient<'a> {
    token::StellarAssetClient::new(env, &env.register_stellar_asset_contract(admin.clone()))
}

fn setup_test_env() -> (
    Env,
    TipJarClient,
    Address,
    Address,
    Address,
    token::StellarAssetClient,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let writer = Address::generate(&env);
    let buyer = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token = create_token_contract(&env, &token_admin);

    // Mint tokens to writer and buyer
    token.mint(&writer, &100_000_000);
    token.mint(&buyer, &100_000_000);

    let contract_id = env.register_contract(None, tipjar::WASM);
    let client = TipJarClient::new(&env, &contract_id);

    // Initialize contract
    client.init(&admin);
    client.add_token(&admin, &token.address);

    // Initialize options trading
    client.init_options_trading(&admin);

    (env, client, admin, writer, buyer, token)
}

#[test]
fn test_write_call_option() {
    let (env, client, _admin, writer, _buyer, token) = setup_test_env();

    let strike_price = 1_000_000i128;
    let amount = 10_000_000i128;
    let expiration = env.ledger().timestamp() + 86400; // 1 day

    let option_id = client.write_option(
        &writer,
        &OptionType::Call,
        &token.address,
        &strike_price,
        &amount,
        &expiration,
    );

    assert_eq!(option_id, 0);

    // Verify option was created
    let option = client.get_option(&option_id).unwrap();
    assert_eq!(option.writer, writer);
    assert_eq!(option.option_type, OptionType::Call);
    assert_eq!(option.strike_price, strike_price);
    assert_eq!(option.amount, amount);
    assert_eq!(option.status, OptionStatus::Active);
    assert!(option.holder.is_none());

    // Verify collateral was locked
    assert_eq!(option.collateral, amount);

    // Verify position tracking
    let position = client.get_option_position(&writer);
    assert_eq!(position.written_count, 1);
    assert_eq!(position.total_collateral, amount);
}

#[test]
fn test_write_put_option() {
    let (env, client, _admin, writer, _buyer, token) = setup_test_env();

    let strike_price = 1_000_000i128;
    let amount = 10_000_000i128;
    let expiration = env.ledger().timestamp() + 86400;

    let option_id = client.write_option(
        &writer,
        &OptionType::Put,
        &token.address,
        &strike_price,
        &amount,
        &expiration,
    );

    let option = client.get_option(&option_id).unwrap();
    assert_eq!(option.option_type, OptionType::Put);

    // Put collateral should be strike_price * amount / 1_000_000
    let expected_collateral = (strike_price * amount) / 1_000_000;
    assert_eq!(option.collateral, expected_collateral);
}

#[test]
fn test_buy_option() {
    let (env, client, _admin, writer, buyer, token) = setup_test_env();

    let strike_price = 1_000_000i128;
    let amount = 10_000_000i128;
    let expiration = env.ledger().timestamp() + 86400;

    // Write option
    let option_id = client.write_option(
        &writer,
        &OptionType::Call,
        &token.address,
        &strike_price,
        &amount,
        &expiration,
    );

    // Buy option
    let spot_price = 1_200_000i128; // Spot > strike (in the money)
    client.buy_option(&buyer, &option_id, &spot_price);

    // Verify option has holder
    let option = client.get_option(&option_id).unwrap();
    assert_eq!(option.holder, Some(buyer.clone()));
    assert!(option.premium > 0);

    // Verify buyer position
    let buyer_position = client.get_option_position(&buyer);
    assert_eq!(buyer_position.held_count, 1);
    assert_eq!(buyer_position.premiums_paid, option.premium);

    // Verify writer position
    let writer_position = client.get_option_position(&writer);
    assert_eq!(writer_position.premiums_earned, option.premium);
}

#[test]
fn test_exercise_call_option() {
    let (env, client, _admin, writer, buyer, token) = setup_test_env();

    let strike_price = 1_000_000i128;
    let amount = 10_000_000i128;
    let expiration = env.ledger().timestamp() + 86400;

    // Write and buy option
    let option_id = client.write_option(
        &writer,
        &OptionType::Call,
        &token.address,
        &strike_price,
        &amount,
        &expiration,
    );

    let spot_price = 1_200_000i128;
    client.buy_option(&buyer, &option_id, &spot_price);

    // Exercise option
    let payoff = client.exercise_option(&buyer, &option_id, &spot_price);
    assert!(payoff > 0);

    // Verify option status
    let option = client.get_option(&option_id).unwrap();
    assert_eq!(option.status, OptionStatus::Exercised);

    // Verify buyer position updated
    let buyer_position = client.get_option_position(&buyer);
    assert_eq!(buyer_position.held_count, 0);
}

#[test]
#[should_panic(expected = "Option is out of the money")]
fn test_exercise_out_of_money_option() {
    let (env, client, _admin, writer, buyer, token) = setup_test_env();

    let strike_price = 1_000_000i128;
    let amount = 10_000_000i128;
    let expiration = env.ledger().timestamp() + 86400;

    // Write call option
    let option_id = client.write_option(
        &writer,
        &OptionType::Call,
        &token.address,
        &strike_price,
        &amount,
        &expiration,
    );

    let spot_price = 1_200_000i128;
    client.buy_option(&buyer, &option_id, &spot_price);

    // Try to exercise with spot < strike (out of money)
    let out_of_money_spot = 800_000i128;
    client.exercise_option(&buyer, &option_id, &out_of_money_spot);
}

#[test]
fn test_expire_option() {
    let (env, client, _admin, writer, _buyer, token) = setup_test_env();

    let strike_price = 1_000_000i128;
    let amount = 10_000_000i128;
    let expiration = env.ledger().timestamp() + 100;

    let option_id = client.write_option(
        &writer,
        &OptionType::Call,
        &token.address,
        &strike_price,
        &amount,
        &expiration,
    );

    // Fast forward past expiration
    env.ledger().set(LedgerInfo {
        timestamp: expiration + 1,
        protocol_version: 20,
        sequence_number: 10,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 3110400,
    });

    // Expire option
    client.expire_option(&option_id);

    // Verify status
    let option = client.get_option(&option_id).unwrap();
    assert_eq!(option.status, OptionStatus::Expired);

    // Verify collateral returned to writer
    let writer_position = client.get_option_position(&writer);
    assert_eq!(writer_position.total_collateral, 0);
}

#[test]
fn test_cancel_unsold_option() {
    let (env, client, _admin, writer, _buyer, token) = setup_test_env();

    let strike_price = 1_000_000i128;
    let amount = 10_000_000i128;
    let expiration = env.ledger().timestamp() + 86400;

    let option_id = client.write_option(
        &writer,
        &OptionType::Call,
        &token.address,
        &strike_price,
        &amount,
        &expiration,
    );

    // Cancel before selling
    client.cancel_option(&writer, &option_id);

    // Verify status
    let option = client.get_option(&option_id).unwrap();
    assert_eq!(option.status, OptionStatus::Cancelled);

    // Verify collateral returned
    let writer_position = client.get_option_position(&writer);
    assert_eq!(writer_position.written_count, 0);
    assert_eq!(writer_position.total_collateral, 0);
}

#[test]
#[should_panic(expected = "Option already has a holder")]
fn test_cannot_cancel_sold_option() {
    let (env, client, _admin, writer, buyer, token) = setup_test_env();

    let strike_price = 1_000_000i128;
    let amount = 10_000_000i128;
    let expiration = env.ledger().timestamp() + 86400;

    let option_id = client.write_option(
        &writer,
        &OptionType::Call,
        &token.address,
        &strike_price,
        &amount,
        &expiration,
    );

    // Buy option
    let spot_price = 1_200_000i128;
    client.buy_option(&buyer, &option_id, &spot_price);

    // Try to cancel after selling
    client.cancel_option(&writer, &option_id);
}

#[test]
fn test_calculate_premium() {
    let (env, client, _admin, _writer, _buyer, _token) = setup_test_env();

    let spot_price = 1_000_000i128;
    let strike_price = 1_000_000i128;
    let amount = 10_000_000i128;
    let time_to_expiry = 86400u64; // 1 day

    // Calculate premium for call option
    let call_premium = client.calculate_option_premium(
        &OptionType::Call,
        &spot_price,
        &strike_price,
        &amount,
        &time_to_expiry,
    );
    assert!(call_premium > 0);

    // Calculate premium for put option
    let put_premium = client.calculate_option_premium(
        &OptionType::Put,
        &spot_price,
        &strike_price,
        &amount,
        &time_to_expiry,
    );
    assert!(put_premium > 0);

    // Premium should decrease as time to expiry decreases
    let short_time_premium = client.calculate_option_premium(
        &OptionType::Call,
        &spot_price,
        &strike_price,
        &amount,
        &3600u64, // 1 hour
    );
    assert!(short_time_premium < call_premium);
}

#[test]
fn test_get_written_and_held_options() {
    let (env, client, _admin, writer, buyer, token) = setup_test_env();

    let strike_price = 1_000_000i128;
    let amount = 10_000_000i128;
    let expiration = env.ledger().timestamp() + 86400;

    // Write multiple options
    let option_id_1 = client.write_option(
        &writer,
        &OptionType::Call,
        &token.address,
        &strike_price,
        &amount,
        &expiration,
    );

    let option_id_2 = client.write_option(
        &writer,
        &OptionType::Put,
        &token.address,
        &strike_price,
        &amount,
        &expiration,
    );

    // Verify written options
    let written = client.get_written_options(&writer);
    assert_eq!(written.len(), 2);
    assert!(written.contains(&option_id_1));
    assert!(written.contains(&option_id_2));

    // Buy one option
    let spot_price = 1_200_000i128;
    client.buy_option(&buyer, &option_id_1, &spot_price);

    // Verify held options
    let held = client.get_held_options(&buyer);
    assert_eq!(held.len(), 1);
    assert!(held.contains(&option_id_1));
}

#[test]
fn test_batch_expire_options() {
    let (env, client, _admin, writer, _buyer, token) = setup_test_env();

    let strike_price = 1_000_000i128;
    let amount = 10_000_000i128;
    let expiration = env.ledger().timestamp() + 100;

    // Write multiple options
    let mut option_ids = soroban_sdk::Vec::new(&env);
    for _ in 0..3 {
        let id = client.write_option(
            &writer,
            &OptionType::Call,
            &token.address,
            &strike_price,
            &amount,
            &expiration,
        );
        option_ids.push_back(id);
    }

    // Fast forward past expiration
    env.ledger().set(LedgerInfo {
        timestamp: expiration + 1,
        protocol_version: 20,
        sequence_number: 10,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 3110400,
    });

    // Batch expire
    let expired_count = client.batch_expire_options(&option_ids);
    assert_eq!(expired_count, 3);

    // Verify all expired
    for i in 0..option_ids.len() {
        let id = option_ids.get(i).unwrap();
        let option = client.get_option(&id).unwrap();
        assert_eq!(option.status, OptionStatus::Expired);
    }
}

#[test]
fn test_option_position_tracking() {
    let (env, client, _admin, writer, buyer, token) = setup_test_env();

    let strike_price = 1_000_000i128;
    let amount = 10_000_000i128;
    let expiration = env.ledger().timestamp() + 86400;

    // Initial positions should be empty
    let writer_pos = client.get_option_position(&writer);
    assert_eq!(writer_pos.written_count, 0);
    assert_eq!(writer_pos.held_count, 0);
    assert_eq!(writer_pos.total_collateral, 0);
    assert_eq!(writer_pos.premiums_earned, 0);
    assert_eq!(writer_pos.premiums_paid, 0);

    // Write option
    let option_id = client.write_option(
        &writer,
        &OptionType::Call,
        &token.address,
        &strike_price,
        &amount,
        &expiration,
    );

    // Check writer position after writing
    let writer_pos = client.get_option_position(&writer);
    assert_eq!(writer_pos.written_count, 1);
    assert_eq!(writer_pos.total_collateral, amount);

    // Buy option
    let spot_price = 1_200_000i128;
    client.buy_option(&buyer, &option_id, &spot_price);

    let option = client.get_option(&option_id).unwrap();
    let premium = option.premium;

    // Check positions after buying
    let writer_pos = client.get_option_position(&writer);
    assert_eq!(writer_pos.premiums_earned, premium);

    let buyer_pos = client.get_option_position(&buyer);
    assert_eq!(buyer_pos.held_count, 1);
    assert_eq!(buyer_pos.premiums_paid, premium);

    // Exercise option
    client.exercise_option(&buyer, &option_id, &spot_price);

    // Check positions after exercise
    let buyer_pos = client.get_option_position(&buyer);
    assert_eq!(buyer_pos.held_count, 0);

    let writer_pos = client.get_option_position(&writer);
    assert_eq!(writer_pos.total_collateral, 0);
}

#[test]
fn test_get_active_options() {
    let (env, client, _admin, writer, _buyer, token) = setup_test_env();

    let strike_price = 1_000_000i128;
    let amount = 10_000_000i128;
    let expiration = env.ledger().timestamp() + 86400;

    // Initially no active options
    let active = client.get_active_options();
    assert_eq!(active.len(), 0);

    // Write options
    let id1 = client.write_option(
        &writer,
        &OptionType::Call,
        &token.address,
        &strike_price,
        &amount,
        &expiration,
    );

    let id2 = client.write_option(
        &writer,
        &OptionType::Put,
        &token.address,
        &strike_price,
        &amount,
        &expiration,
    );

    // Check active options
    let active = client.get_active_options();
    assert_eq!(active.len(), 2);
    assert!(active.contains(&id1));
    assert!(active.contains(&id2));

    // Cancel one
    client.cancel_option(&writer, &id1);

    // Check active options again
    let active = client.get_active_options();
    assert_eq!(active.len(), 1);
    assert!(active.contains(&id2));
}
