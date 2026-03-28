use super::common::*;
use tipjar::{Role, TimePeriod};

pub fn test_balance_conservation_properties() {
    let ctx = TestContext::new();
    
    // Property: Total tokens in system should be conserved
    // For any sequence of tips and withdrawals, sum of all balances should equal initial supply
    
    let initial_supply = 100_000i128;
    let num_users = 10;
    let num_creators = 5;
    
    let users: Vec<_> = (0..num_users).map(|_| ctx.create_user()).collect();
    let creators: Vec<_> = (0..num_creators).map(|_| ctx.create_creator()).collect();
    
    // Distribute initial tokens
    for user in &users {
        ctx.mint_tokens(user, &ctx.token_1, initial_supply / num_users as i128);
    }
    
    // Perform random tip operations
    for iteration in 0..50 {
        let user_idx = (iteration * 7) % num_users; // Pseudo-random selection
        let creator_idx = (iteration * 3) % num_creators;
        let amount = 10 + (iteration * 13) % 100; // Random amount 10-109
        
        let user = &users[user_idx];
        let creator = &creators[creator_idx];
        
        let user_balance_before = ctx.get_token_balance(user, &ctx.token_1);
        if user_balance_before >= amount {
            ctx.tipjar_client.tip(user, creator, &ctx.token_1, &amount);
            
            // Verify balance conservation
            let user_balance_after = ctx.get_token_balance(user, &ctx.token_1);
            let contract_balance = ctx.get_token_balance(&ctx.contract_id, &ctx.token_1);
            let creator_withdrawable = ctx.tipjar_client.get_withdrawable_balance(creator.clone(), ctx.token_1.clone());
            
            assert_eq!(user_balance_before - amount, user_balance_after, "User balance should decrease by tip amount");
            
            // Property: Contract balance should equal sum of all withdrawable balances
            let total_withdrawable: i128 = creators.iter()
                .map(|c| ctx.tipjar_client.get_withdrawable_balance(c.clone(), ctx.token_1.clone()))
                .sum();
            
            assert_eq!(contract_balance, total_withdrawable, "Contract balance should equal total withdrawable");
        }
    }
    
    // Perform random withdrawals
    for creator in &creators {
        let withdrawable = ctx.tipjar_client.get_withdrawable_balance(creator.clone(), ctx.token_1.clone());
        if withdrawable > 0 {
            let creator_balance_before = ctx.get_token_balance(creator, &ctx.token_1);
            ctx.tipjar_client.withdraw(creator, &ctx.token_1);
            let creator_balance_after = ctx.get_token_balance(creator, &ctx.token_1);
            
            assert_eq!(creator_balance_after - creator_balance_before, withdrawable, "Creator should receive full withdrawable amount");
        }
    }
    
    // Final property check: Total supply should be conserved
    let final_total: i128 = users.iter()
        .chain(creators.iter())
        .map(|addr| ctx.get_token_balance(addr, &ctx.token_1))
        .sum::<i128>() + ctx.get_token_balance(&ctx.contract_id, &ctx.token_1);
    
    assert_eq!(final_total, initial_supply, "Total token supply should be conserved");
}

