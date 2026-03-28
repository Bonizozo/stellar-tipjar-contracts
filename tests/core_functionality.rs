use super::common::*;
use tipjar::{Role, TipJarError};

pub fn test_complete_tip_workflows() {
    let ctx = TestContext::new();
    
    // Test complete tip-to-withdrawal cycle with multiple tokens
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    
    // Mint tokens for sender
    ctx.mint_tokens(&sender, &ctx.token_1, 1000);
    ctx.mint_tokens(&sender, &ctx.token_2, 2000);
    
    // Test basic tip workflow
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &300);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 300);
    assert_total_tips_equals(&ctx, &creator, &ctx.token_1, 300);
    
    // Test tip with message workflow
    let message = create_tip_message(&ctx.env, "Great content!");
    let metadata = create_metadata(&ctx.env, &[("platform", "youtube"), ("video_id", "abc123")]);
    ctx.tipjar_client.tip_with_message(&sender, &creator, &ctx.token_2, &500, &message, &metadata);
    
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_2, 500);
    assert_total_tips_equals(&ctx, &creator, &ctx.token_2, 500);
    
    // Verify messages are stored
    let messages = ctx.tipjar_client.get_messages(&creator);
    assert_eq!(messages.len(), 1);
    let stored_message = messages.get(0).unwrap();
    assert_eq!(stored_message.amount, 500);
    assert_eq!(stored_message.message, message);
    
    // Test withdrawal workflow
    let creator_balance_before = ctx.get_token_balance(&creator, &ctx.token_1);
    ctx.tipjar_client.withdraw(&creator, &ctx.token_1);
    
    assert_balance_equals(&ctx, &creator, &ctx.token_1, creator_balance_before + 300);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 0);
    assert_total_tips_equals(&ctx, &creator, &ctx.token_1, 300); // Total remains
    
    // Test multi-token independence
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_2, 500); // Unchanged
}

pub fn test_role_based_operations() {
    let ctx = TestContext::new();
    
    let user = ctx.create_user();
    let creator = ctx.create_creator();
    let moderator = ctx.create_moderator();
    
    // Test admin operations
    assert!(ctx.tipjar_client.has_role(&ctx.admin, &Role::Admin));
    
    // Test role granting and revoking
    ctx.tipjar_client.grant_role(&ctx.admin, &user, &Role::Creator);
    assert!(ctx.tipjar_client.has_role(&user, &Role::Creator));
    
    ctx.tipjar_client.revoke_role(&ctx.admin, &user);
    assert!(!ctx.tipjar_client.has_role(&user, &Role::Creator));
    
    // Test moderator can pause/unpause
    ctx.tipjar_client.pause(&moderator);
    
    // Verify operations are blocked when paused
    ctx.mint_tokens(&user, &ctx.token_1, 1000);
    let result = ctx.tipjar_client.try_tip(&user, &creator, &ctx.token_1, &100);
    assert!(result.is_err());
    
    ctx.tipjar_client.unpause(&moderator);
    
    // Verify operations work after unpause
    ctx.tipjar_client.tip(&user, &creator, &ctx.token_1, &100);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 100);
    
    // Test creator can withdraw
    ctx.tipjar_client.withdraw(&creator, &ctx.token_1);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 0);
    
    // Test unauthorized operations fail
    let result = ctx.tipjar_client.try_grant_role(&user, &creator, &Role::Admin);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    let result = ctx.tipjar_client.try_pause(&user);
    assert_error_contains(result, TipJarError::Unauthorized);
}

