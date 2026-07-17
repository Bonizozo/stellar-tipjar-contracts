use super::common::*;
use tipjar::{BatchTip, TipJarError, TimePeriod};

pub fn test_batch_operations() {
    let ctx = TestContext::new();
    
    let sender = ctx.create_user();
    let creator1 = ctx.create_creator();
    let creator2 = ctx.create_creator();
    let creator3 = ctx.create_creator();
    
    ctx.mint_tokens(&sender, &ctx.token_1, 5000);
    
    // Test successful batch operation
    let tips = create_batch_tips(
        &ctx.env,
        &[creator1.clone(), creator2.clone(), creator3.clone()],
        &ctx.token_1,
        &[100, 200, 300],
    );
    
    let results = ctx.tipjar_client.tip_batch(&sender, &tips);
    assert_eq!(results.len(), 3);
    for i in 0..3 {
        assert_eq!(results.get(i).unwrap(), Ok(()));
    }
    
    assert_withdrawable_balance_equals(&ctx, &creator1, &ctx.token_1, 100);
    assert_withdrawable_balance_equals(&ctx, &creator2, &ctx.token_1, 200);
    assert_withdrawable_balance_equals(&ctx, &creator3, &ctx.token_1, 300);
    
    // Test mixed success/failure batch
    let mixed_tips = create_batch_tips(
        &ctx.env,
        &[creator1.clone(), creator2.clone(), creator3.clone()],
        &ctx.token_1,
        &[150, 0, 250], // Middle tip has invalid amount
    );
    
    let mixed_results = ctx.tipjar_client.tip_batch(&sender, &mixed_tips);
    assert_eq!(mixed_results.len(), 3);
    assert_eq!(mixed_results.get(0).unwrap(), Ok(()));
    assert_eq!(mixed_results.get(1).unwrap(), Err(TipJarError::InvalidAmount));
    assert_eq!(mixed_results.get(2).unwrap(), Ok(()));
    
    // Verify only successful tips were processed
    assert_withdrawable_balance_equals(&ctx, &creator1, &ctx.token_1, 250); // 100 + 150
    assert_withdrawable_balance_equals(&ctx, &creator2, &ctx.token_1, 200); // Unchanged
    assert_withdrawable_balance_equals(&ctx, &creator3, &ctx.token_1, 550); // 300 + 250
    
    // Test batch with non-whitelisted token
    let invalid_tips = create_batch_tips(
        &ctx.env,
        &[creator1.clone()],
        &ctx.token_3, // Not whitelisted
        &[100],
    );
    
    let invalid_results = ctx.tipjar_client.tip_batch(&sender, &invalid_tips);
    assert_eq!(invalid_results.get(0).unwrap(), Err(TipJarError::TokenNotWhitelisted));
    
    // Test batch size limit (51 entries should fail)
    let large_batch = create_batch_tips(
        &ctx.env,
        &vec![creator1.clone(); 51],
        &ctx.token_1,
        &vec![1; 51],
    );
    
    let result = ctx.tipjar_client.try_tip_batch(&sender, &large_batch);
    assert_error_contains(result, TipJarError::BatchTooLarge);
    
    // Test exactly 50 entries (should succeed)
    ctx.mint_tokens(&sender, &ctx.token_1, 5000);
    let max_batch = create_batch_tips(
        &ctx.env,
        &vec![creator1.clone(); 50],
        &ctx.token_1,
        &vec![10; 50],
    );
    
    let max_results = ctx.tipjar_client.tip_batch(&sender, &max_batch);
    assert_eq!(max_results.len(), 50);
    for i in 0..50 {
        assert_eq!(max_results.get(i).unwrap(), Ok(()));
    }
}