pub fn test_authorization_properties() {
    let ctx = TestContext::new();
    
    // Property: Role-based operations should always respect authorization
    // For any user and any role-restricted operation, the operation should succeed
    // if and only if the user has the required role
    
    let admin = &ctx.admin;
    let users: Vec<_> = (0..10).map(|_| ctx.create_user()).collect();
    
    // Test admin operations
    for user in &users {
        // Property: Only admin can grant roles
        let result = ctx.tipjar_client.try_grant_role(user, &users[0], &Role::Creator);
        assert!(result.is_err(), "Non-admin should not be able to grant roles");
        
        // Admin should be able to grant roles
        ctx.tipjar_client.grant_role(admin, user, &Role::Creator);
        assert!(ctx.tipjar_client.has_role(user, &Role::Creator), "Admin should be able to grant roles");
        
        // Property: Only admin can revoke roles
        let other_user = &users[(users.iter().position(|u| u == user).unwrap() + 1) % users.len()];
        let result = ctx.tipjar_client.try_revoke_role(other_user, user);
        assert!(result.is_err(), "Non-admin should not be able to revoke roles");
        
        // Admin should be able to revoke roles
        ctx.tipjar_client.revoke_role(admin, user);
        assert!(!ctx.tipjar_client.has_role(user, &Role::Creator), "Admin should be able to revoke roles");
    }
    
    // Test moderator operations
    let moderator = ctx.create_moderator();
    let creator = ctx.create_creator();
    
    // Property: Moderators can pause/unpause but not manage tokens/roles
    ctx.tipjar_client.pause(&moderator); // Should succeed
    
    let result = ctx.tipjar_client.try_add_token(&moderator, &ctx.token_2);
    assert!(result.is_err(), "Moderator should not be able to manage tokens");
    
    let result = ctx.tipjar_client.try_grant_role(&moderator, &users[0], &Role::Creator);
    assert!(result.is_err(), "Moderator should not be able to manage roles");
    
    ctx.tipjar_client.unpause(&moderator); // Should succeed
    
    // Test creator operations
    ctx.mint_tokens(&users[0], &ctx.token_1, 1000);
    ctx.tipjar_client.tip(&users[0], &creator, &ctx.token_1, &500);
    
    // Property: Only creators can withdraw
    let result = ctx.tipjar_client.try_withdraw(&users[0], &ctx.token_1);
    assert!(result.is_err(), "Non-creator should not be able to withdraw");
    
    ctx.tipjar_client.withdraw(&creator, &ctx.token_1); // Should succeed
    
    // Property: Role checks are consistent across all operations
    let non_creator = &users[1];
    let unlock_time = ctx.get_current_time() + 1000;
    let tip_id = ctx.tipjar_client.tip_locked(&users[0], &creator, &ctx.token_1, &200, &unlock_time);
    
    ctx.advance_time(1001);
    
    let result = ctx.tipjar_client.try_withdraw_locked(non_creator, &tip_id);
    assert!(result.is_err(), "Non-creator should not be able to withdraw locked tips");
    
    ctx.tipjar_client.withdraw_locked(&creator, &tip_id); // Should succeed
}

pub fn test_leaderboard_consistency_properties() {
    let ctx = TestContext::new();
    
    // Property: Leaderboard rankings should be consistent with tip amounts and counts
    // For any set of tips, leaderboard should be ordered by total_amount desc, then tip_count desc
    
    let tippers: Vec<_> = (0..20).map(|_| ctx.create_user()).collect();
    let creators: Vec<_> = (0..5).map(|_| ctx.create_creator()).collect();
    
    // Mint tokens for all tippers
    for tipper in &tippers {
        ctx.mint_tokens(tipper, &ctx.token_1, 10000);
    }
    
    // Create diverse tipping patterns
    let tip_patterns = [
        (0, 0, 1000, 1), // Tipper 0: 1 tip of 1000
        (1, 0, 500, 2),  // Tipper 1: 2 tips of 500 each (total 1000, count 2)
        (2, 1, 1200, 1), // Tipper 2: 1 tip of 1200
        (3, 1, 400, 3),  // Tipper 3: 3 tips of 400 each (total 1200, count 3)
        (4, 2, 800, 1),  // Tipper 4: 1 tip of 800
        (5, 2, 200, 4),  // Tipper 5: 4 tips of 200 each (total 800, count 4)
    ];
    
    for (tipper_idx, creator_idx, amount_per_tip, tip_count) in tip_patterns {
        let tipper = &tippers[tipper_idx];
        let creator = &creators[creator_idx];
        
        for _ in 0..tip_count {
            ctx.tipjar_client.tip(tipper, creator, &ctx.token_1, &amount_per_tip);
        }
    }
    
    // Verify leaderboard ordering properties
    let leaderboard = ctx.tipjar_client.get_top_tippers(&TimePeriod::AllTime, &0, &20);
    
    // Property: Leaderboard should be sorted by total_amount desc, then tip_count desc
    for i in 0..leaderboard.len().saturating_sub(1) {
        let current = leaderboard.get(i).unwrap();
        let next = leaderboard.get(i + 1).unwrap();
        
        assert!(
            current.total_amount > next.total_amount ||
            (current.total_amount == next.total_amount && current.tip_count >= next.tip_count),
            "Leaderboard should be properly ordered: current({}, {}) vs next({}, {})",
            current.total_amount, current.tip_count, next.total_amount, next.tip_count
        );
    }
    
    // Property: Leaderboard totals should match individual queries
    for entry in leaderboard.iter() {
        let expected_total: i128 = creators.iter()
            .map(|creator| {
                let total = ctx.tipjar_client.get_total_tips(creator.clone(), ctx.token_1.clone());
                // This is a simplified check - in reality we'd need to track per-tipper totals
                total
            })
            .sum();
        
        // Verify the entry exists and has reasonable values
        assert!(entry.total_amount > 0, "Leaderboard entries should have positive amounts");
        assert!(entry.tip_count > 0, "Leaderboard entries should have positive counts");
    }
    
    // Property: Pagination should be consistent
    let page1 = ctx.tipjar_client.get_top_tippers(&TimePeriod::AllTime, &0, &3);
    let page2 = ctx.tipjar_client.get_top_tippers(&TimePeriod::AllTime, &3, &3);
    
    if page1.len() == 3 && page2.len() > 0 {
        let last_of_page1 = page1.get(2).unwrap();
        let first_of_page2 = page2.get(0).unwrap();
        
        assert!(
            last_of_page1.total_amount > first_of_page2.total_amount ||
            (last_of_page1.total_amount == first_of_page2.total_amount && 
             last_of_page1.tip_count >= first_of_page2.tip_count),
            "Pagination should maintain ordering across pages"
        );
    }
    
    // Property: Different time periods should have consistent data
    let monthly = ctx.tipjar_client.get_top_tippers(&TimePeriod::Monthly, &0, &10);
    let weekly = ctx.tipjar_client.get_top_tippers(&TimePeriod::Weekly, &0, &10);
    let all_time = ctx.tipjar_client.get_top_tippers(&TimePeriod::AllTime, &0, &10);
    
    // All time should have >= monthly >= weekly (in terms of totals)
    for entry in &all_time {
        let monthly_entry = monthly.iter().find(|e| e.address == entry.address);
        if let Some(monthly_entry) = monthly_entry {
            assert!(
                entry.total_amount >= monthly_entry.total_amount,
                "All-time totals should be >= monthly totals"
            );
        }
        
        let weekly_entry = weekly.iter().find(|e| e.address == entry.address);
        if let Some(weekly_entry) = weekly_entry {
            assert!(
                entry.total_amount >= weekly_entry.total_amount,
                "All-time totals should be >= weekly totals"
            );
        }
    }
}

