use super::common::*;
use tipjar::{TipJarError, Role};

pub fn test_insufficient_balance_scenarios() {
    let ctx = TestContext::new();
    
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    
    // Test tip with insufficient balance
    ctx.mint_tokens(&sender, &ctx.token_1, 100);
    
    let result = ctx.tipjar_client.try_tip(&sender, &creator, &ctx.token_1, &200);
    assert!(result.is_err()); // Should fail due to insufficient balance
    
    // Verify no state changes occurred
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 0);
    assert_total_tips_equals(&ctx, &creator, &ctx.token_1, 0);
    assert_balance_equals(&ctx, &sender, &ctx.token_1, 100); // Unchanged
    
    // Test batch with insufficient balance for some entries
    ctx.mint_tokens(&sender, &ctx.token_1, 150); // Total 250
    
    let batch = create_batch_tips(
        &ctx.env,
        &[creator.clone(), creator.clone(), creator.clone()],
        &ctx.token_1,
        &[100, 200, 50], // Total 350, but only 250 available
    );
    
    let results = ctx.tipjar_client.tip_batch(&sender, &batch);
    assert_eq!(results.len(), 3);
    assert_eq!(results.get(0).unwrap(), Ok(()));           // 100 - succeeds
    assert_eq!(results.get(1).unwrap(), Err(TipJarError::InsufficientBalance)); // 200 - fails
    assert_eq!(results.get(2).unwrap(), Ok(()));           // 50 - succeeds (150 remaining)
    
    // Verify only successful tips were processed
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 150); // 100 + 50
    assert_balance_equals(&ctx, &sender, &ctx.token_1, 100); // 250 - 150 = 100
    
    // Test locked tip with insufficient balance
    let result = ctx.tipjar_client.try_tip_locked(
        &sender, &creator, &ctx.token_1, &200, &(ctx.get_current_time() + 1000)
    );
    assert!(result.is_err());
    
    // Test matching program creation with insufficient sponsor balance
    ctx.mint_tokens(&sender, &ctx.token_1, 50); // Total 150, but need 500 for program
    let result = ctx.tipjar_client.try_create_matching_program(
        &sender, &creator, &ctx.token_1, &100, &500
    );
    assert!(result.is_err());
    
    // Test withdrawal with nothing to withdraw
    let empty_creator = ctx.create_creator();
    let result = ctx.tipjar_client.try_withdraw(&empty_creator, &ctx.token_1);
    assert_error_contains(result, TipJarError::NothingToWithdraw);
}

