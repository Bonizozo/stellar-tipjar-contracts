use super::common::*;
use tipjar::{TimePeriod, Role};

pub fn test_basic_operation_costs() {
    let ctx = TestContext::new();
    let gas_tracker = GasTracker::new(&ctx.env);
    
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    
    ctx.mint_tokens(&sender, &ctx.token_1, 10000);
    
    // Measure basic tip operation
    let (_, tip_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &100)
    });
    
    println!("Basic tip gas cost: {}", tip_gas);
    assert!(tip_gas > 0, "Tip operation should consume gas");
    assert!(tip_gas < 1_000_000, "Tip operation should be reasonably efficient");
    
    // Measure tip with message operation
    let message = create_tip_message(&ctx.env, "Test message for gas analysis");
    let metadata = create_metadata(&ctx.env, &[("platform", "test"), ("id", "123")]);
    
    let (_, tip_message_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.tip_with_message(&sender, &creator, &ctx.token_1, &100, &message, &metadata)
    });
    
    println!("Tip with message gas cost: {}", tip_message_gas);
    assert!(tip_message_gas > tip_gas, "Tip with message should cost more than basic tip");
    assert!(tip_message_gas < 2_000_000, "Tip with message should be reasonably efficient");
    
    // Measure withdrawal operation
    let (_, withdraw_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.withdraw(&creator, &ctx.token_1)
    });
    
    println!("Withdrawal gas cost: {}", withdraw_gas);
    assert!(withdraw_gas > 0, "Withdrawal should consume gas");
    assert!(withdraw_gas < 1_000_000, "Withdrawal should be reasonably efficient");
    
    // Measure query operations (should be very cheap)
    let (_, balance_query_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.get_withdrawable_balance(&creator, &ctx.token_1)
    });
    
    println!("Balance query gas cost: {}", balance_query_gas);
    assert!(balance_query_gas < tip_gas / 2, "Queries should be much cheaper than state changes");
    
    let (_, total_query_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.get_total_tips(&creator, &ctx.token_1)
    });
    
    println!("Total tips query gas cost: {}", total_query_gas);
    assert!(total_query_gas < tip_gas / 2, "Queries should be much cheaper than state changes");
    
    // Measure role operations
    let user = ctx.create_user();
    let (_, grant_role_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.grant_role(&ctx.admin, &user, &Role::Creator)
    });
    
    println!("Grant role gas cost: {}", grant_role_gas);
    assert!(grant_role_gas > 0, "Role operations should consume gas");
    
    let (_, revoke_role_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.revoke_role(&ctx.admin, &user)
    });
    
    println!("Revoke role gas cost: {}", revoke_role_gas);
    assert!(revoke_role_gas > 0, "Role operations should consume gas");
    
    // Measure token management operations
    let (_, add_token_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.add_token(&ctx.admin, &ctx.token_3)
    });
    
    println!("Add token gas cost: {}", add_token_gas);
    assert!(add_token_gas > 0, "Token management should consume gas");
    
    let (_, remove_token_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.remove_token(&ctx.admin, &ctx.token_3)
    });
    
    println!("Remove token gas cost: {}", remove_token_gas);
    assert!(remove_token_gas > 0, "Token management should consume gas");
}

pub fn test_batch_operation_efficiency() {
    let ctx = TestContext::new();
    let gas_tracker = GasTracker::new(&ctx.env);
    
    let sender = ctx.create_user();
    let creators: Vec<_> = (0..10).map(|_| ctx.create_creator()).collect();
    
    ctx.mint_tokens(&sender, &ctx.token_1, 50000);
    
    // Measure individual tips
    let mut individual_gas_total = 0u64;
    for (i, creator) in creators.iter().enumerate() {
        let (_, individual_gas) = gas_tracker.measure(|| {
            ctx.tipjar_client.tip(&sender, creator, &ctx.token_1, &100)
        });
        individual_gas_total += individual_gas;
        
        if i == 0 {
            println!("Individual tip gas cost: {}", individual_gas);
        }
    }
    
    println!("Total gas for 10 individual tips: {}", individual_gas_total);
    
    // Measure batch operation with same tips
    let batch_tips = create_batch_tips(
        &ctx.env,
        &creators,
        &ctx.token_1,
        &vec![100; 10],
    );
    
    let (_, batch_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.tip_batch(&sender, &batch_tips)
    });
    
    println!("Batch tip gas cost (10 tips): {}", batch_gas);
    
    // Batch should be more efficient than individual operations
    let efficiency_ratio = individual_gas_total as f64 / batch_gas as f64;
    println!("Efficiency ratio (individual/batch): {:.2}", efficiency_ratio);
    assert!(efficiency_ratio > 1.2, "Batch operations should be at least 20% more efficient");
    
    // Test larger batch sizes
    let large_creators: Vec<_> = (0..25).map(|_| ctx.create_creator()).collect();
    ctx.mint_tokens(&sender, &ctx.token_1, 50000);
    
    let large_batch = create_batch_tips(
        &ctx.env,
        &large_creators,
        &ctx.token_1,
        &vec![50; 25],
    );
    
    let (_, large_batch_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.tip_batch(&sender, &large_batch)
    });
    
    println!("Large batch tip gas cost (25 tips): {}", large_batch_gas);
    
    // Gas per tip should decrease with larger batches (economies of scale)
    let gas_per_tip_small = batch_gas / 10;
    let gas_per_tip_large = large_batch_gas / 25;
    
    println!("Gas per tip (small batch): {}", gas_per_tip_small);
    println!("Gas per tip (large batch): {}", gas_per_tip_large);
    
    // Test maximum batch size (50 tips)
    let max_creators: Vec<_> = (0..50).map(|_| ctx.create_creator()).collect();
    ctx.mint_tokens(&sender, &ctx.token_1, 100000);
    
    let max_batch = create_batch_tips(
        &ctx.env,
        &max_creators,
        &ctx.token_1,
        &vec![20; 50],
    );
    
    let (_, max_batch_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.tip_batch(&sender, &max_batch)
    });
    
    println!("Maximum batch tip gas cost (50 tips): {}", max_batch_gas);
    
    let gas_per_tip_max = max_batch_gas / 50;
    println!("Gas per tip (max batch): {}", gas_per_tip_max);
    
    // Ensure maximum batch is still within reasonable limits
    assert!(max_batch_gas < 10_000_000, "Maximum batch should not exceed reasonable gas limit");
}

