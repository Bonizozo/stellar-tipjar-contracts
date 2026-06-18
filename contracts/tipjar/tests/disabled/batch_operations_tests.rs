#![cfg(test)]

extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env, Vec};
use tipjar::{
    BatchResult, BatchStats, TipJarContract, TipJarContractClient, TipJarError, TipOperation,
    WithdrawOperation,
};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Returns (env, client, admin, tipper, creator, token).
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
    let token_id = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin, &0u32, &0u64);
    client.add_token(&admin, &token_id);

    let tipper = Address::generate(&env);
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&tipper, &10_000_000i128);

    let creator = Address::generate(&env);

    (env, client, admin, tipper, creator, token_id)
}

/// Returns (env, client, admin, tipper, creator, token1, token2).
fn setup_two_tokens() -> (
    Env,
    TipJarContractClient<'static>,
    Address,
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

    let token1 = env.register_stellar_asset_contract(token_admin.clone());
    let token2 = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin, &0u32, &0u64);
    client.add_token(&admin, &token1);
    client.add_token(&admin, &token2);

    let tipper = Address::generate(&env);
    let asset1 = soroban_sdk::token::StellarAssetClient::new(&env, &token1);
    let asset2 = soroban_sdk::token::StellarAssetClient::new(&env, &token2);
    asset1.mint(&tipper, &10_000_000i128);
    asset2.mint(&tipper, &10_000_000i128);

    let creator = Address::generate(&env);

    (env, client, admin, tipper, creator, token1, token2)
}

// ── batch_tip_v2 ──────────────────────────────────────────────────────────────

#[test]
fn test_batch_tip_v2_single_recipient() {
    let (env, client, _admin, tipper, creator, token) = setup();

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    ops.push_back(TipOperation {
        creator: creator.clone(),
        token: token.clone(),
        amount: 1_000i128,
    });

    let results = client.batch_tip_v2(&tipper, &ops);

    assert_eq!(results.len(), 1);
    let r: BatchResult = results.get(0).unwrap();
    assert!(r.success);
    assert_eq!(r.index, 0);

    assert_eq!(client.get_withdrawable_balance(&creator, &token), 1_000i128);
}

#[test]
fn test_batch_tip_v2_multiple_recipients() {
    let (env, client, _admin, tipper, _creator, token) = setup();

    let creator1 = Address::generate(&env);
    let creator2 = Address::generate(&env);
    let creator3 = Address::generate(&env);

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    ops.push_back(TipOperation {
        creator: creator1.clone(),
        token: token.clone(),
        amount: 500i128,
    });
    ops.push_back(TipOperation {
        creator: creator2.clone(),
        token: token.clone(),
        amount: 750i128,
    });
    ops.push_back(TipOperation {
        creator: creator3.clone(),
        token: token.clone(),
        amount: 250i128,
    });

    let results = client.batch_tip_v2(&tipper, &ops);

    assert_eq!(results.len(), 3);
    for i in 0..3u32 {
        let r: BatchResult = results.get(i).unwrap();
        assert!(r.success);
        assert_eq!(r.index, i);
    }

    assert_eq!(client.get_withdrawable_balance(&creator1, &token), 500i128);
    assert_eq!(client.get_withdrawable_balance(&creator2, &token), 750i128);
    assert_eq!(client.get_withdrawable_balance(&creator3, &token), 250i128);
}

#[test]
fn test_batch_tip_v2_multiple_tokens() {
    let (env, client, _admin, tipper, creator, token1, token2) = setup_two_tokens();

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    ops.push_back(TipOperation {
        creator: creator.clone(),
        token: token1.clone(),
        amount: 1_000i128,
    });
    ops.push_back(TipOperation {
        creator: creator.clone(),
        token: token2.clone(),
        amount: 2_000i128,
    });

    let results = client.batch_tip_v2(&tipper, &ops);

    assert_eq!(results.len(), 2);
    assert_eq!(
        client.get_withdrawable_balance(&creator, &token1),
        1_000i128
    );
    assert_eq!(
        client.get_withdrawable_balance(&creator, &token2),
        2_000i128
    );
}

