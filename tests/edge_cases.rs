use super::common::*;
use tipjar::{TipJarError, TimePeriod, TipHistoryQuery};

pub fn test_boundary_conditions() {
    let ctx = TestContext::new();
    
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    
    // Test maximum values
    ctx.mint_tokens(&sender, &ctx.token_1, i128::MAX);
    
    // Test large tip amount (should work)
    let large_amount = 1_000_000_000_000i128; // 1 trillion
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &large_amount);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, large_amount);
    
    // Test minimum valid values
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &1);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, large_amount + 1);
    
    // Test zero and negative amounts (should fail)
    let result = ctx.tipjar_client.try_tip(&sender, &creator, &ctx.token_1, &0);
    assert_error_contains(result, TipJarError::InvalidAmount);
    
    let result = ctx.tipjar_client.try_tip(&sender, &creator, &ctx.token_1, &-100);
    assert_error_contains(result, TipJarError::InvalidAmount);
    
    // Test empty message (should work)
    let empty_message = create_tip_message(&ctx.env, "");
    let empty_metadata = create_metadata(&ctx.env, &[]);
    ctx.tipjar_client.tip_with_message(&sender, &creator, &ctx.token_1, &100, &empty_message, &empty_metadata);
    
    // Test maximum message length (280 characters)
    let max_message = create_tip_message(&ctx.env, &"a".repeat(280));
    ctx.tipjar_client.tip_with_message(&sender, &creator, &ctx.token_1, &100, &max_message, &empty_metadata);
    
    // Test message too long (should fail)
    let long_message = create_tip_message(&ctx.env, &"a".repeat(281));
    let result = ctx.tipjar_client.try_tip_with_message(&sender, &creator, &ctx.token_1, &100, &long_message, &empty_metadata);
    assert_error_contains(result, TipJarError::MessageTooLong);
    
    // Test empty collections
    let empty_tips = create_batch_tips(&ctx.env, &[], &ctx.token_1, &[]);
    let results = ctx.tipjar_client.tip_batch(&sender, &empty_tips);
    assert_eq!(results.len(), 0);
    
    // Test leaderboard with no participants
    let empty_leaderboard = ctx.tipjar_client.get_top_tippers(&TimePeriod::Weekly, &0, &10);
    assert_eq!(empty_leaderboard.len(), 0);
    
    // Test tip history with no messages
    let new_creator = ctx.create_creator();
    let empty_messages = ctx.tipjar_client.get_messages(&new_creator);
    assert_eq!(empty_messages.len(), 0);
    
    let empty_history = ctx.tipjar_client.get_creator_tips(&new_creator, &10);
    assert_eq!(empty_history.len(), 0);
    
    // Test time boundary conditions
    let current_time = ctx.get_current_time();
    
    // Test locked tip with minimum future time (current + 1)
    ctx.tipjar_client.tip_locked(&sender, &creator, &ctx.token_1, &100, &(current_time + 1));
    
    // Test locked tip with maximum future time
    let max_future_time = u64::MAX;
    ctx.tipjar_client.tip_locked(&sender, &creator, &ctx.token_1, &100, &max_future_time);
}

pub fn test_concurrent_operations() {
    let ctx = TestContext::new();
    
    let sender1 = ctx.create_user();
    let sender2 = ctx.create_user();
    let creator = ctx.create_creator();
    
    ctx.mint_tokens(&sender1, &ctx.token_1, 2000);
    ctx.mint_tokens(&sender2, &ctx.token_1, 2000);
    
    // Simulate concurrent tips to same creator
    ctx.tipjar_client.tip(&sender1, &creator, &ctx.token_1, &300);
    ctx.tipjar_client.tip(&sender2, &creator, &ctx.token_1, &400);
    
    // Verify both tips are properly accumulated
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 700);
    assert_total_tips_equals(&ctx, &creator, &ctx.token_1, 700);
    
    // Test concurrent batch operations
    let batch1 = create_batch_tips(&ctx.env, &[creator.clone()], &ctx.token_1, &[200]);
    let batch2 = create_batch_tips(&ctx.env, &[creator.clone()], &ctx.token_1, &[300]);
    
    ctx.tipjar_client.tip_batch(&sender1, &batch1);
    ctx.tipjar_client.tip_batch(&sender2, &batch2);
    
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 1200); // 700 + 200 + 300
    
    // Test concurrent locked tips
    let current_time = ctx.get_current_time();
    let unlock_time = current_time + 1000;
    
    let tip_id_1 = ctx.tipjar_client.tip_locked(&sender1, &creator, &ctx.token_1, &150, &unlock_time);
    let tip_id_2 = ctx.tipjar_client.tip_locked(&sender2, &creator, &ctx.token_1, &250, &unlock_time);
    
    assert_eq!(tip_id_1, 0);
    assert_eq!(tip_id_2, 1);
    
    // Advance time and withdraw both
    ctx.advance_time(1001);
    
    let creator_balance_before = ctx.get_token_balance(&creator, &ctx.token_1);
    ctx.tipjar_client.withdraw_locked(&creator, &tip_id_1);
    ctx.tipjar_client.withdraw_locked(&creator, &tip_id_2);
    
    assert_balance_equals(&ctx, &creator, &ctx.token_1, creator_balance_before + 400);
    
    // Test concurrent role operations
    let user1 = ctx.create_user();
    let user2 = ctx.create_user();
    
    ctx.tipjar_client.grant_role(&ctx.admin, &user1, &tipjar::Role::Creator);
    ctx.tipjar_client.grant_role(&ctx.admin, &user2, &tipjar::Role::Moderator);
    
    assert!(ctx.tipjar_client.has_role(&user1, &tipjar::Role::Creator));
    assert!(ctx.tipjar_client.has_role(&user2, &tipjar::Role::Moderator));
    
    // Test concurrent matching programs
    let sponsor1 = ctx.create_user();
    let sponsor2 = ctx.create_user();
    
    ctx.mint_tokens(&sponsor1, &ctx.token_1, 1000);
    ctx.mint_tokens(&sponsor2, &ctx.token_1, 1000);
    
    let program_id_1 = ctx.tipjar_client.create_matching_program(&sponsor1, &creator, &ctx.token_1, &100, &500);
    let program_id_2 = ctx.tipjar_client.create_matching_program(&sponsor2, &creator, &ctx.token_1, &150, &600);
    
    // First program should be used first
    let matched = ctx.tipjar_client.tip_with_match(&sender1, &creator, &ctx.token_1, &200);
    assert_eq!(matched, 200); // From first program (1:1 ratio)
    
    let program_1 = ctx.tipjar_client.get_matching_program(&program_id_1);
    assert_eq!(program_1.current_matched, 200);
    
    let program_2 = ctx.tipjar_client.get_matching_program(&program_id_2);
    assert_eq!(program_2.current_matched, 0); // Unused
}

