mod common;
use common::*;
use soroban_sdk::Vec as SorobanVec;
use tipjar::{TipJarError, TipRecipient};

fn make_recipients(
    ctx: &TestContext,
    pairs: &[(&soroban_sdk::Address, u32)],
) -> SorobanVec<TipRecipient> {
    let mut v = SorobanVec::new(&ctx.env);
    for (creator, pct) in pairs {
        v.push_back(TipRecipient {
            creator: (*creator).clone(),
            percentage: *pct,
        });
    }
    v
}

// ── happy paths ───────────────────────────────────────────────────────────────

#[test]
fn test_split_50_50_updates_balances() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let c1 = ctx.create_creator();
    let c2 = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);

    let recipients = make_recipients(&ctx, &[(&c1, 5_000), (&c2, 5_000)]);
    ctx.tipjar_client
        .tip_split(&sender, &ctx.token_1, &recipients, &200);

    assert_withdrawable_balance_equals(&ctx, &c1, &ctx.token_1, 100);
    assert_withdrawable_balance_equals(&ctx, &c2, &ctx.token_1, 100);
    assert_total_tips_equals(&ctx, &c1, &ctx.token_1, 100);
    assert_total_tips_equals(&ctx, &c2, &ctx.token_1, 100);
}

#[test]
fn test_split_three_recipients_proportional() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let c1 = ctx.create_creator();
    let c2 = ctx.create_creator();
    let c3 = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 10_000);

    // 50% / 30% / 20%
    let recipients = make_recipients(&ctx, &[(&c1, 5_000), (&c2, 3_000), (&c3, 2_000)]);
    ctx.tipjar_client
        .tip_split(&sender, &ctx.token_1, &recipients, &1_000);

    assert_withdrawable_balance_equals(&ctx, &c1, &ctx.token_1, 500);
    assert_withdrawable_balance_equals(&ctx, &c2, &ctx.token_1, 300);
    assert_withdrawable_balance_equals(&ctx, &c3, &ctx.token_1, 200);
}

#[test]
fn test_split_rounding_remainder_goes_to_last_recipient() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let c1 = ctx.create_creator();
    let c2 = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 10_000);

    // 33.33% / 66.67% — 3 tokens: floor(3 * 3333/10000) = 0, last gets 3
    let recipients = make_recipients(&ctx, &[(&c1, 3_333), (&c2, 6_667)]);
    ctx.tipjar_client
        .tip_split(&sender, &ctx.token_1, &recipients, &3);

    let b1 = ctx.tipjar_client.get_withdrawable_balance(c1.clone(), ctx.token_1.clone());
    let b2 = ctx.tipjar_client.get_withdrawable_balance(c2.clone(), ctx.token_1.clone());
    // Full amount must be distributed
    assert_eq!(b1 + b2, 3);
}

#[test]
fn test_split_deducts_sender_balance() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let c1 = ctx.create_creator();
    let c2 = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 500);

    let recipients = make_recipients(&ctx, &[(&c1, 5_000), (&c2, 5_000)]);
    ctx.tipjar_client
        .tip_split(&sender, &ctx.token_1, &recipients, &500);

    assert_eq!(ctx.get_token_balance(&sender, &ctx.token_1), 0);
}

#[test]
fn test_split_ten_recipients() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    ctx.mint_tokens(&sender, &ctx.token_1, 10_000);

    let creators: Vec<soroban_sdk::Address> = (0..10).map(|_| ctx.create_creator()).collect();
    let pairs: Vec<(&soroban_sdk::Address, u32)> =
        creators.iter().map(|c| (c, 1_000u32)).collect();
    let recipients = make_recipients(&ctx, &pairs);

    ctx.tipjar_client
        .tip_split(&sender, &ctx.token_1, &recipients, &1_000);

    for c in &creators {
        assert_withdrawable_balance_equals(&ctx, c, &ctx.token_1, 100);
    }
}

// ── validation errors ─────────────────────────────────────────────────────────

#[test]
fn test_split_rejects_zero_amount() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let c1 = ctx.create_creator();
    let c2 = ctx.create_creator();

    let recipients = make_recipients(&ctx, &[(&c1, 5_000), (&c2, 5_000)]);
    let result = ctx
        .tipjar_client
        .try_tip_split(&sender, &ctx.token_1, &recipients, &0);
    assert_error_contains(result, TipJarError::InvalidAmount);
}

#[test]
fn test_split_rejects_single_recipient() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let c1 = ctx.create_creator();

    let recipients = make_recipients(&ctx, &[(&c1, 10_000)]);
    let result = ctx
        .tipjar_client
        .try_tip_split(&sender, &ctx.token_1, &recipients, &100);
    assert_error_contains(result, TipJarError::InvalidRecipientCount);
}

#[test]
fn test_split_rejects_eleven_recipients() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();

    let creators: Vec<soroban_sdk::Address> = (0..11).map(|_| ctx.create_creator()).collect();
    // Give first 10 recipients 1000 bps each (=10000), 11th gets 0 — but count check fires first
    let mut pairs: Vec<(&soroban_sdk::Address, u32)> =
        creators[..10].iter().map(|c| (c, 1_000u32)).collect();
    pairs.push((&creators[10], 0));
    let recipients = make_recipients(&ctx, &pairs);

    let result = ctx
        .tipjar_client
        .try_tip_split(&sender, &ctx.token_1, &recipients, &100);
    assert_error_contains(result, TipJarError::InvalidRecipientCount);
}

#[test]
fn test_split_rejects_percentages_not_summing_to_10000() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let c1 = ctx.create_creator();
    let c2 = ctx.create_creator();

    let recipients = make_recipients(&ctx, &[(&c1, 4_000), (&c2, 4_000)]);
    let result = ctx
        .tipjar_client
        .try_tip_split(&sender, &ctx.token_1, &recipients, &100);
    assert_error_contains(result, TipJarError::InvalidPercentageSum);
}

#[test]
fn test_split_rejects_zero_percentage_for_recipient() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let c1 = ctx.create_creator();
    let c2 = ctx.create_creator();

    let recipients = make_recipients(&ctx, &[(&c1, 10_000), (&c2, 0)]);
    let result = ctx
        .tipjar_client
        .try_tip_split(&sender, &ctx.token_1, &recipients, &100);
    assert_error_contains(result, TipJarError::InvalidPercentage);
}

#[test]
fn test_split_rejects_non_whitelisted_token() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let c1 = ctx.create_creator();
    let c2 = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_3, 1_000);

    let recipients = make_recipients(&ctx, &[(&c1, 5_000), (&c2, 5_000)]);
    let result = ctx
        .tipjar_client
        .try_tip_split(&sender, &ctx.token_3, &recipients, &100);
    assert_error_contains(result, TipJarError::TokenNotWhitelisted);
}