#[test]
fn test_batch_tip_v2_returns_correct_indices() {
    let (env, client, _admin, tipper, _creator, token) = setup();

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    for i in 0..5u32 {
        let c = Address::generate(&env);
        ops.push_back(TipOperation {
            creator: c,
            token: token.clone(),
            amount: (i as i128 + 1) * 100,
        });
    }

    let results = client.batch_tip_v2(&tipper, &ops);

    assert_eq!(results.len(), 5);
    for i in 0..5u32 {
        let r: BatchResult = results.get(i).unwrap();
        assert_eq!(r.index, i);
        assert!(r.success);
    }
}

#[test]
fn test_batch_tip_v2_empty_fails() {
    let (env, client, _admin, tipper, _creator, _token) = setup();

    let ops: Vec<TipOperation> = Vec::new(&env);
    let result = client.try_batch_tip_v2(&tipper, &ops);
    assert_eq!(result, Err(Ok(TipJarError::BatchSizeExceeded)));
}

#[test]
fn test_batch_tip_v2_exceeds_max_size_fails() {
    let (env, client, _admin, tipper, creator, token) = setup();

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    // Push 21 operations — one over the limit of 20.
    for _ in 0..21u32 {
        ops.push_back(TipOperation {
            creator: creator.clone(),
            token: token.clone(),
            amount: 100i128,
        });
    }

    let result = client.try_batch_tip_v2(&tipper, &ops);
    assert_eq!(result, Err(Ok(TipJarError::BatchSizeExceeded)));
}

#[test]
fn test_batch_tip_v2_invalid_amount_fails() {
    let (env, client, _admin, tipper, creator, token) = setup();

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    ops.push_back(TipOperation {
        creator: creator.clone(),
        token: token.clone(),
        amount: 0i128,
    });

    let result = client.try_batch_tip_v2(&tipper, &ops);
    assert_eq!(result, Err(Ok(TipJarError::InvalidAmount)));
}

#[test]
fn test_batch_tip_v2_unwhitelisted_token_fails() {
    let (env, client, _admin, tipper, creator, _token) = setup();

    // Register a token but don't whitelist it.
    let bad_token_admin = Address::generate(&env);
    let bad_token = env.register_stellar_asset_contract(bad_token_admin.clone());
    soroban_sdk::token::StellarAssetClient::new(&env, &bad_token).mint(&tipper, &1_000_000i128);

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    ops.push_back(TipOperation {
        creator: creator.clone(),
        token: bad_token.clone(),
        amount: 100i128,
    });

    let result = client.try_batch_tip_v2(&tipper, &ops);
    assert_eq!(result, Err(Ok(TipJarError::TokenNotWhitelisted)));
}

#[test]
fn test_batch_tip_v2_atomic_all_or_nothing() {
    let (env, client, _admin, tipper, creator, token) = setup();

    // Second operation has amount 0 — should fail atomically (no partial state).
    let mut ops: Vec<TipOperation> = Vec::new(&env);
    ops.push_back(TipOperation {
        creator: creator.clone(),
        token: token.clone(),
        amount: 500i128,
    });
    ops.push_back(TipOperation {
        creator: creator.clone(),
        token: token.clone(),
        amount: 0i128,
    });

    let result = client.try_batch_tip_v2(&tipper, &ops);
    assert_eq!(result, Err(Ok(TipJarError::InvalidAmount)));

    // No balance should have been credited.
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 0i128);
}

#[test]
fn test_batch_tip_v2_paused_fails() {
    let (env, client, admin, tipper, creator, token) = setup();

    let reason = soroban_sdk::String::from_str(&env, "maintenance");
    client.pause(&admin, &reason);

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    ops.push_back(TipOperation {
        creator: creator.clone(),
        token: token.clone(),
        amount: 100i128,
    });

    let result = client.try_batch_tip_v2(&tipper, &ops);
    assert_eq!(result, Err(Ok(TipJarError::ContractPaused)));
}