pub fn test_malformed_inputs() {
    let ctx = TestContext::new();
    
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    
    ctx.mint_tokens(&sender, &ctx.token_1, 1000);
    
    // Test tip history with malformed query
    let malformed_query = TipHistoryQuery {
        creator: Some(creator.clone()),
        sender: None,
        min_amount: Some(-100), // Negative minimum
        max_amount: Some(50),   // Max less than min
        start_time: Some(u64::MAX),
        end_time: Some(0),      // End before start
        limit: 0,               // Zero limit
        offset: u64::MAX as u32, // Very high offset
    };
    
    // Should handle gracefully and return empty results
    let results = ctx.tipjar_client.get_tip_history(&malformed_query);
    assert_eq!(results.len(), 0);
    
    // Test with extremely large limit (should be capped)
    let large_limit_query = TipHistoryQuery {
        creator: Some(creator.clone()),
        sender: None,
        min_amount: None,
        max_amount: None,
        start_time: None,
        end_time: None,
        limit: 1000, // Should be capped to 100
        offset: 0,
    };
    
    // Add some messages first
    let message = create_tip_message(&ctx.env, "test");
    let metadata = create_metadata(&ctx.env, &[]);
    ctx.tipjar_client.tip_with_message(&sender, &creator, &ctx.token_1, &100, &message, &metadata);
    
    let capped_results = ctx.tipjar_client.get_tip_history(&large_limit_query);
    assert!(capped_results.len() <= 100); // Should be capped
    
    // Test creator tips with large limit
    let large_creator_tips = ctx.tipjar_client.get_creator_tips(&creator, &1000);
    assert!(large_creator_tips.len() <= 100); // Should be capped
    
    // Test leaderboard with large limit
    let large_leaderboard = ctx.tipjar_client.get_top_tippers(&TimePeriod::AllTime, &0, &1000);
    assert!(large_leaderboard.len() <= 100); // Should be capped
    
    // Test batch with mixed valid/invalid creators
    let invalid_creator = ctx.create_user(); // Not a creator role
    let mixed_batch = create_batch_tips(
        &ctx.env,
        &[creator.clone(), invalid_creator],
        &ctx.token_1,
        &[100, 200],
    );
    
    // Should process valid entries and handle invalid ones gracefully
    let results = ctx.tipjar_client.tip_batch(&sender, &mixed_batch);
    assert_eq!(results.len(), 2);
    assert_eq!(results.get(0).unwrap(), Ok(()));
    // Second entry should succeed too (role not checked in batch tips)
    assert_eq!(results.get(1).unwrap(), Ok(()));
    
    // Test matching program with extreme ratios
    let sponsor = ctx.create_user();
    ctx.mint_tokens(&sponsor, &ctx.token_1, 1000);
    
    // Very high ratio (should work but be capped by budget)
    let high_ratio_program = ctx.tipjar_client.create_matching_program(
        &sponsor, &creator, &ctx.token_1, &10000, &500 // 100:1 ratio
    );
    
    let matched = ctx.tipjar_client.tip_with_match(&sender, &creator, &ctx.token_1, &1);
    assert_eq!(matched, 100); // 1 * 10000 / 100 = 100, but capped by budget
    
    // Test locked tip with same unlock time for multiple tips
    let current_time = ctx.get_current_time();
    let same_unlock_time = current_time + 1000;
    
    let tip_id_1 = ctx.tipjar_client.tip_locked(&sender, &creator, &ctx.token_1, &100, &same_unlock_time);
    let tip_id_2 = ctx.tipjar_client.tip_locked(&sender, &creator, &ctx.token_1, &200, &same_unlock_time);
    
    // Both should be withdrawable at the same time
    ctx.advance_time(1001);
    
    ctx.tipjar_client.withdraw_locked(&creator, &tip_id_1);
    ctx.tipjar_client.withdraw_locked(&creator, &tip_id_2);
}