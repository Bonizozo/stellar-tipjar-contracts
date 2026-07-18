mod common;
use common::*;
use tipjar::{Subscription, SubscriptionStatus, TipJarError};

const ONE_DAY: u64 = 86_400;
const ONE_MONTH: u64 = 2_592_000;

// ── helpers ──────────────────────────────────────────────────────────────────

fn setup_subscription(ctx: &TestContext) -> (soroban_sdk::Address, soroban_sdk::Address) {
    let subscriber = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&subscriber, &ctx.token_1, 10_000);
    ctx.tipjar_client.create_subscription(
        &subscriber,
        &creator,
        &ctx.token_1,
        &100,
        &ONE_MONTH,
    );
    (subscriber, creator)
}

// ── creation ─────────────────────────────────────────────────────────────────

#[test]
fn test_create_subscription_stores_correct_data() {
    let ctx = TestContext::new();
    let (subscriber, creator) = setup_subscription(&ctx);

    let sub: Subscription = ctx
        .tipjar_client
        .get_subscription(&subscriber, &creator)
        .unwrap();

    assert_eq!(sub.amount, 100);
    assert_eq!(sub.interval_seconds, ONE_MONTH);
    assert_eq!(sub.status, SubscriptionStatus::Active);
    assert_eq!(sub.last_payment, 0);
    assert_eq!(sub.next_payment, ctx.get_current_time());
}

#[test]
fn test_create_subscription_rejects_zero_amount() {
    let ctx = TestContext::new();
    let subscriber = ctx.create_user();
    let creator = ctx.create_creator();

    let result = ctx.tipjar_client.try_create_subscription(
        &subscriber,
        &creator,
        &ctx.token_1,
        &0,
        &ONE_MONTH,
    );
    assert_error_contains(result, TipJarError::InvalidAmount);
}

#[test]
fn test_create_subscription_rejects_short_interval() {
    let ctx = TestContext::new();
    let subscriber = ctx.create_user();
    let creator = ctx.create_creator();

    let result = ctx.tipjar_client.try_create_subscription(
        &subscriber,
        &creator,
        &ctx.token_1,
        &100,
        &(ONE_DAY - 1),
    );
    assert_error_contains(result, TipJarError::InvalidInterval);
}

// ── payment execution ─────────────────────────────────────────────────────────

#[test]
fn test_execute_payment_transfers_tokens_and_updates_balances() {
    let ctx = TestContext::new();
    let (subscriber, creator) = setup_subscription(&ctx);

    ctx.tipjar_client
        .execute_subscription_payment(&subscriber, &creator);

    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 100);
    assert_total_tips_equals(&ctx, &creator, &ctx.token_1, 100);
    assert_eq!(ctx.get_token_balance(&subscriber, &ctx.token_1), 9_900);
}

#[test]
fn test_execute_payment_advances_next_payment() {
    let ctx = TestContext::new();
    let (subscriber, creator) = setup_subscription(&ctx);

    let before = ctx.get_current_time();
    ctx.tipjar_client
        .execute_subscription_payment(&subscriber, &creator);

    let sub: Subscription = ctx
        .tipjar_client
        .get_subscription(&subscriber, &creator)
        .unwrap();
    assert_eq!(sub.last_payment, before);
    assert_eq!(sub.next_payment, before + ONE_MONTH);
}

#[test]
fn test_execute_payment_fails_before_interval_elapses() {
    let ctx = TestContext::new();
    let (subscriber, creator) = setup_subscription(&ctx);

    ctx.tipjar_client
        .execute_subscription_payment(&subscriber, &creator);

    // Advance less than the full interval
    ctx.advance_time(ONE_MONTH - 1);

    let result = ctx
        .tipjar_client
        .try_execute_subscription_payment(&subscriber, &creator);
    assert_error_contains(result, TipJarError::PaymentNotDue);
}

#[test]
fn test_execute_payment_succeeds_after_interval_elapses() {
    let ctx = TestContext::new();
    let (subscriber, creator) = setup_subscription(&ctx);

    ctx.tipjar_client
        .execute_subscription_payment(&subscriber, &creator);
    ctx.advance_time(ONE_MONTH);
    ctx.tipjar_client
        .execute_subscription_payment(&subscriber, &creator);

    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 200);
}

#[test]
fn test_execute_payment_fails_for_nonexistent_subscription() {
    let ctx = TestContext::new();
    let subscriber = ctx.create_user();
    let creator = ctx.create_creator();

    let result = ctx
        .tipjar_client
        .try_execute_subscription_payment(&subscriber, &creator);
    assert_error_contains(result, TipJarError::SubscriptionNotFound);
}

// ── pause / resume ────────────────────────────────────────────────────────────

#[test]
fn test_pause_prevents_payment_execution() {
    let ctx = TestContext::new();
    let (subscriber, creator) = setup_subscription(&ctx);

    ctx.tipjar_client.pause_subscription(&subscriber, &creator);

    let result = ctx
        .tipjar_client
        .try_execute_subscription_payment(&subscriber, &creator);
    assert_error_contains(result, TipJarError::SubscriptionNotActive);
}

#[test]
fn test_resume_allows_payment_execution() {
    let ctx = TestContext::new();
    let (subscriber, creator) = setup_subscription(&ctx);

    ctx.tipjar_client.pause_subscription(&subscriber, &creator);
    ctx.tipjar_client.resume_subscription(&subscriber, &creator);

    // Should succeed immediately after resume
    ctx.tipjar_client
        .execute_subscription_payment(&subscriber, &creator);
    assert_withdrawable_balance_equals(&ctx, &creator, &ctx.token_1, 100);
}

#[test]
fn test_pause_already_paused_fails() {
    let ctx = TestContext::new();
    let (subscriber, creator) = setup_subscription(&ctx);

    ctx.tipjar_client.pause_subscription(&subscriber, &creator);
    let result = ctx
        .tipjar_client
        .try_pause_subscription(&subscriber, &creator);
    assert_error_contains(result, TipJarError::SubscriptionNotActive);
}

// ── cancellation ──────────────────────────────────────────────────────────────

#[test]
fn test_cancel_subscription_sets_cancelled_status() {
    let ctx = TestContext::new();
    let (subscriber, creator) = setup_subscription(&ctx);

    ctx.tipjar_client
        .cancel_subscription(&subscriber, &creator);

    let sub: Subscription = ctx
        .tipjar_client
        .get_subscription(&subscriber, &creator)
        .unwrap();
    assert_eq!(sub.status, SubscriptionStatus::Cancelled);
}

#[test]
fn test_cancelled_subscription_cannot_be_paid() {
    let ctx = TestContext::new();
    let (subscriber, creator) = setup_subscription(&ctx);

    ctx.tipjar_client
        .cancel_subscription(&subscriber, &creator);

    let result = ctx
        .tipjar_client
        .try_execute_subscription_payment(&subscriber, &creator);
    assert_error_contains(result, TipJarError::SubscriptionNotActive);
}

// ── query ─────────────────────────────────────────────────────────────────────

#[test]
fn test_get_subscription_returns_none_when_missing() {
    let ctx = TestContext::new();
    let subscriber = ctx.create_user();
    let creator = ctx.create_creator();

    let result = ctx
        .tipjar_client
        .get_subscription(&subscriber, &creator);
    assert!(result.is_none());
}