// ── batch_withdraw ────────────────────────────────────────────────────────────

/// Seeds `creator` with `amount` of `token` by tipping through the contract.
fn seed_creator_balance(
    env: &Env,
    client: &TipJarContractClient,
    tipper: &Address,
    creator: &Address,
    token: &Address,
    amount: i128,
) {
    let mut ops: Vec<TipOperation> = Vec::new(env);
    ops.push_back(TipOperation {
        creator: creator.clone(),
        token: token.clone(),
        amount,
    });
    client.batch_tip_v2(tipper, &ops);
}

#[test]
fn test_batch_withdraw_single_token() {
    let (env, client, _admin, tipper, creator, token) = setup();
    seed_creator_balance(&env, &client, &tipper, &creator, &token, 5_000i128);

    let mut ops: Vec<WithdrawOperation> = Vec::new(&env);
    ops.push_back(WithdrawOperation {
        token: token.clone(),
        amount: 3_000i128,
    });

    let results = client.batch_withdraw(&creator, &ops);

    assert_eq!(results.len(), 1);
    let r: BatchResult = results.get(0).unwrap();
    assert!(r.success);
    assert_eq!(r.index, 0);

    // Remaining balance should be 2_000.
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 2_000i128);
}

#[test]
fn test_batch_withdraw_multiple_tokens() {
    let (env, client, _admin, tipper, creator, token1, token2) = setup_two_tokens();
    seed_creator_balance(&env, &client, &tipper, &creator, &token1, 4_000i128);
    seed_creator_balance(&env, &client, &tipper, &creator, &token2, 6_000i128);

    let mut ops: Vec<WithdrawOperation> = Vec::new(&env);
    ops.push_back(WithdrawOperation {
        token: token1.clone(),
        amount: 4_000i128,
    });
    ops.push_back(WithdrawOperation {
        token: token2.clone(),
        amount: 6_000i128,
    });

    let results = client.batch_withdraw(&creator, &ops);

    assert_eq!(results.len(), 2);
    for i in 0..2u32 {
        let r: BatchResult = results.get(i).unwrap();
        assert!(r.success);
        assert_eq!(r.index, i);
    }

    assert_eq!(client.get_withdrawable_balance(&creator, &token1), 0i128);
    assert_eq!(client.get_withdrawable_balance(&creator, &token2), 0i128);
}

#[test]
fn test_batch_withdraw_partial_amount() {
    let (env, client, _admin, tipper, creator, token) = setup();
    seed_creator_balance(&env, &client, &tipper, &creator, &token, 10_000i128);

    let mut ops: Vec<WithdrawOperation> = Vec::new(&env);
    ops.push_back(WithdrawOperation {
        token: token.clone(),
        amount: 3_000i128,
    });
    ops.push_back(WithdrawOperation {
        token: token.clone(),
        amount: 2_000i128,
    });

    let results = client.batch_withdraw(&creator, &ops);

    assert_eq!(results.len(), 2);
    // 10_000 - 3_000 - 2_000 = 5_000 remaining.
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 5_000i128);
}

#[test]
fn test_batch_withdraw_empty_fails() {
    let (env, client, _admin, _tipper, creator, _token) = setup();

    let ops: Vec<WithdrawOperation> = Vec::new(&env);
    let result = client.try_batch_withdraw(&creator, &ops);
    assert_eq!(result, Err(Ok(TipJarError::BatchSizeExceeded)));
}

#[test]
fn test_batch_withdraw_exceeds_max_size_fails() {
    let (env, client, _admin, tipper, creator, token) = setup();
    seed_creator_balance(&env, &client, &tipper, &creator, &token, 1_000_000i128);

    let mut ops: Vec<WithdrawOperation> = Vec::new(&env);
    for _ in 0..21u32 {
        ops.push_back(WithdrawOperation {
            token: token.clone(),
            amount: 100i128,
        });
    }

    let result = client.try_batch_withdraw(&creator, &ops);
    assert_eq!(result, Err(Ok(TipJarError::BatchSizeExceeded)));
}