pub fn test_locked_tips_workflows() {
    let ctx = TestContext::new();
    
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    
    ctx.mint_tokens(&sender, &ctx.token_1, 2000);
    
    let current_time = ctx.get_current_time();
    let unlock_time_1 = current_time + 1000;
    let unlock_time_2 = current_time + 2000;
    
    // Test creating locked tips
    let tip_id_1 = ctx.tipjar_client.tip_locked(&sender, &creator, &ctx.token_1, &500, &unlock_time_1);
    let tip_id_2 = ctx.tipjar_client.tip_locked(&sender, &creator, &ctx.token_1, &300, &unlock_time_2);
    
    assert_eq!(tip_id_1, 0);
    assert_eq!(tip_id_2, 1);
    
    // Verify locked tips are stored correctly
    let locked_tip_1 = ctx.tipjar_client.get_locked_tip(&creator, &tip_id_1);
    assert_eq!(locked_tip_1.amount, 500);
    assert_eq!(locked_tip_1.unlock_timestamp, unlock_time_1);
    assert_eq!(locked_tip_1.sender, sender);
    
    // Test early withdrawal fails
    let result = ctx.tipjar_client.try_withdraw_locked(&creator, &tip_id_1);
    assert_error_contains(result, TipJarError::TipStillLocked);
    
    // Test withdrawal after unlock time
    ctx.advance_time(1001);
    
    let creator_balance_before = ctx.get_token_balance(&creator, &ctx.token_1);
    ctx.tipjar_client.withdraw_locked(&creator, &tip_id_1);
    
    assert_balance_equals(&ctx, &creator, &ctx.token_1, creator_balance_before + 500);
    
    // Verify locked tip is removed after withdrawal
    let result = ctx.tipjar_client.try_get_locked_tip(&creator, &tip_id_1);
    assert_error_contains(result, TipJarError::LockedTipNotFound);
    
    // Test second locked tip still exists
    let locked_tip_2 = ctx.tipjar_client.get_locked_tip(&creator, &tip_id_2);
    assert_eq!(locked_tip_2.amount, 300);
    
    // Test withdrawal of second tip after its unlock time
    ctx.advance_time(1000); // Total 2001 seconds from start
    ctx.tipjar_client.withdraw_locked(&creator, &tip_id_2);
    
    // Test invalid unlock time (past or current)
    let result = ctx.tipjar_client.try_tip_locked(&sender, &creator, &ctx.token_1, &100, &current_time);
    assert_error_contains(result, TipJarError::InvalidUnlockTime);
    
    // Test locked tips with non-whitelisted token
    let result = ctx.tipjar_client.try_tip_locked(&sender, &creator, &ctx.token_3, &100, &(current_time + 3000));
    assert_error_contains(result, TipJarError::TokenNotWhitelisted);
}

pub fn test_matching_programs() {
    let ctx = TestContext::new();
    
    let sponsor = ctx.create_user();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    
    ctx.mint_tokens(&sponsor, &ctx.token_1, 2000);
    ctx.mint_tokens(&sender, &ctx.token_1, 1000);
    
    // Test creating matching program (1:1 ratio)
    let program_id = ctx.tipjar_client.create_matching_program(
        &sponsor, &creator, &ctx.token_1, &100, &1000 // 1:1 ratio, 1000 budget
    );
    
    // Verify sponsor's tokens were deposited
    assert_balance_equals(&ctx, &sponsor, &ctx.token_1, 1000);
    
    // Test tip with matching
    let matched_amount = ctx.tipjar_client.tip_with_match(&sender, &creator, &ctx.token_1, &200);
    assert_eq!(matched_amount, 200); // 1:1 match
    
    // Verify creator received tip + match
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 400); // 200 + 200
    assert_total_tips_equals(&ctx, &creator, &ctx.token_1, 400);
    
    // Verify program state updated
    let program = ctx.tipjar_client.get_matching_program(&program_id);
    assert_eq!(program.current_matched, 200);
    assert!(program.active);
    
    // Test 2:1 matching ratio
    let program_id_2 = ctx.tipjar_client.create_matching_program(
        &sponsor, &creator, &ctx.token_1, &200, &800 // 2:1 ratio, 800 budget
    );
    
    let matched_amount_2 = ctx.tipjar_client.tip_with_match(&sender, &creator, &ctx.token_1, &100);
    assert_eq!(matched_amount_2, 200); // 2:1 match
    
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 700); // Previous 400 + 100 + 200
    
    // Test budget exhaustion
    ctx.tipjar_client.tip_with_match(&sender, &creator, &ctx.token_1, &400); // Uses remaining 600 budget from first program
    
    let program_after_exhaustion = ctx.tipjar_client.get_matching_program(&program_id);
    assert!(!program_after_exhaustion.active); // Should be deactivated
    
    // Test program cancellation with refund
    let sponsor_balance_before = ctx.get_token_balance(&sponsor, &ctx.token_1);
    ctx.tipjar_client.cancel_matching_program(&sponsor, &program_id_2);
    
    let program_cancelled = ctx.tipjar_client.get_matching_program(&program_id_2);
    assert!(!program_cancelled.active);
    
    // Sponsor should get unspent budget back (800 - 200 = 600)
    assert_balance_equals(&ctx, &sponsor, &ctx.token_1, sponsor_balance_before + 600);
    
    // Test unauthorized cancellation
    let other_user = ctx.create_user();
    let result = ctx.tipjar_client.try_cancel_matching_program(&other_user, &program_id);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    // Test invalid match ratio
    let result = ctx.tipjar_client.try_create_matching_program(
        &sponsor, &creator, &ctx.token_1, &0, &100 // Invalid ratio
    );
    assert_error_contains(result, TipJarError::InvalidMatchRatio);
}

