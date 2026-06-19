mod common;
use common::*;
use tipjar::{PauseScope, TipJarError};

// ── is_feature_paused defaults ────────────────────────────────────────────────

#[test]
fn test_feature_pause_false_by_default() {
    let ctx = TestContext::new();
    assert!(!ctx.tipjar_client.is_feature_paused(&PauseScope::Tipping));
    assert!(!ctx.tipjar_client.is_feature_paused(&PauseScope::Withdrawals));
    assert!(!ctx.tipjar_client.is_feature_paused(&PauseScope::Subscriptions));
}

// ── admin controls ────────────────────────────────────────────────────────────

#[test]
fn test_pause_feature_requires_admin() {
    let ctx = TestContext::new();
    let non_admin = ctx.create_user();
    let reason = soroban_sdk::String::from_str(&ctx.env, "test");
    let result = ctx
        .tipjar_client
        .try_pause_feature(&non_admin, &PauseScope::Tipping, &reason);
    assert_error_contains(result, TipJarError::Unauthorized);
}

#[test]
fn test_unpause_feature_requires_admin() {
    let ctx = TestContext::new();
    let reason = soroban_sdk::String::from_str(&ctx.env, "test");
    ctx.tipjar_client
        .pause_feature(&ctx.admin, &PauseScope::Tipping, &reason);
    let non_admin = ctx.create_user();
    let result = ctx
        .tipjar_client
        .try_unpause_feature(&non_admin, &PauseScope::Tipping);
    assert_error_contains(result, TipJarError::Unauthorized);
}

#[test]
fn test_pause_and_unpause_feature_roundtrip() {
    let ctx = TestContext::new();
    let reason = soroban_sdk::String::from_str(&ctx.env, "security review");
    ctx.tipjar_client
        .pause_feature(&ctx.admin, &PauseScope::Tipping, &reason);
    assert!(ctx.tipjar_client.is_feature_paused(&PauseScope::Tipping));
    ctx.tipjar_client
        .unpause_feature(&ctx.admin, &PauseScope::Tipping);
    assert!(!ctx.tipjar_client.is_feature_paused(&PauseScope::Tipping));
}

// ── scopes are independent ────────────────────────────────────────────────────

#[test]
fn test_scopes_are_independent() {
    let ctx = TestContext::new();
    let reason = soroban_sdk::String::from_str(&ctx.env, "test");
    ctx.tipjar_client
        .pause_feature(&ctx.admin, &PauseScope::Withdrawals, &reason);

    // Only Withdrawals is paused
    assert!(!ctx.tipjar_client.is_feature_paused(&PauseScope::Tipping));
    assert!(ctx
        .tipjar_client
        .is_feature_paused(&PauseScope::Withdrawals));
    assert!(!ctx
        .tipjar_client
        .is_feature_paused(&PauseScope::Subscriptions));
}

// ── Tipping scope ─────────────────────────────────────────────────────────────

#[test]
fn test_tip_blocked_when_tipping_scope_paused() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);
    let reason = soroban_sdk::String::from_str(&ctx.env, "exploit");
    ctx.tipjar_client
        .pause_feature(&ctx.admin, &PauseScope::Tipping, &reason);

    let result = ctx.tipjar_client.try_tip(&sender, &creator, &ctx.token_1, &100);
    assert_error_contains(result, TipJarError::FeaturePaused);
}

#[test]
fn test_tip_split_blocked_when_tipping_scope_paused() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let c1 = ctx.create_creator();
    let c2 = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);
    let reason = soroban_sdk::String::from_str(&ctx.env, "exploit");
    ctx.tipjar_client
        .pause_feature(&ctx.admin, &PauseScope::Tipping, &reason);

    let mut recipients = soroban_sdk::Vec::new(&ctx.env);
    recipients.push_back(tipjar::TipRecipient {
        creator: c1,
        percentage: 5_000,
    });
    recipients.push_back(tipjar::TipRecipient {
        creator: c2,
        percentage: 5_000,
    });
    let result = ctx
        .tipjar_client
        .try_tip_split(&sender, &ctx.token_1, &recipients, &100);
    assert_error_contains(result, TipJarError::FeaturePaused);
}

#[test]
fn test_tip_works_after_tipping_scope_unpaused() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);
    let reason = soroban_sdk::String::from_str(&ctx.env, "test");
    ctx.tipjar_client
        .pause_feature(&ctx.admin, &PauseScope::Tipping, &reason);
    ctx.tipjar_client
        .unpause_feature(&ctx.admin, &PauseScope::Tipping);
    ctx.tipjar_client
        .tip(&sender, &creator, &ctx.token_1, &100);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 100);
}

// ── Withdrawals scope ─────────────────────────────────────────────────────────

#[test]
fn test_withdraw_blocked_when_withdrawals_scope_paused() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &100);

    let reason = soroban_sdk::String::from_str(&ctx.env, "audit");
    ctx.tipjar_client
        .pause_feature(&ctx.admin, &PauseScope::Withdrawals, &reason);

    let result = ctx.tipjar_client.try_withdraw(&creator, &ctx.token_1, &None);
    assert_error_contains(result, TipJarError::FeaturePaused);
}

#[test]
fn test_tip_still_works_when_only_withdrawals_paused() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);

    let reason = soroban_sdk::String::from_str(&ctx.env, "audit");
    ctx.tipjar_client
        .pause_feature(&ctx.admin, &PauseScope::Withdrawals, &reason);

    // Tips must still go through
    ctx.tipjar_client
        .tip(&sender, &creator, &ctx.token_1, &200);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 200);
}

// ── Subscriptions scope ───────────────────────────────────────────────────────

#[test]
fn test_create_subscription_blocked_when_subscriptions_scope_paused() {
    let ctx = TestContext::new();
    let subscriber = ctx.create_user();
    let creator = ctx.create_creator();
    let reason = soroban_sdk::String::from_str(&ctx.env, "maintenance");
    ctx.tipjar_client
        .pause_feature(&ctx.admin, &PauseScope::Subscriptions, &reason);

    let result = ctx.tipjar_client.try_create_subscription(
        &subscriber,
        &creator,
        &ctx.token_1,
        &100,
        &86_400,
    );
    assert_error_contains(result, TipJarError::FeaturePaused);
}

// ── global pause still blocks everything ─────────────────────────────────────

#[test]
fn test_global_pause_blocks_even_when_no_feature_paused() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);

    let reason = soroban_sdk::String::from_str(&ctx.env, "emergency");
    ctx.tipjar_client.pause(&ctx.admin, &reason);

    // Feature flags are clear, but global pause should still block
    assert!(!ctx.tipjar_client.is_feature_paused(&PauseScope::Tipping));
    let result = ctx.tipjar_client.try_tip(&sender, &creator, &ctx.token_1, &100);
    assert_error_contains(result, TipJarError::ContractPaused);
}