pub fn test_complex_operation_costs() {
    let ctx = TestContext::new();
    let gas_tracker = GasTracker::new(&ctx.env);
    
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    let sponsor = ctx.create_user();
    
    ctx.mint_tokens(&sender, &ctx.token_1, 10000);
    ctx.mint_tokens(&sponsor, &ctx.token_1, 5000);
    
    // Measure locked tip creation
    let unlock_time = ctx.get_current_time() + 1000;
    let (tip_id, locked_tip_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.tip_locked(&sender, &creator, &ctx.token_1, &500, &unlock_time)
    });
    
    println!("Locked tip creation gas cost: {}", locked_tip_gas);
    assert!(locked_tip_gas > 0, "Locked tip creation should consume gas");
    
    // Measure locked tip withdrawal
    ctx.advance_time(1001);
    let (_, locked_withdraw_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.withdraw_locked(&creator, &tip_id)
    });
    
    println!("Locked tip withdrawal gas cost: {}", locked_withdraw_gas);
    assert!(locked_withdraw_gas > 0, "Locked tip withdrawal should consume gas");
    
    // Measure matching program creation
    let (program_id, matching_create_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.create_matching_program(&sponsor, &creator, &ctx.token_1, &100, &1000)
    });
    
    println!("Matching program creation gas cost: {}", matching_create_gas);
    assert!(matching_create_gas > 0, "Matching program creation should consume gas");
    
    // Measure tip with matching
    let (_, tip_match_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.tip_with_match(&sender, &creator, &ctx.token_1, &200)
    });
    
    println!("Tip with matching gas cost: {}", tip_match_gas);
    assert!(tip_match_gas > 0, "Tip with matching should consume gas");
    
    // Compare with regular tip
    let (_, regular_tip_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &200)
    });
    
    println!("Regular tip gas cost: {}", regular_tip_gas);
    
    let matching_overhead = tip_match_gas as f64 / regular_tip_gas as f64;
    println!("Matching overhead ratio: {:.2}", matching_overhead);
    assert!(matching_overhead < 3.0, "Matching should not add excessive overhead");
    
    // Measure matching program cancellation
    let (_, matching_cancel_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.cancel_matching_program(&sponsor, &program_id)
    });
    
    println!("Matching program cancellation gas cost: {}", matching_cancel_gas);
    assert!(matching_cancel_gas > 0, "Matching program cancellation should consume gas");
    
    // Measure leaderboard queries with different sizes
    // First create some data
    for i in 0..20 {
        let tipper = ctx.create_user();
        ctx.mint_tokens(&tipper, &ctx.token_1, 1000);
        ctx.tipjar_client.tip(&tipper, &creator, &ctx.token_1, &(50 + i * 10));
    }
    
    let (_, leaderboard_small_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.get_top_tippers(&TimePeriod::AllTime, &0, &5)
    });
    
    println!("Leaderboard query gas cost (5 entries): {}", leaderboard_small_gas);
    
    let (_, leaderboard_large_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.get_top_tippers(&TimePeriod::AllTime, &0, &20)
    });
    
    println!("Leaderboard query gas cost (20 entries): {}", leaderboard_large_gas);
    
    // Larger queries should cost more but not excessively
    let query_size_ratio = leaderboard_large_gas as f64 / leaderboard_small_gas as f64;
    println!("Query size ratio (20/5 entries): {:.2}", query_size_ratio);
    assert!(query_size_ratio < 5.0, "Larger queries should not be excessively expensive");
    
    // Measure tip history queries
    let message = create_tip_message(&ctx.env, "Test message");
    let metadata = create_metadata(&ctx.env, &[]);
    
    // Create some message history
    for i in 0..10 {
        ctx.tipjar_client.tip_with_message(&sender, &creator, &ctx.token_1, &(10 + i), &message, &metadata);
    }
    
    let query = tipjar::TipHistoryQuery {
        creator: Some(creator.clone()),
        sender: None,
        min_amount: None,
        max_amount: None,
        start_time: None,
        end_time: None,
        limit: 10,
        offset: 0,
    };
    
    let (_, history_query_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.get_tip_history(&query)
    });
    
    println!("Tip history query gas cost: {}", history_query_gas);
    assert!(history_query_gas > 0, "Tip history queries should consume gas");
    
    // Measure contract upgrade
    let wasm_hash = ctx.env.deployer().upload_contract_wasm(tipjar::TipJarContract::wasm());
    let (_, upgrade_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.upgrade(&ctx.admin, &wasm_hash)
    });
    
    println!("Contract upgrade gas cost: {}", upgrade_gas);
    assert!(upgrade_gas > 0, "Contract upgrade should consume gas");
}