pub fn test_leaderboard_functionality() {
    let ctx = TestContext::new();
    
    let tipper1 = ctx.create_user();
    let tipper2 = ctx.create_user();
    let tipper3 = ctx.create_user();
    let creator1 = ctx.create_creator();
    let creator2 = ctx.create_creator();
    
    ctx.mint_tokens(&tipper1, &ctx.token_1, 2000);
    ctx.mint_tokens(&tipper2, &ctx.token_1, 2000);
    ctx.mint_tokens(&tipper3, &ctx.token_1, 2000);
    
    // Create tips with different patterns for leaderboard testing
    // Tipper1: 1 tip of 500 (total: 500, count: 1)
    ctx.tipjar_client.tip(&tipper1, &creator1, &ctx.token_1, &500);
    
    // Tipper2: 2 tips of 250 each (total: 500, count: 2) - should rank higher due to count
    ctx.tipjar_client.tip(&tipper2, &creator1, &ctx.token_1, &250);
    ctx.tipjar_client.tip(&tipper2, &creator2, &ctx.token_1, &250);
    
    // Tipper3: 1 tip of 600 (total: 600, count: 1) - should rank highest
    ctx.tipjar_client.tip(&tipper3, &creator2, &ctx.token_1, &600);
    
    // Test AllTime leaderboard for tippers
    let top_tippers = ctx.tipjar_client.get_top_tippers(&TimePeriod::AllTime, &0, &10);
    assert_eq!(top_tippers.len(), 3);
    
    // Should be ordered by total_amount desc, then tip_count desc
    let first = top_tippers.get(0).unwrap();
    let second = top_tippers.get(1).unwrap();
    let third = top_tippers.get(2).unwrap();
    
    assert_eq!(first.address, tipper3); // 600 total
    assert_eq!(first.total_amount, 600);
    assert_eq!(first.tip_count, 1);
    
    assert_eq!(second.address, tipper2); // 500 total, 2 count
    assert_eq!(second.total_amount, 500);
    assert_eq!(second.tip_count, 2);
    
    assert_eq!(third.address, tipper1); // 500 total, 1 count
    assert_eq!(third.total_amount, 500);
    assert_eq!(third.tip_count, 1);
    
    // Test AllTime leaderboard for creators
    let top_creators = ctx.tipjar_client.get_top_creators(&TimePeriod::AllTime, &0, &10);
    assert_eq!(top_creators.len(), 2);
    
    let top_creator = top_creators.get(0).unwrap();
    assert_eq!(top_creator.address, creator1); // 750 total (500 + 250)
    assert_eq!(top_creator.total_amount, 750);
    assert_eq!(top_creator.tip_count, 2);
    
    let second_creator = top_creators.get(1).unwrap();
    assert_eq!(second_creator.address, creator2); // 850 total (250 + 600)
    assert_eq!(second_creator.total_amount, 850);
    assert_eq!(second_creator.tip_count, 2);
    
    // Test pagination
    let paginated = ctx.tipjar_client.get_top_tippers(&TimePeriod::AllTime, &1, &2);
    assert_eq!(paginated.len(), 2);
    assert_eq!(paginated.get(0).unwrap().address, tipper2);
    assert_eq!(paginated.get(1).unwrap().address, tipper1);
    
    // Test empty results for high offset
    let empty = ctx.tipjar_client.get_top_tippers(&TimePeriod::AllTime, &10, &5);
    assert_eq!(empty.len(), 0);
    
    // Test zero limit returns empty
    let zero_limit = ctx.tipjar_client.get_top_tippers(&TimePeriod::AllTime, &0, &0);
    assert_eq!(zero_limit.len(), 0);
    
    // Test leaderboard queries work while paused
    ctx.tipjar_client.pause(&ctx.admin);
    let paused_results = ctx.tipjar_client.get_top_tippers(&TimePeriod::AllTime, &0, &5);
    assert_eq!(paused_results.len(), 3);
    ctx.tipjar_client.unpause(&ctx.admin);
}

pub fn test_cross_contract_integrations() {
    let ctx = TestContext::new();
    
    // Note: This is a simplified test since we don't have actual DEX/NFT contracts
    // In a real integration test, you would deploy mock contracts
    
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    
    ctx.mint_tokens(&sender, &ctx.token_1, 1000);
    
    // Test DEX integration error handling (no DEX configured)
    let result = ctx.tipjar_client.try_tip_with_swap(
        &sender, &creator, &ctx.token_1, &ctx.token_2, &100, &90
    );
    assert_error_contains(result, TipJarError::DexNotConfigured);
    
    // Test NFT integration error handling (no NFT contract configured)
    let nft_metadata = create_tip_message(&ctx.env, "Special tip NFT");
    let result = ctx.tipjar_client.try_tip_with_nft_reward(
        &sender, &creator, &ctx.token_1, &500, &400, &nft_metadata
    );
    assert_error_contains(result, TipJarError::NftNotConfigured);
    
    // Test setting DEX and NFT contracts (admin only)
    let mock_dex = ctx.create_user(); // Mock address
    let mock_nft = ctx.create_user(); // Mock address
    
    ctx.tipjar_client.set_dex(&ctx.admin, &mock_dex);
    ctx.tipjar_client.set_nft_contract(&ctx.admin, &mock_nft);
    
    // Test non-admin cannot set contracts
    let user = ctx.create_user();
    let result = ctx.tipjar_client.try_set_dex(&user, &mock_dex);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    let result = ctx.tipjar_client.try_set_nft_contract(&user, &mock_nft);
    assert_error_contains(result, TipJarError::Unauthorized);
}