pub fn test_unauthorized_access() {
    let ctx = TestContext::new();
    
    let user = ctx.create_user();
    let creator = ctx.create_creator();
    let moderator = ctx.create_moderator();
    
    // Test non-admin cannot manage tokens
    let result = ctx.tipjar_client.try_add_token(&user, &ctx.token_2);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    let result = ctx.tipjar_client.try_remove_token(&user, &ctx.token_1);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    // Test non-admin cannot manage roles
    let result = ctx.tipjar_client.try_grant_role(&user, &creator, &Role::Admin);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    let result = ctx.tipjar_client.try_revoke_role(&user, &creator);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    // Test non-admin/moderator cannot pause
    let result = ctx.tipjar_client.try_pause(&user);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    let result = ctx.tipjar_client.try_pause(&creator);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    // Test moderator can pause but not manage tokens/roles
    ctx.tipjar_client.pause(&moderator); // Should succeed
    
    let result = ctx.tipjar_client.try_add_token(&moderator, &ctx.token_2);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    let result = ctx.tipjar_client.try_grant_role(&moderator, &user, &Role::Creator);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    ctx.tipjar_client.unpause(&ctx.admin);
    
    // Test non-creator cannot withdraw
    let result = ctx.tipjar_client.try_withdraw(&user, &ctx.token_1);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    // Test non-creator cannot withdraw locked tips
    ctx.mint_tokens(&user, &ctx.token_1, 1000);
    let tip_id = ctx.tipjar_client.tip_locked(
        &user, &creator, &ctx.token_1, &500, &(ctx.get_current_time() + 1000)
    );
    
    ctx.advance_time(1001);
    
    let result = ctx.tipjar_client.try_withdraw_locked(&user, &tip_id);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    // Test non-admin cannot upgrade
    let wasm_hash = ctx.env.deployer().upload_contract_wasm(tipjar::TipJarContract::wasm());
    let result = ctx.tipjar_client.try_upgrade(&user, &wasm_hash);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    // Test non-admin cannot set DEX/NFT contracts
    let mock_address = ctx.create_user();
    let result = ctx.tipjar_client.try_set_dex(&user, &mock_address);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    let result = ctx.tipjar_client.try_set_nft_contract(&user, &mock_address);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    // Test unauthorized matching program operations
    let sponsor = ctx.create_user();
    ctx.mint_tokens(&sponsor, &ctx.token_1, 1000);
    
    let program_id = ctx.tipjar_client.create_matching_program(
        &sponsor, &creator, &ctx.token_1, &100, &500
    );
    
    // Non-sponsor cannot cancel program
    let result = ctx.tipjar_client.try_cancel_matching_program(&user, &program_id);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    // Test role revocation of non-existent role
    let result = ctx.tipjar_client.try_revoke_role(&ctx.admin, &user);
    assert_error_contains(result, TipJarError::RoleNotFound);
}

pub fn test_invalid_token_operations() {
    let ctx = TestContext::new();
    
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    
    ctx.mint_tokens(&sender, &ctx.token_3, 1000); // token_3 is not whitelisted
    
    // Test tip with non-whitelisted token
    let result = ctx.tipjar_client.try_tip(&sender, &creator, &ctx.token_3, &100);
    assert_error_contains(result, TipJarError::TokenNotWhitelisted);
    
    // Test tip_with_message with non-whitelisted token
    let message = create_tip_message(&ctx.env, "test");
    let metadata = create_metadata(&ctx.env, &[]);
    let result = ctx.tipjar_client.try_tip_with_message(
        &sender, &creator, &ctx.token_3, &100, &message, &metadata
    );
    assert_error_contains(result, TipJarError::TokenNotWhitelisted);
    
    // Test batch with non-whitelisted token
    let batch = create_batch_tips(&ctx.env, &[creator.clone()], &ctx.token_3, &[100]);
    let results = ctx.tipjar_client.tip_batch(&sender, &batch);
    assert_eq!(results.get(0).unwrap(), Err(TipJarError::TokenNotWhitelisted));
    
    // Test locked tip with non-whitelisted token
    let result = ctx.tipjar_client.try_tip_locked(
        &sender, &creator, &ctx.token_3, &100, &(ctx.get_current_time() + 1000)
    );
    assert_error_contains(result, TipJarError::TokenNotWhitelisted);
    
    // Test matching program with non-whitelisted token
    let result = ctx.tipjar_client.try_create_matching_program(
        &sender, &creator, &ctx.token_3, &100, &500
    );
    assert_error_contains(result, TipJarError::TokenNotWhitelisted);
    
    // Test tip_with_match with non-whitelisted token
    ctx.mint_tokens(&sender, &ctx.token_1, 1000);
    let program_id = ctx.tipjar_client.create_matching_program(
        &sender, &creator, &ctx.token_1, &100, &500
    );
    
    let result = ctx.tipjar_client.try_tip_with_match(
        &sender, &creator, &ctx.token_3, &100
    );
    assert_error_contains(result, TipJarError::TokenNotWhitelisted);
    
    // Test cross-contract operations with non-whitelisted tokens
    let mock_dex = ctx.create_user();
    ctx.tipjar_client.set_dex(&ctx.admin, &mock_dex);
    
    let result = ctx.tipjar_client.try_tip_with_swap(
        &sender, &creator, &ctx.token_1, &ctx.token_3, &100, &90 // tip_token not whitelisted
    );
    assert_error_contains(result, TipJarError::TokenNotWhitelisted);
    
    // Test removing token from whitelist affects existing operations
    ctx.tipjar_client.remove_token(&ctx.admin, &ctx.token_1);
    
    let result = ctx.tipjar_client.try_tip(&sender, &creator, &ctx.token_1, &100);
    assert_error_contains(result, TipJarError::TokenNotWhitelisted);
    
    // But existing balances should still be withdrawable
    ctx.tipjar_client.add_token(&ctx.admin, &ctx.token_1); // Re-add for withdrawal
    // (In a real scenario, you might want withdrawal to work even for delisted tokens)
}

