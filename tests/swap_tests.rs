mod common;
use common::*;
use soroban_sdk::{Bytes, BytesN};
use tipjar::{swap::AtomicSwap, swap::SwapStatus, TipJarError};

const ONE_HOUR: u64 = 3_600;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Returns a 32-byte preimage and its SHA-256 hash lock.
fn make_hash_lock(ctx: &TestContext, secret: &[u8; 32]) -> (BytesN<32>, BytesN<32>) {
    let preimage = BytesN::from_array(&ctx.env, secret);
    let hash: BytesN<32> = ctx
        .env
        .crypto()
        .sha256(&Bytes::from(&preimage))
        .into();
    (preimage, hash)
}

fn setup_swap(ctx: &TestContext) -> (soroban_sdk::Address, soroban_sdk::Address, BytesN<32>, BytesN<32>, u64) {
    let initiator = ctx.create_user();
    let recipient = ctx.create_user();
    ctx.mint_tokens(&initiator, &ctx.token_1, 1_000);

    let (preimage, hash_lock) = make_hash_lock(ctx, b"super_secret_preimage_32bytes!!!");
    let time_lock = ctx.get_current_time() + ONE_HOUR;

    let id = ctx.tipjar_client.create_swap(
        &initiator,
        &recipient,
        &ctx.token_1,
        &500,
        &hash_lock,
        &time_lock,
    );
    assert_eq!(id, 1);
    (initiator, recipient, preimage, hash_lock, time_lock)
}

// ── create_swap ───────────────────────────────────────────────────────────────

#[test]
fn test_create_swap_escrows_tokens() {
    let ctx = TestContext::new();
    let (initiator, _recipient, _preimage, _hash_lock, _time_lock) = setup_swap(&ctx);

    // Initiator paid 500; contract holds it.
    assert_balance_equals(&ctx, &initiator, &ctx.token_1, 500);
    assert_balance_equals(&ctx, &ctx.contract_id, &ctx.token_1, 500);
}

#[test]
fn test_create_swap_stores_correct_data() {
    let ctx = TestContext::new();
    let (initiator, recipient, _preimage, hash_lock, time_lock) = setup_swap(&ctx);

    let swap: AtomicSwap = ctx.tipjar_client.get_swap(&1);
    assert_eq!(swap.id, 1);
    assert_eq!(swap.initiator, initiator);
    assert_eq!(swap.recipient, recipient);
    assert_eq!(swap.amount, 500);
    assert_eq!(swap.hash_lock, hash_lock);
    assert_eq!(swap.time_lock, time_lock);
    assert_eq!(swap.status, SwapStatus::Pending);
}

#[test]
fn test_create_swap_rejects_zero_amount() {
    let ctx = TestContext::new();
    let initiator = ctx.create_user();
    let recipient = ctx.create_user();
    let (_, hash_lock) = make_hash_lock(&ctx, b"super_secret_preimage_32bytes!!!");

    let result = ctx.tipjar_client.try_create_swap(
        &initiator,
        &recipient,
        &ctx.token_1,
        &0,
        &hash_lock,
        &(ctx.get_current_time() + ONE_HOUR),
    );
    assert_error_contains(result, TipJarError::InvalidAmount);
}

#[test]
fn test_create_swap_rejects_expired_time_lock() {
    let ctx = TestContext::new();
    let initiator = ctx.create_user();
    let recipient = ctx.create_user();
    let (_, hash_lock) = make_hash_lock(&ctx, b"super_secret_preimage_32bytes!!!");

    let result = ctx.tipjar_client.try_create_swap(
        &initiator,
        &recipient,
        &ctx.token_1,
        &100,
        &hash_lock,
        &ctx.get_current_time(), // not in the future
    );
    assert_error_contains(result, TipJarError::InvalidTimeLock);
}

// ── execute_swap ──────────────────────────────────────────────────────────────

