mod common;
use common::*;
use tipjar::TipJarError;

// ── pause / unpause ───────────────────────────────────────────────────────────

#[test]
fn test_is_paused_false_by_default() {
    let ctx = TestContext::new();
    assert!(!ctx.tipjar_client.is_paused());
}

#[test]
fn test_pause_sets_paused_state() {
    let ctx = TestContext::new();
    let reason = soroban_sdk::String::from_str(&ctx.env, "security audit");
    ctx.tipjar_client.pause(&ctx.admin, &reason);
    assert!(ctx.tipjar_client.is_paused());
}

#[test]
fn test_unpause_clears_paused_state() {
    let ctx = TestContext::new();
    let reason = soroban_sdk::String::from_str(&ctx.env, "security audit");
    ctx.tipjar_client.pause(&ctx.admin, &reason);
    ctx.tipjar_client.unpause(&ctx.admin);
    assert!(!ctx.tipjar_client.is_paused());
}

#[test]
fn test_pause_requires_admin() {
    let ctx = TestContext::new();
    let non_admin = ctx.create_user();
    let reason = soroban_sdk::String::from_str(&ctx.env, "test");
    let result = ctx.tipjar_client.try_pause(&non_admin, &reason);
    assert_error_contains(result, TipJarError::Unauthorized);
}

#[test]
fn test_unpause_requires_admin() {
    let ctx = TestContext::new();
    let reason = soroban_sdk::String::from_str(&ctx.env, "test");
    ctx.tipjar_client.pause(&ctx.admin, &reason);
    let non_admin = ctx.create_user();
    let result = ctx.tipjar_client.try_unpause(&non_admin);
    assert_error_contains(result, TipJarError::Unauthorized);
}

// ── state-changing ops blocked when paused ────────────────────────────────────

fn pause_contract(ctx: &TestContext) {
    let reason = soroban_sdk::String::from_str(&ctx.env, "emergency");
    ctx.tipjar_client.pause(&ctx.admin, &reason);
}

#[test]
fn test_tip_blocked_when_paused() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);
    pause_contract(&ctx);
    let result = ctx.tipjar_client.try_tip(&sender, &creator, &ctx.token_1, &100);
    assert_error_contains(result, TipJarError::ContractPaused);
}

#[test]
fn test_withdraw_blocked_when_paused() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &100);
    pause_contract(&ctx);
    let result = ctx.tipjar_client.try_withdraw(&creator, &ctx.token_1, &None);
    assert_error_contains(result, TipJarError::ContractPaused);
}

#[test]
fn test_create_subscription_blocked_when_paused() {
    let ctx = TestContext::new();
    let subscriber = ctx.create_user();
    let creator = ctx.create_creator();
    pause_contract(&ctx);
    let result = ctx.tipjar_client.try_create_subscription(
        &subscriber, &creator, &ctx.token_1, &100, &86_400,
    );
    assert_error_contains(result, TipJarError::ContractPaused);
}

#[test]
fn test_tip_split_blocked_when_paused() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let c1 = ctx.create_creator();
    let c2 = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);
    pause_contract(&ctx);
    let mut recipients = soroban_sdk::Vec::new(&ctx.env);
    recipients.push_back(tipjar::TipRecipient { creator: c1, percentage: 5_000 });
    recipients.push_back(tipjar::TipRecipient { creator: c2, percentage: 5_000 });
    let result = ctx.tipjar_client.try_tip_split(&sender, &ctx.token_1, &recipients, &100);
    assert_error_contains(result, TipJarError::ContractPaused);
}

// ── view functions accessible during pause ────────────────────────────────────

#[test]
fn test_view_functions_work_when_paused() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &100);

    pause_contract(&ctx);

    // All reads must succeed
    assert_eq!(ctx.tipjar_client.is_paused(), true);
    assert_eq!(
        ctx.tipjar_client.get_withdrawable_balance(creator.clone(), ctx.token_1.clone()),
        100
    );
    assert_eq!(
        ctx.tipjar_client.get_total_tips(creator.clone(), ctx.token_1.clone()),
        100
    );
}

// ── operations resume after unpause ──────────────────────────────────────────

#[test]
fn test_tip_works_after_unpause() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);

    pause_contract(&ctx);
    ctx.tipjar_client.unpause(&ctx.admin);

    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &100);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 100);
}