pub fn test_time_based_failures() {
    let ctx = TestContext::new();
    
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    
    ctx.mint_tokens(&sender, &ctx.token_1, 2000);
    
    let current_time = ctx.get_current_time();
    
    // Test locked tip with invalid unlock times
    let result = ctx.tipjar_client.try_tip_locked(
        &sender, &creator, &ctx.token_1, &100, &current_time // Current time, not future
    );
    assert_error_contains(result, TipJarError::InvalidUnlockTime);
    
    let result = ctx.tipjar_client.try_tip_locked(
        &sender, &creator, &ctx.token_1, &100, &(current_time - 1000) // Past time
    );
    assert_error_contains(result, TipJarError::InvalidUnlockTime);
    
    // Test premature withdrawal of locked tip
    let unlock_time = current_time + 2000;
    let tip_id = ctx.tipjar_client.tip_locked(&sender, &creator, &ctx.token_1, &500, &unlock_time);
    
    let result = ctx.tipjar_client.try_withdraw_locked(&creator, &tip_id);
    assert_error_contains(result, TipJarError::TipStillLocked);
    
    // Advance time partially (still locked)
    ctx.advance_time(1000);
    let result = ctx.tipjar_client.try_withdraw_locked(&creator, &tip_id);
    assert_error_contains(result, TipJarError::TipStillLocked);
    
    // Advance to exact unlock time (should still be locked)
    ctx.advance_time(1000); // Total 2000, equals unlock_time
    let result = ctx.tipjar_client.try_withdraw_locked(&creator, &tip_id);
    assert_error_contains(result, TipJarError::TipStillLocked);
    
    // Advance past unlock time (should work)
    ctx.advance_time(1); // Total 2001, past unlock_time
    ctx.tipjar_client.withdraw_locked(&creator, &tip_id); // Should succeed
    
    // Test multiple locked tips with different unlock times
    let tip_id_1 = ctx.tipjar_client.tip_locked(&sender, &creator, &ctx.token_1, &200, &(current_time + 3000));
    let tip_id_2 = ctx.tipjar_client.tip_locked(&sender, &creator, &ctx.token_1, &300, &(current_time + 4000));
    
    // First tip should still be locked
    let result = ctx.tipjar_client.try_withdraw_locked(&creator, &tip_id_1);
    assert_error_contains(result, TipJarError::TipStillLocked);
    
    // Advance to unlock first tip
    ctx.advance_time(1000); // Total 3001 from original current_time
    ctx.tipjar_client.withdraw_locked(&creator, &tip_id_1); // Should succeed
    
    // Second tip should still be locked
    let result = ctx.tipjar_client.try_withdraw_locked(&creator, &tip_id_2);
    assert_error_contains(result, TipJarError::TipStillLocked);
    
    // Test edge case: unlock exactly at timestamp boundary
    let precise_unlock_time = ctx.get_current_time() + 1;
    let tip_id_3 = ctx.tipjar_client.tip_locked(&sender, &creator, &ctx.token_1, &100, &precise_unlock_time);
    
    ctx.advance_time(1); // Advance to exact unlock time
    let result = ctx.tipjar_client.try_withdraw_locked(&creator, &tip_id_3);
    assert_error_contains(result, TipJarError::TipStillLocked); // Should still be locked at exact time
    
    ctx.advance_time(1); // One second past
    ctx.tipjar_client.withdraw_locked(&creator, &tip_id_3); // Should succeed
}