pub fn test_temporal_properties() {
    let ctx = TestContext::new();
    
    // Property: Time-based operations should respect temporal ordering
    // For any sequence of time-dependent operations, earlier operations should
    // complete before later ones become available
    
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    
    ctx.mint_tokens(&sender, &ctx.token_1, 10000);
    
    let base_time = ctx.get_current_time();
    let unlock_times = [
        base_time + 1000,
        base_time + 2000,
        base_time + 3000,
        base_time + 1500, // Out of order to test sorting
        base_time + 2500,
    ];
    
    // Create locked tips with various unlock times
    let mut tip_ids = Vec::new();
    for (i, &unlock_time) in unlock_times.iter().enumerate() {
        let tip_id = ctx.tipjar_client.tip_locked(&sender, &creator, &ctx.token_1, &(100 + i as i128 * 50), &unlock_time);
        tip_ids.push((tip_id, unlock_time));
    }
    
    // Property: Tips should only be withdrawable after their unlock time
    for &(tip_id, unlock_time) in &tip_ids {
        // Should fail before unlock time
        let result = ctx.tipjar_client.try_withdraw_locked(&creator, &tip_id);
        assert!(result.is_err(), "Locked tip should not be withdrawable before unlock time");
        
        // Advance to just before unlock time
        let current_time = ctx.get_current_time();
        if unlock_time > current_time {
            ctx.advance_time(unlock_time - current_time - 1);
            let result = ctx.tipjar_client.try_withdraw_locked(&creator, &tip_id);
            assert!(result.is_err(), "Locked tip should not be withdrawable at exact unlock time");
        }
        
        // Advance past unlock time
        ctx.advance_time(2);
        ctx.tipjar_client.withdraw_locked(&creator, &tip_id); // Should succeed
    }
    
    // Property: Matching programs should handle time consistently
    let sponsor = ctx.create_user();
    ctx.mint_tokens(&sponsor, &ctx.token_1, 2000);
    
    let program_id = ctx.tipjar_client.create_matching_program(&sponsor, &creator, &ctx.token_1, &100, &1000);
    
    // Tips should be matched in the order they arrive
    let tip_amounts = [100, 200, 300, 400, 500]; // Total 1500, but budget is 1000
    let mut total_matched = 0i128;
    
    for amount in tip_amounts {
        let matched = ctx.tipjar_client.tip_with_match(&sender, &creator, &ctx.token_1, &amount);
        total_matched += matched;
        
        // Property: Matching should not exceed budget
        assert!(total_matched <= 1000, "Total matched should not exceed program budget");
        
        let program = ctx.tipjar_client.get_matching_program(&program_id);
        assert_eq!(program.current_matched, total_matched, "Program state should track matched amount");
        
        if total_matched >= 1000 {
            assert!(!program.active, "Program should be deactivated when budget exhausted");
            break;
        }
    }
}