#[test]
fn test_batch_withdraw_invalid_amount_fails() {
    let (env, client, _admin, tipper, creator, token) = setup();
    seed_creator_balance(&env, &client, &tipper, &creator, &token, 1_000i128);

    let mut ops: Vec<WithdrawOperation> = Vec::new(&env);
    ops.push_back(WithdrawOperation {
        token: token.clone(),
        amount: 0i128,
    });

    let result = client.try_batch_withdraw(&creator, &ops);
    assert_eq!(result, Err(Ok(TipJarError::InvalidAmount)));
}

#[test]
fn test_batch_withdraw_insufficient_balance_fails() {
    let (env, client, _admin, tipper, creator, token) = setup();
    seed_creator_balance(&env, &client, &tipper, &creator, &token, 500i128);

    let mut ops: Vec<WithdrawOperation> = Vec::new(&env);
    ops.push_back(WithdrawOperation {
        token: token.clone(),
        amount: 1_000i128,
    });

    let result = client.try_batch_withdraw(&creator, &ops);
    assert_eq!(result, Err(Ok(TipJarError::InsufficientBalance)));
}

#[test]
fn test_batch_withdraw_atomic_all_or_nothing() {
    let (env, client, _admin, tipper, creator, token) = setup();
    // Seed only 500 — second op will exceed balance.
    seed_creator_balance(&env, &client, &tipper, &creator, &token, 500i128);

    let mut ops: Vec<WithdrawOperation> = Vec::new(&env);
    ops.push_back(WithdrawOperation {
        token: token.clone(),
        amount: 300i128,
    });
    ops.push_back(WithdrawOperation {
        token: token.clone(),
        amount: 300i128,
    }); // 300+300 > 500

    let result = client.try_batch_withdraw(&creator, &ops);
    assert_eq!(result, Err(Ok(TipJarError::InsufficientBalance)));

    // Balance must be unchanged — no partial withdrawal.
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 500i128);
}

#[test]
fn test_batch_withdraw_paused_fails() {
    let (env, client, admin, tipper, creator, token) = setup();
    seed_creator_balance(&env, &client, &tipper, &creator, &token, 1_000i128);

    let reason = soroban_sdk::String::from_str(&env, "maintenance");
    client.pause(&admin, &reason);

    let mut ops: Vec<WithdrawOperation> = Vec::new(&env);
    ops.push_back(WithdrawOperation {
        token: token.clone(),
        amount: 500i128,
    });

    let result = client.try_batch_withdraw(&creator, &ops);
    assert_eq!(result, Err(Ok(TipJarError::ContractPaused)));
}

#[test]
fn test_batch_withdraw_returns_correct_indices() {
    let (env, client, _admin, tipper, creator, token1, token2) = setup_two_tokens();
    seed_creator_balance(&env, &client, &tipper, &creator, &token1, 3_000i128);
    seed_creator_balance(&env, &client, &tipper, &creator, &token2, 3_000i128);

    let mut ops: Vec<WithdrawOperation> = Vec::new(&env);
    ops.push_back(WithdrawOperation {
        token: token1.clone(),
        amount: 1_000i128,
    });
    ops.push_back(WithdrawOperation {
        token: token2.clone(),
        amount: 1_000i128,
    });

    let results = client.batch_withdraw(&creator, &ops);

    assert_eq!(results.len(), 2);
    assert_eq!(results.get(0).unwrap().index, 0);
    assert_eq!(results.get(1).unwrap().index, 1);
}

// ── gas efficiency ────────────────────────────────────────────────────────────

#[test]
fn test_batch_tip_v2_max_batch_size_succeeds() {
    let (env, client, _admin, tipper, _creator, token) = setup();

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    for _ in 0..20u32 {
        let c = Address::generate(&env);
        ops.push_back(TipOperation {
            creator: c,
            token: token.clone(),
            amount: 100i128,
        });
    }

    let results = client.batch_tip_v2(&tipper, &ops);
    assert_eq!(results.len(), 20);
}