#[test]
fn test_execute_swap_transfers_to_recipient() {
    let ctx = TestContext::new();
    let (_initiator, recipient, preimage, _hash_lock, _time_lock) = setup_swap(&ctx);

    ctx.tipjar_client.execute_swap(&1, &preimage);

    assert_balance_equals(&ctx, &recipient, &ctx.token_1, 500);
    assert_balance_equals(&ctx, &ctx.contract_id, &ctx.token_1, 0);

    let swap: AtomicSwap = ctx.tipjar_client.get_swap(&1);
    assert_eq!(swap.status, SwapStatus::Completed);
}

#[test]
fn test_execute_swap_rejects_wrong_preimage() {
    let ctx = TestContext::new();
    let (_initiator, _recipient, _preimage, _hash_lock, _time_lock) = setup_swap(&ctx);

    let wrong = BytesN::from_array(&ctx.env, b"wrong_preimage_wrong_preimage!!!");
    let result = ctx.tipjar_client.try_execute_swap(&1, &wrong);
    assert_error_contains(result, TipJarError::InvalidPreimage);
}

#[test]
fn test_execute_swap_rejects_already_completed() {
    let ctx = TestContext::new();
    let (_initiator, _recipient, preimage, _hash_lock, _time_lock) = setup_swap(&ctx);

    ctx.tipjar_client.execute_swap(&1, &preimage);

    let result = ctx.tipjar_client.try_execute_swap(&1, &preimage);
    assert_error_contains(result, TipJarError::SwapNotPending);
}

// ── refund_swap ───────────────────────────────────────────────────────────────

#[test]
fn test_refund_swap_returns_tokens_after_expiry() {
    let ctx = TestContext::new();
    let (initiator, _recipient, _preimage, _hash_lock, _time_lock) = setup_swap(&ctx);

    ctx.advance_time(ONE_HOUR + 1);
    ctx.tipjar_client.refund_swap(&1);

    assert_balance_equals(&ctx, &initiator, &ctx.token_1, 1_000);
    assert_balance_equals(&ctx, &ctx.contract_id, &ctx.token_1, 0);

    let swap: AtomicSwap = ctx.tipjar_client.get_swap(&1);
    assert_eq!(swap.status, SwapStatus::Refunded);
}

#[test]
fn test_refund_swap_rejects_before_expiry() {
    let ctx = TestContext::new();
    let (_initiator, _recipient, _preimage, _hash_lock, _time_lock) = setup_swap(&ctx);

    let result = ctx.tipjar_client.try_refund_swap(&1);
    assert_error_contains(result, TipJarError::TimeLockNotExpired);
}

#[test]
fn test_refund_swap_rejects_already_completed() {
    let ctx = TestContext::new();
    let (_initiator, _recipient, preimage, _hash_lock, _time_lock) = setup_swap(&ctx);

    ctx.tipjar_client.execute_swap(&1, &preimage);
    ctx.advance_time(ONE_HOUR + 1);

    let result = ctx.tipjar_client.try_refund_swap(&1);
    assert_error_contains(result, TipJarError::SwapNotPending);
}

// ── get_swap ──────────────────────────────────────────────────────────────────

#[test]
fn test_get_swap_panics_for_unknown_id() {
    let ctx = TestContext::new();
    let result = ctx.tipjar_client.try_get_swap(&99);
    assert!(result.is_err());
}

// ── counter increments ────────────────────────────────────────────────────────

#[test]
fn test_swap_ids_increment() {
    let ctx = TestContext::new();
    let initiator = ctx.create_user();
    let recipient = ctx.create_user();
    ctx.mint_tokens(&initiator, &ctx.token_1, 2_000);

    let (_, hash_lock) = make_hash_lock(&ctx, b"super_secret_preimage_32bytes!!!");
    let time_lock = ctx.get_current_time() + ONE_HOUR;

    let id1 = ctx.tipjar_client.create_swap(&initiator, &recipient, &ctx.token_1, &100, &hash_lock, &time_lock);
    let id2 = ctx.tipjar_client.create_swap(&initiator, &recipient, &ctx.token_1, &100, &hash_lock, &time_lock);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}