pub fn test_contract_pause_scenarios() {
    let ctx = TestContext::new();
    
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    let moderator = ctx.create_moderator();
    
    ctx.mint_tokens(&sender, &ctx.token_1, 2000);
    
    // Establish some state before pausing
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &300);
    
    let unlock_time = ctx.get_current_time() + 1000;
    let tip_id = ctx.tipjar_client.tip_locked(&sender, &creator, &ctx.token_1, &200, &unlock_time);
    
    let sponsor = ctx.create_user();
    ctx.mint_tokens(&sponsor, &ctx.token_1, 1000);
    let program_id = ctx.tipjar_client.create_matching_program(&sponsor, &creator, &ctx.token_1, &100, &500);
    
    // Pause the contract
    ctx.tipjar_client.pause(&ctx.admin);
    
    // Test all state-changing operations are blocked
    let result = ctx.tipjar_client.try_tip(&sender, &creator, &ctx.token_1, &100);
    assert!(result.is_err());
    
    let message = create_tip_message(&ctx.env, "test");
    let metadata = create_metadata(&ctx.env, &[]);
    let result = ctx.tipjar_client.try_tip_with_message(&sender, &creator, &ctx.token_1, &100, &message, &metadata);
    assert!(result.is_err());
    
    let batch = create_batch_tips(&ctx.env, &[creator.clone()], &ctx.token_1, &[100]);
    let result = ctx.tipjar_client.try_tip_batch(&sender, &batch);
    assert!(result.is_err());
    
    let result = ctx.tipjar_client.try_withdraw(&creator, &ctx.token_1);
    assert!(result.is_err());
    
    let result = ctx.tipjar_client.try_tip_locked(&sender, &creator, &ctx.token_1, &100, &(unlock_time + 1000));
    assert!(result.is_err());
    
    ctx.advance_time(1001);
    let result = ctx.tipjar_client.try_withdraw_locked(&creator, &tip_id);
    assert!(result.is_err());
    
    let result = ctx.tipjar_client.try_tip_with_match(&sender, &creator, &ctx.token_1, &100);
    assert!(result.is_err());
    
    let result = ctx.tipjar_client.try_create_matching_program(&sponsor, &creator, &ctx.token_1, &100, &200);
    assert!(result.is_err());
    
    let result = ctx.tipjar_client.try_cancel_matching_program(&sponsor, &program_id);
    assert!(result.is_err());
    
    // Test query operations still work
    let balance = ctx.tipjar_client.get_withdrawable_balance(&creator, &ctx.token_1);
    assert_eq!(balance, 300);
    
    let total = ctx.tipjar_client.get_total_tips(&creator, &ctx.token_1);
    assert_eq!(total, 300);
    
    let locked_tip = ctx.tipjar_client.get_locked_tip(&creator, &tip_id);
    assert_eq!(locked_tip.amount, 200);
    
    let program = ctx.tipjar_client.get_matching_program(&program_id);
    assert_eq!(program.max_match_amount, 500);
    
    let messages = ctx.tipjar_client.get_messages(&creator);
    // Should return existing messages
    
    let leaderboard = ctx.tipjar_client.get_top_tippers(&tipjar::TimePeriod::AllTime, &0, &10);
    // Should return existing leaderboard
    
    // Test moderator can unpause
    ctx.tipjar_client.unpause(&moderator);
    
    // Test operations work after unpause
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &150);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 450);
    
    ctx.tipjar_client.withdraw(&creator, &ctx.token_1);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 0);
    
    ctx.tipjar_client.withdraw_locked(&creator, &tip_id);
    
    let matched = ctx.tipjar_client.tip_with_match(&sender, &creator, &ctx.token_1, &100);
    assert_eq!(matched, 100);
}