#[test]
fn test_batch_withdraw_max_batch_size_succeeds() {
    let (env, client, _admin, tipper, creator, token) = setup();
    // Seed enough for 20 withdrawals of 100 each.
    seed_creator_balance(&env, &client, &tipper, &creator, &token, 2_000i128);

    let mut ops: Vec<WithdrawOperation> = Vec::new(&env);
    for _ in 0..20u32 {
        ops.push_back(WithdrawOperation {
            token: token.clone(),
            amount: 100i128,
        });
    }

    let results = client.batch_withdraw(&creator, &ops);
    assert_eq!(results.len(), 20);
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 0i128);
}

// ── batch_tip_multi ───────────────────────────────────────────────────────────

#[test]
fn test_batch_tip_multi_all_valid() {
    let (env, client, _admin, tipper, _creator, token) = setup();

    let creator1 = Address::generate(&env);
    let creator2 = Address::generate(&env);

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    ops.push_back(TipOperation { creator: creator1.clone(), token: token.clone(), amount: 300i128 });
    ops.push_back(TipOperation { creator: creator2.clone(), token: token.clone(), amount: 700i128 });

    let results = client.batch_tip_multi(&tipper, &ops);

    assert_eq!(results.len(), 2);
    assert!(results.get(0).unwrap().success);
    assert!(results.get(1).unwrap().success);
    assert_eq!(client.get_withdrawable_balance(&creator1, &token), 300i128);
    assert_eq!(client.get_withdrawable_balance(&creator2, &token), 700i128);
}

#[test]
fn test_batch_tip_multi_partial_failure_invalid_amount() {
    let (env, client, _admin, tipper, creator, token) = setup();

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    ops.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 500i128 });
    ops.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 0i128 }); // invalid
    ops.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 200i128 });

    let results = client.batch_tip_multi(&tipper, &ops);

    assert_eq!(results.len(), 3);
    assert!(results.get(0).unwrap().success);
    assert!(!results.get(1).unwrap().success); // skipped
    assert!(results.get(2).unwrap().success);

    // Only 500 + 200 = 700 credited.
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 700i128);
}

#[test]
fn test_batch_tip_multi_partial_failure_unwhitelisted_token() {
    let (env, client, _admin, tipper, creator, token) = setup();

    let bad_token_admin = Address::generate(&env);
    let bad_token = env.register_stellar_asset_contract(bad_token_admin.clone());
    soroban_sdk::token::StellarAssetClient::new(&env, &bad_token).mint(&tipper, &1_000_000i128);

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    ops.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 400i128 });
    ops.push_back(TipOperation { creator: creator.clone(), token: bad_token.clone(), amount: 100i128 }); // unwhitelisted
    ops.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 100i128 });

    let results = client.batch_tip_multi(&tipper, &ops);

    assert_eq!(results.len(), 3);
    assert!(results.get(0).unwrap().success);
    assert!(!results.get(1).unwrap().success); // skipped
    assert!(results.get(2).unwrap().success);

    assert_eq!(client.get_withdrawable_balance(&creator, &token), 500i128);
    assert_eq!(client.get_withdrawable_balance(&creator, &bad_token), 0i128);
}

#[test]
fn test_batch_tip_multi_all_invalid_returns_all_failed() {
    let (env, client, _admin, tipper, creator, token) = setup();

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    ops.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 0i128 });
    ops.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: -1i128 });

    let results = client.batch_tip_multi(&tipper, &ops);

    assert_eq!(results.len(), 2);
    assert!(!results.get(0).unwrap().success);
    assert!(!results.get(1).unwrap().success);
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 0i128);
}