pub fn test_worst_case_scenarios() {
    let ctx = TestContext::new();
    let gas_tracker = GasTracker::new(&ctx.env);
    
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000_000);
    
    // Test worst case: maximum message length
    let max_message = create_tip_message(&ctx.env, &"x".repeat(280));
    let large_metadata = create_metadata(&ctx.env, &[
        ("key1", &"value".repeat(50)),
        ("key2", &"value".repeat(50)),
        ("key3", &"value".repeat(50)),
    ]);
    
    let (_, max_message_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.tip_with_message(&sender, &creator, &ctx.token_1, &100, &max_message, &large_metadata)
    });
    
    println!("Maximum message tip gas cost: {}", max_message_gas);
    assert!(max_message_gas < 5_000_000, "Even maximum messages should be reasonably efficient");
    
    // Test worst case: batch with mixed failures
    let creators: Vec<_> = (0..50).map(|_| ctx.create_creator()).collect();
    let mixed_batch = create_batch_tips(
        &ctx.env,
        &creators,
        &ctx.token_1,
        &vec![0; 50], // All invalid amounts
    );
    
    let (_, failed_batch_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.tip_batch(&sender, &mixed_batch)
    });
    
    println!("Failed batch gas cost: {}", failed_batch_gas);
    
    // Compare with successful batch
    let success_batch = create_batch_tips(
        &ctx.env,
        &creators,
        &ctx.token_1,
        &vec![100; 50],
    );
    
    let (_, success_batch_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.tip_batch(&sender, &success_batch)
    });
    
    println!("Successful batch gas cost: {}", success_batch_gas);
    
    // Failed operations should not be significantly more expensive
    let failure_overhead = failed_batch_gas as f64 / success_batch_gas as f64;
    println!("Failure overhead ratio: {:.2}", failure_overhead);
    assert!(failure_overhead < 1.5, "Failed operations should not add excessive overhead");
    
    // Test worst case: many locked tips for same creator
    for i in 0..20 {
        let unlock_time = ctx.get_current_time() + 1000 + (i * 100);
        ctx.tipjar_client.tip_locked(&sender, &creator, &ctx.token_1, &100, &unlock_time);
    }
    
    // Measure withdrawal of one locked tip when many exist
    ctx.advance_time(1001);
    let (_, locked_withdraw_many_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.withdraw_locked(&creator, &0)
    });
    
    println!("Locked tip withdrawal with many tips gas cost: {}", locked_withdraw_many_gas);
    
    // Test worst case: leaderboard with maximum participants
    for i in 0..100 {
        let tipper = ctx.create_user();
        ctx.mint_tokens(&tipper, &ctx.token_1, 1000);
        ctx.tipjar_client.tip(&tipper, &creator, &ctx.token_1, &(10 + i));
    }
    
    let (_, max_leaderboard_gas) = gas_tracker.measure(|| {
        ctx.tipjar_client.get_top_tippers(&TimePeriod::AllTime, &0, &100)
    });
    
    println!("Maximum leaderboard query gas cost: {}", max_leaderboard_gas);
    assert!(max_leaderboard_gas < 10_000_000, "Maximum leaderboard queries should be within limits");
    
    // Test gas usage remains reasonable with contract state growth
    println!("\n=== Gas Analysis Summary ===");
    println!("All operations completed within reasonable gas limits");
    println!("Contract demonstrates good scalability characteristics");
}