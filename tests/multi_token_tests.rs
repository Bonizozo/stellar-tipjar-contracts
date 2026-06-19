mod common;
use common::*;
use tipjar::TipJarError;

// ── whitelist management ──────────────────────────────────────────────────────

#[test]
fn test_add_token_whitelists_token() {
    let ctx = TestContext::new();
    assert!(!ctx.tipjar_client.is_whitelisted(&ctx.token_3));
    ctx.tipjar_client.add_token(&ctx.admin, &ctx.token_3);
    assert!(ctx.tipjar_client.is_whitelisted(&ctx.token_3));
}

#[test]
fn test_remove_token_removes_from_whitelist() {
    let ctx = TestContext::new();
    ctx.tipjar_client.add_token(&ctx.admin, &ctx.token_3);
    assert!(ctx.tipjar_client.is_whitelisted(&ctx.token_3));
    ctx.tipjar_client.remove_token(&ctx.admin, &ctx.token_3);
    assert!(!ctx.tipjar_client.is_whitelisted(&ctx.token_3));
}

#[test]
fn test_non_admin_cannot_add_token() {
    let ctx = TestContext::new();
    let user = ctx.create_user();
    let result = ctx.tipjar_client.try_add_token(&user, &ctx.token_3);
    assert_error_contains(result, TipJarError::Unauthorized);
}

#[test]
fn test_non_admin_cannot_remove_token() {
    let ctx = TestContext::new();
    let user = ctx.create_user();
    let result = ctx.tipjar_client.try_remove_token(&user, &ctx.token_1);
    assert_error_contains(result, TipJarError::Unauthorized);
}

#[test]
fn test_tip_with_non_whitelisted_token_fails() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_3, 1_000);
    let result = ctx.tipjar_client.try_tip(&sender, &creator, &ctx.token_3, &100);
    assert_error_contains(result, TipJarError::TokenNotWhitelisted);
}

#[test]
fn test_tip_after_remove_token_fails() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);

    // Works before removal
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &100);

    // Remove and verify it fails
    ctx.tipjar_client.remove_token(&ctx.admin, &ctx.token_1);
    let result = ctx.tipjar_client.try_tip(&sender, &creator, &ctx.token_1, &100);
    assert_error_contains(result, TipJarError::TokenNotWhitelisted);
}

// ── per-token balance tracking ────────────────────────────────────────────────

#[test]
fn test_balances_tracked_independently_per_token() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);
    ctx.mint_tokens(&sender, &ctx.token_2, 1_000);

    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &300);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_2, &500);

    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 300);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_2, 500);
    assert_total_tips_equals(&ctx, &creator, &ctx.token_1, 300);
    assert_total_tips_equals(&ctx, &creator, &ctx.token_2, 500);
}

#[test]
fn test_multiple_tips_accumulate_per_token() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);
    ctx.mint_tokens(&sender, &ctx.token_2, 1_000);

    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &100);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &200);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_2, &400);

    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 300);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_2, 400);
}

#[test]
fn test_different_creators_have_independent_balances() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let c1 = ctx.create_creator();
    let c2 = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);

    ctx.tipjar_client.tip(&sender, &c1, &ctx.token_1, &200);
    ctx.tipjar_client.tip(&sender, &c2, &ctx.token_1, &300);

    assert_withdrawable_balance_equals(&ctx, &c1, &ctx.token_1, 200);
    assert_withdrawable_balance_equals(&ctx, &c2, &ctx.token_1, 300);
}

// ── withdrawal per token ──────────────────────────────────────────────────────

#[test]
fn test_withdraw_specific_token() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);
    ctx.mint_tokens(&sender, &ctx.token_2, 1_000);

    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &300);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_2, &500);

    let before = ctx.get_token_balance(&creator, &ctx.token_1);
    ctx.tipjar_client.withdraw(&creator, &ctx.token_1, &None);

    assert_eq!(ctx.get_token_balance(&creator, &ctx.token_1), before + 300);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 0);
    // token_2 balance untouched
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_2, 500);
}

#[test]
fn test_withdraw_nothing_fails() {
    let ctx = TestContext::new();
    let creator = ctx.create_creator();
    let result = ctx.tipjar_client.try_withdraw(&creator, &ctx.token_1, &None);
    assert_error_contains(result, TipJarError::NothingToWithdraw);
}

#[test]
fn test_withdraw_each_token_independently() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);
    ctx.mint_tokens(&sender, &ctx.token_2, 1_000);

    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &400);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_2, &600);

    ctx.tipjar_client.withdraw(&creator, &ctx.token_1, &None);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 0);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_2, 600);

    ctx.tipjar_client.withdraw(&creator, &ctx.token_2, &None);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_2, 0);
}

// ── creator token list ────────────────────────────────────────────────────────

#[test]
fn test_get_creator_tokens_empty_before_any_tip() {
    let ctx = TestContext::new();
    let creator = ctx.create_creator();
    let tokens = ctx.tipjar_client.get_creator_tokens(&creator);
    assert_eq!(tokens.len(), 0);
}

#[test]
fn test_get_creator_tokens_tracks_tipped_tokens() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);
    ctx.mint_tokens(&sender, &ctx.token_2, 1_000);

    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &100);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_2, &200);

    let tokens = ctx.tipjar_client.get_creator_tokens(&creator);
    assert_eq!(tokens.len(), 2);
    assert!(tokens.contains(&ctx.token_1));
    assert!(tokens.contains(&ctx.token_2));
}

#[test]
fn test_get_creator_tokens_no_duplicates() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);

    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &100);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &200);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &300);

    let tokens = ctx.tipjar_client.get_creator_tokens(&creator);
    assert_eq!(tokens.len(), 1);
}

#[test]
fn test_creator_tokens_are_independent_per_creator() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let c1 = ctx.create_creator();
    let c2 = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);
    ctx.mint_tokens(&sender, &ctx.token_2, 1_000);

    ctx.tipjar_client.tip(&sender, &c1, &ctx.token_1, &100);
    ctx.tipjar_client.tip(&sender, &c2, &ctx.token_2, &200);

    let t1 = ctx.tipjar_client.get_creator_tokens(&c1);
    let t2 = ctx.tipjar_client.get_creator_tokens(&c2);

    assert_eq!(t1.len(), 1);
    assert!(t1.contains(&ctx.token_1));
    assert_eq!(t2.len(), 1);
    assert!(t2.contains(&ctx.token_2));
}

// ── re-whitelist after removal ────────────────────────────────────────────────

#[test]
fn test_re_add_removed_token_allows_tipping_again() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_3, 1_000);

    ctx.tipjar_client.add_token(&ctx.admin, &ctx.token_3);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_3, &100);

    ctx.tipjar_client.remove_token(&ctx.admin, &ctx.token_3);
    let result = ctx.tipjar_client.try_tip(&sender, &creator, &ctx.token_3, &100);
    assert_error_contains(result, TipJarError::TokenNotWhitelisted);

    ctx.tipjar_client.add_token(&ctx.admin, &ctx.token_3);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_3, &200);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_3, 300);
}