pub fn test_token_management_workflows() {
    let ctx = TestContext::new();
    
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    
    ctx.mint_tokens(&sender, &ctx.token_1, 1000);
    ctx.mint_tokens(&sender, &ctx.token_3, 1000);
    
    // Test whitelisted token works
    assert!(ctx.tipjar_client.is_whitelisted(&ctx.token_1));
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &100);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 100);
    
    // Test non-whitelisted token fails
    assert!(!ctx.tipjar_client.is_whitelisted(&ctx.token_3));
    let result = ctx.tipjar_client.try_tip(&sender, &creator, &ctx.token_3, &100);
    assert_error_contains(result, TipJarError::TokenNotWhitelisted);
    
    // Test adding token to whitelist
    ctx.tipjar_client.add_token(&ctx.admin, &ctx.token_3);
    assert!(ctx.tipjar_client.is_whitelisted(&ctx.token_3));
    
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_3, &200);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_3, 200);
    
    // Test removing token from whitelist
    ctx.tipjar_client.remove_token(&ctx.admin, &ctx.token_3);
    assert!(!ctx.tipjar_client.is_whitelisted(&ctx.token_3));
    
    let result = ctx.tipjar_client.try_tip(&sender, &creator, &ctx.token_3, &100);
    assert_error_contains(result, TipJarError::TokenNotWhitelisted);
    
    // Test non-admin cannot manage tokens
    let user = ctx.create_user();
    let result = ctx.tipjar_client.try_add_token(&user, &ctx.token_3);
    assert_error_contains(result, TipJarError::Unauthorized);
}

pub fn test_pause_unpause_workflows() {
    let ctx = TestContext::new();
    
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    let moderator = ctx.create_moderator();
    
    ctx.mint_tokens(&sender, &ctx.token_1, 1000);
    
    // Test normal operations work when not paused
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &100);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 100);
    
    // Test pause blocks state-changing operations
    ctx.tipjar_client.pause(&ctx.admin);
    
    let result = ctx.tipjar_client.try_tip(&sender, &creator, &ctx.token_1, &100);
    assert!(result.is_err());
    
    let result = ctx.tipjar_client.try_withdraw(&creator, &ctx.token_1);
    assert!(result.is_err());
    
    // Test queries still work when paused
    let balance = ctx.tipjar_client.get_withdrawable_balance(&creator, &ctx.token_1);
    assert_eq!(balance, 100);
    
    let total = ctx.tipjar_client.get_total_tips(&creator, &ctx.token_1);
    assert_eq!(total, 100);
    
    // Test moderator can unpause
    ctx.tipjar_client.unpause(&moderator);
    
    // Test operations work after unpause
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &200);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 300);
    
    ctx.tipjar_client.withdraw(&creator, &ctx.token_1);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 0);
}

pub fn test_upgrade_workflows() {
    let ctx = TestContext::new();
    
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    
    ctx.mint_tokens(&sender, &ctx.token_1, 1000);
    
    // Establish state before upgrade
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &500);
    assert_eq!(ctx.tipjar_client.get_version(), 0);
    
    // Simulate upgrade
    let wasm_hash = ctx.env.deployer().upload_contract_wasm(tipjar::TipJarContract::wasm());
    ctx.tipjar_client.upgrade(&ctx.admin, &wasm_hash);
    
    // Verify version incremented
    assert_eq!(ctx.tipjar_client.get_version(), 1);
    
    // Verify state preserved
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 500);
    assert_total_tips_equals(&ctx, &creator, &ctx.token_1, 500);
    
    // Verify functionality still works
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &300);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 800);
    
    // Test multiple upgrades
    ctx.tipjar_client.upgrade(&ctx.admin, &wasm_hash);
    assert_eq!(ctx.tipjar_client.get_version(), 2);
    
    // Test non-admin cannot upgrade
    let user = ctx.create_user();
    let result = ctx.tipjar_client.try_upgrade(&user, &wasm_hash);
    assert_error_contains(result, TipJarError::Unauthorized);
    
    // Verify upgrade events are emitted
    let events = ctx.get_events();
    let upgrade_events: Vec<_> = events.iter()
        .filter(|(_, topics, _)| {
            if let Ok(symbol) = soroban_sdk::Symbol::try_from_val(&ctx.env, &topics.get(0).unwrap()) {
                symbol == soroban_sdk::Symbol::new(&ctx.env, "upgraded")
            } else {
                false
            }
        })
        .collect();
    assert_eq!(upgrade_events.len(), 2); // Two upgrades performed
}