#[test]
fn test_batch_tip_multi_empty_fails() {
    let (env, client, _admin, tipper, _creator, _token) = setup();

    let ops: Vec<TipOperation> = Vec::new(&env);
    let result = client.try_batch_tip_multi(&tipper, &ops);
    assert_eq!(result, Err(Ok(TipJarError::BatchSizeExceeded)));
}

#[test]
fn test_batch_tip_multi_exceeds_limit_fails() {
    let (env, client, _admin, tipper, creator, token) = setup();

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    for _ in 0..21u32 {
        ops.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 10i128 });
    }

    let result = client.try_batch_tip_multi(&tipper, &ops);
    assert_eq!(result, Err(Ok(TipJarError::BatchSizeExceeded)));
}

#[test]
fn test_batch_tip_multi_max_batch_size_succeeds() {
    let (env, client, _admin, tipper, _creator, token) = setup();

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    for _ in 0..20u32 {
        let c = Address::generate(&env);
        ops.push_back(TipOperation { creator: c, token: token.clone(), amount: 10i128 });
    }

    let results = client.batch_tip_multi(&tipper, &ops);
    assert_eq!(results.len(), 20);
    for i in 0..20u32 {
        assert!(results.get(i).unwrap().success);
    }
}

#[test]
fn test_batch_tip_multi_paused_fails() {
    let (env, client, admin, tipper, creator, token) = setup();

    let reason = soroban_sdk::String::from_str(&env, "maintenance");
    client.pause(&admin, &reason);

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    ops.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 100i128 });

    let result = client.try_batch_tip_multi(&tipper, &ops);
    assert_eq!(result, Err(Ok(TipJarError::ContractPaused)));
}

// ── batch stats tracking ──────────────────────────────────────────────────────

#[test]
fn test_get_batch_stats_initial_zero() {
    let (_env, client, _admin, _tipper, _creator, _token) = setup();

    let stats = client.get_batch_stats();
    assert_eq!(stats.total_batches, 0);
    assert_eq!(stats.total_tips, 0);
    assert_eq!(stats.total_amount, 0);
    assert_eq!(stats.total_skipped, 0);
}

#[test]
fn test_batch_stats_updated_after_batch_tip_multi() {
    let (env, client, _admin, tipper, creator, token) = setup();

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    ops.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 500i128 });
    ops.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 0i128 }); // skipped

    client.batch_tip_multi(&tipper, &ops);

    let stats = client.get_batch_stats();
    assert_eq!(stats.total_batches, 1);
    assert_eq!(stats.total_tips, 1);
    assert_eq!(stats.total_amount, 500i128);
    assert_eq!(stats.total_skipped, 1);
}

#[test]
fn test_batch_stats_accumulate_across_multiple_batches() {
    let (env, client, _admin, tipper, creator, token) = setup();

    let mut ops1: Vec<TipOperation> = Vec::new(&env);
    ops1.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 300i128 });
    ops1.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 200i128 });
    client.batch_tip_multi(&tipper, &ops1);

    let mut ops2: Vec<TipOperation> = Vec::new(&env);
    ops2.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 100i128 });
    ops2.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 0i128 }); // skipped
    client.batch_tip_multi(&tipper, &ops2);

    let stats = client.get_batch_stats();
    assert_eq!(stats.total_batches, 2);
    assert_eq!(stats.total_tips, 3);       // 2 + 1
    assert_eq!(stats.total_amount, 600i128); // 500 + 100
    assert_eq!(stats.total_skipped, 1);
}

#[test]
fn test_batch_tip_multi_result_indices_correct() {
    let (env, client, _admin, tipper, creator, token) = setup();

    let mut ops: Vec<TipOperation> = Vec::new(&env);
    ops.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 100i128 });
    ops.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 0i128 });
    ops.push_back(TipOperation { creator: creator.clone(), token: token.clone(), amount: 200i128 });

    let results = client.batch_tip_multi(&tipper, &ops);

    assert_eq!(results.get(0).unwrap().index, 0);
    assert_eq!(results.get(1).unwrap().index, 1);
    assert_eq!(results.get(2).unwrap().index, 2);
}
