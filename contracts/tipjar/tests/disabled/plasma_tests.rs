#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, BytesN as _},
    Address, BytesN, Env, Vec,
};

use tipjar::{
    plasma::{ExitStatus, PlasmaBlockStatus},
    DataKey, TipJarContract, TipJarContractClient,
};

fn setup_test_env() -> (Env, TipJarContractClient, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TipJarContract);
    let client = TipJarContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    let token = Address::generate(&env);

    (env, client, admin, operator, token)
}

#[test]
fn test_plasma_block_commit() {
    let (env, _client, _admin, operator, _token) = setup_test_env();

    // Initialize Plasma operator
    env.storage()
        .instance()
        .set(&DataKey::PlasmaOperator, &operator);

    // Commit a Plasma block
    let tx_root = BytesN::random(&env);
    let total_volume = 1_000_000i128;
    let tip_count = 10u32;

    let block_number =
        tipjar::plasma::block::commit_block(&env, &operator, tx_root.clone(), total_volume, tip_count);

    assert_eq!(block_number, 1);

    // Verify block was stored
    let block = tipjar::plasma::block::get_block(&env, block_number).expect("block not found");
    assert_eq!(block.block_number, 1);
    assert_eq!(block.tx_root, tx_root);
    assert_eq!(block.operator, operator);
    assert_eq!(block.total_volume, total_volume);
    assert_eq!(block.tip_count, tip_count);
    assert_eq!(block.status, PlasmaBlockStatus::Committed);
}

#[test]
fn test_plasma_block_finalization() {
    let (env, _client, _admin, operator, _token) = setup_test_env();

    env.storage()
        .instance()
        .set(&DataKey::PlasmaOperator, &operator);

    let tx_root = BytesN::random(&env);
    let block_number =
        tipjar::plasma::block::commit_block(&env, &operator, tx_root, 1_000_000, 10);

    // Advance time by 1 hour (minimum finalization delay)
    env.ledger().with_mut(|li| {
        li.timestamp += 3600;
    });

    // Finalize the block
    tipjar::plasma::block::finalize_block(&env, block_number);

    let block = tipjar::plasma::block::get_block(&env, block_number).expect("block not found");
    assert_eq!(block.status, PlasmaBlockStatus::Finalized);
}

#[test]
#[should_panic(expected = "finalization delay not elapsed")]
fn test_plasma_block_early_finalization_fails() {
    let (env, _client, _admin, operator, _token) = setup_test_env();

    env.storage()
        .instance()
        .set(&DataKey::PlasmaOperator, &operator);

    let tx_root = BytesN::random(&env);
    let block_number =
        tipjar::plasma::block::commit_block(&env, &operator, tx_root, 1_000_000, 10);

    // Try to finalize immediately (should fail)
    tipjar::plasma::block::finalize_block(&env, block_number);
}

#[test]
fn test_plasma_exit_initiation() {
    let (env, _client, _admin, operator, token) = setup_test_env();

    env.storage()
        .instance()
        .set(&DataKey::PlasmaOperator, &operator);

    // Commit and finalize a block
    let tx_root = BytesN::random(&env);
    let block_number =
        tipjar::plasma::block::commit_block(&env, &operator, tx_root.clone(), 1_000_000, 10);

    env.ledger().with_mut(|li| {
        li.timestamp += 3600;
    });
    tipjar::plasma::block::finalize_block(&env, block_number);

    // Initiate an exit
    let exitor = Address::generate(&env);
    let amount = 100_000i128;
    let tx_hash = BytesN::random(&env);
    let proof: Vec<BytesN<32>> = Vec::new(&env);

    let exit_id = tipjar::plasma::exit::initiate_exit(
        &env,
        &exitor,
        block_number,
        token.clone(),
        amount,
        tx_hash.clone(),
        proof,
    );

    assert_eq!(exit_id, 1);

    // Verify exit was stored
    let exit = tipjar::plasma::exit::get_exit(&env, exit_id).expect("exit not found");
    assert_eq!(exit.exit_id, 1);
    assert_eq!(exit.block_number, block_number);
    assert_eq!(exit.exitor, exitor);
    assert_eq!(exit.token, token);
    assert_eq!(exit.amount, amount);
    assert_eq!(exit.tx_hash, tx_hash);
    assert_eq!(exit.status, ExitStatus::Pending);
}

#[test]
fn test_plasma_exit_processing() {
    let (env, _client, _admin, operator, token) = setup_test_env();

    env.storage()
        .instance()
        .set(&DataKey::PlasmaOperator, &operator);

    // Commit and finalize a block
    let tx_root = BytesN::random(&env);
    let block_number =
        tipjar::plasma::block::commit_block(&env, &operator, tx_root.clone(), 1_000_000, 10);

    env.ledger().with_mut(|li| {
        li.timestamp += 3600;
    });
    tipjar::plasma::block::finalize_block(&env, block_number);

    // Initiate an exit
    let exitor = Address::generate(&env);
    let amount = 100_000i128;
    let tx_hash = BytesN::random(&env);
    let proof: Vec<BytesN<32>> = Vec::new(&env);

    let exit_id = tipjar::plasma::exit::initiate_exit(
        &env,
        &exitor,
        block_number,
        token.clone(),
        amount,
        tx_hash,
        proof,
    );

    // Advance time past challenge period (7 days)
    env.ledger().with_mut(|li| {
        li.timestamp += 7 * 24 * 3600;
    });

    // Process the exit
    tipjar::plasma::exit::process_exit(&env, exit_id);

    let exit = tipjar::plasma::exit::get_exit(&env, exit_id).expect("exit not found");
    assert_eq!(exit.status, ExitStatus::Processed);

    // Verify balance was credited
    let balance: i128 = env
        .storage()
        .persistent()
        .get(&DataKey::CreatorBalance(exitor.clone(), token.clone()))
        .unwrap_or(0);
    assert_eq!(balance, amount);
}