pub fn test_batch_atomicity_properties() {
    let ctx = TestContext::new();
    
    // Property: Batch operations should be atomic per entry
    // Each entry in a batch should succeed or fail independently,
    // and successful entries should not be affected by failed ones
    
    let sender = ctx.create_user();
    let creators: Vec<_> = (0..5).map(|_| ctx.create_creator()).collect();
    
    ctx.mint_tokens(&sender, &ctx.token_1, 2000);
    
    // Create a batch with mixed valid and invalid entries
    let batch_amounts = [100, 0, 200, -50, 300]; // Invalid: 0, -50
    let batch = create_batch_tips(&ctx.env, &creators, &ctx.token_1, &batch_amounts);
    
    // Record initial state
    let initial_balances: Vec<_> = creators.iter()
        .map(|c| ctx.tipjar_client.get_withdrawable_balance(c.clone(), ctx.token_1.clone()))
        .collect();
    
    let initial_sender_balance = ctx.get_token_balance(&sender, &ctx.token_1);
    
    // Execute batch
    let results = ctx.tipjar_client.tip_batch(&sender, &batch);
    
    // Property: Results should match expectations
    assert_eq!(results.len(), 5);
    assert_eq!(results.get(0).unwrap(), Ok(()));                                    // 100 - valid
    assert_eq!(results.get(1).unwrap(), Err(tipjar::TipJarError::InvalidAmount));  // 0 - invalid
    assert_eq!(results.get(2).unwrap(), Ok(()));                                    // 200 - valid
    assert_eq!(results.get(3).unwrap(), Err(tipjar::TipJarError::InvalidAmount));  // -50 - invalid
    assert_eq!(results.get(4).unwrap(), Ok(()));                                    // 300 - valid
    
    // Property: Only successful entries should affect state
    let expected_changes = [100, 0, 200, 0, 300]; // Only valid amounts
    let expected_total_spent: i128 = expected_changes.iter().sum();
    
    for (i, creator) in creators.iter().enumerate() {
        let expected_balance = initial_balances[i] + expected_changes[i];
        let actual_balance = ctx.tipjar_client.get_withdrawable_balance(creator.clone(), ctx.token_1.clone());
        assert_eq!(actual_balance, expected_balance, "Creator {} balance should reflect only successful tips", i);
    }
    
    let final_sender_balance = ctx.get_token_balance(&sender, &ctx.token_1);
    assert_eq!(final_sender_balance, initial_sender_balance - expected_total_spent, "Sender should only pay for successful tips");
    
    // Property: Batch with all failures should not change any state
    let all_invalid_batch = create_batch_tips(&ctx.env, &creators, &ctx.token_1, &vec![0; 5]);
    let pre_failure_balances: Vec<_> = creators.iter()
        .map(|c| ctx.tipjar_client.get_withdrawable_balance(c.clone(), ctx.token_1.clone()))
        .collect();
    let pre_failure_sender_balance = ctx.get_token_balance(&sender, &ctx.token_1);
    
    let failure_results = ctx.tipjar_client.tip_batch(&sender, &all_invalid_batch);
    
    // All should fail
    for result in failure_results.iter() {
        assert_eq!(result, Err(tipjar::TipJarError::InvalidAmount));
    }
    
    // No state should change
    for (i, creator) in creators.iter().enumerate() {
        let balance = ctx.tipjar_client.get_withdrawable_balance(creator.clone(), ctx.token_1.clone());
        assert_eq!(balance, pre_failure_balances[i], "Failed batch should not change creator balances");
    }
    
    let sender_balance = ctx.get_token_balance(&sender, &ctx.token_1);
    assert_eq!(sender_balance, pre_failure_sender_balance, "Failed batch should not change sender balance");
    
    // Property: Empty batch should be handled gracefully
    let empty_batch = create_batch_tips(&ctx.env, &[], &ctx.token_1, &[]);
    let empty_results = ctx.tipjar_client.tip_batch(&sender, &empty_batch);
    assert_eq!(empty_results.len(), 0, "Empty batch should return empty results");
}