#[test]
#[should_panic(expected = "challenge period not elapsed")]
fn test_plasma_exit_early_processing_fails() {
    let (env, _client, _admin, operator, token) = setup_test_env();

    env.storage()
        .instance()
        .set(&DataKey::PlasmaOperator, &operator);

    let tx_root = BytesN::random(&env);
    let block_number =
        tipjar::plasma::block::commit_block(&env, &operator, tx_root.clone(), 1_000_000, 10);

    env.ledger().with_mut(|li| {
        li.timestamp += 3600;
    });
    tipjar::plasma::block::finalize_block(&env, block_number);

    let exitor = Address::generate(&env);
    let tx_hash = BytesN::random(&env);
    let proof: Vec<BytesN<32>> = Vec::new(&env);

    let exit_id = tipjar::plasma::exit::initiate_exit(
        &env,
        &exitor,
        block_number,
        token.clone(),
        100_000,
        tx_hash,
        proof,
    );

    // Try to process immediately (should fail)
    tipjar::plasma::exit::process_exit(&env, exit_id);
}

#[test]
fn test_plasma_exit_challenge() {
    let (env, _client, _admin, operator, token) = setup_test_env();

    env.storage()
        .instance()
        .set(&DataKey::PlasmaOperator, &operator);

    let tx_root = BytesN::random(&env);
    let block_number =
        tipjar::plasma::block::commit_block(&env, &operator, tx_root.clone(), 1_000_000, 10);

    env.ledger().with_mut(|li| {
        li.timestamp += 3600;
    });
    tipjar::plasma::block::finalize_block(&env, block_number);

    let exitor = Address::generate(&env);
    let tx_hash = BytesN::random(&env);
    let proof: Vec<BytesN<32>> = Vec::new(&env);

    let exit_id = tipjar::plasma::exit::initiate_exit(
        &env,
        &exitor,
        block_number,
        token.clone(),
        100_000,
        tx_hash.clone(),
        proof,
    );

    // Challenge the exit with a different spend tx hash
    let challenger = Address::generate(&env);
    let spend_tx_hash = BytesN::random(&env);

    let accepted =
        tipjar::plasma::challenge::challenge_exit(&env, &challenger, exit_id, spend_tx_hash.clone());

    assert!(accepted);

    let exit = tipjar::plasma::exit::get_exit(&env, exit_id).expect("exit not found");
    assert_eq!(exit.status, ExitStatus::Challenged);

    let challenge =
        tipjar::plasma::challenge::get_challenge(&env, exit_id).expect("challenge not found");
    assert_eq!(challenge.exit_id, exit_id);
    assert_eq!(challenge.challenger, challenger);
    assert_eq!(challenge.spend_tx_hash, spend_tx_hash);
}

#[test]
fn test_plasma_multiple_blocks() {
    let (env, _client, _admin, operator, _token) = setup_test_env();

    env.storage()
        .instance()
        .set(&DataKey::PlasmaOperator, &operator);

    // Commit multiple blocks
    for i in 1..=5 {
        let tx_root = BytesN::random(&env);
        let block_number = tipjar::plasma::block::commit_block(
            &env,
            &operator,
            tx_root,
            i as i128 * 100_000,
            i * 10,
        );
        assert_eq!(block_number, i as u64);
    }

    let latest = tipjar::plasma::block::get_latest_block_number(&env);
    assert_eq!(latest, 5);
}

#[test]
fn test_plasma_user_exits_tracking() {
    let (env, _client, _admin, operator, token) = setup_test_env();

    env.storage()
        .instance()
        .set(&DataKey::PlasmaOperator, &operator);

    let tx_root = BytesN::random(&env);
    let block_number =
        tipjar::plasma::block::commit_block(&env, &operator, tx_root.clone(), 1_000_000, 10);

    env.ledger().with_mut(|li| {
        li.timestamp += 3600;
    });
    tipjar::plasma::block::finalize_block(&env, block_number);

    let exitor = Address::generate(&env);

    // Create multiple exits for the same user
    for _ in 0..3 {
        let tx_hash = BytesN::random(&env);
        let proof: Vec<BytesN<32>> = Vec::new(&env);
        tipjar::plasma::exit::initiate_exit(
            &env,
            &exitor,
            block_number,
            token.clone(),
            50_000,
            tx_hash,
            proof,
        );
    }

    let user_exits = tipjar::plasma::exit::get_user_exits(&env, &exitor);
    assert_eq!(user_exits.len(), 3);
}
