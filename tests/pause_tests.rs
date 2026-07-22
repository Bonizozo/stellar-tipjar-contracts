//! Circuit-breaker mechanics: pause/unpause, guardian/admin asymmetry,
//! auto-expiry, pause-check ordering, and event emission.
//!
//! Per-(flag, entrypoint) coverage and the independent-flags acceptance
//! criteria live in `partial_pause_tests.rs`.

mod common;

use common::{expect_error, MaliciousToken, MaliciousTokenClient, TestContext};
use soroban_sdk::{symbol_short, testutils::Events as _, vec, IntoVal};
use tipjar::{Error, PAUSE_FLAG_ALL, PAUSE_FLAG_TIPS, PAUSE_FLAG_WITHDRAWALS};

// ── defaults ─────────────────────────────────────────────────────────────

#[test]
fn nothing_paused_by_default() {
    let ctx = TestContext::new();
    assert!(!ctx.client().is_feature_paused(&PAUSE_FLAG_TIPS));
    assert!(!ctx.client().is_feature_paused(&PAUSE_FLAG_WITHDRAWALS));
    assert_eq!(ctx.client().pause_flags(), 0);
}

// ── admin pause/unpause: persistent, no expiry ──────────────────────────

#[test]
fn admin_pause_tips_persists_and_unpause_clears_it() {
    let ctx = TestContext::new();
    ctx.client().pause_tips(&ctx.admin);
    assert!(ctx.client().is_feature_paused(&PAUSE_FLAG_TIPS));

    // Persistent: does not expire even far in the future.
    ctx.advance_ledger(10_000_000);
    assert!(ctx.client().is_feature_paused(&PAUSE_FLAG_TIPS));

    ctx.client().unpause_tips(&ctx.admin);
    assert!(!ctx.client().is_feature_paused(&PAUSE_FLAG_TIPS));
}

#[test]
fn admin_pause_withdrawals_persists_and_unpause_clears_it() {
    let ctx = TestContext::new();
    ctx.client().pause_withdrawals(&ctx.admin);
    assert!(ctx.client().is_feature_paused(&PAUSE_FLAG_WITHDRAWALS));

    ctx.advance_ledger(10_000_000);
    assert!(ctx.client().is_feature_paused(&PAUSE_FLAG_WITHDRAWALS));

    ctx.client().unpause_withdrawals(&ctx.admin);
    assert!(!ctx.client().is_feature_paused(&PAUSE_FLAG_WITHDRAWALS));
}

#[test]
fn admin_pause_all_sets_both_flags_and_unpause_all_clears_both() {
    let ctx = TestContext::new();
    ctx.client().pause_all(&ctx.admin);
    assert!(ctx.client().is_feature_paused(&PAUSE_FLAG_TIPS));
    assert!(ctx.client().is_feature_paused(&PAUSE_FLAG_WITHDRAWALS));
    assert_eq!(ctx.client().pause_flags(), PAUSE_FLAG_ALL);

    ctx.client().unpause_all(&ctx.admin);
    assert_eq!(ctx.client().pause_flags(), 0);
}

// ── guardian: can pause instantly, can never unpause ────────────────────

#[test]
fn guardian_can_pause_each_flag_instantly() {
    for flag in [PAUSE_FLAG_TIPS, PAUSE_FLAG_WITHDRAWALS, PAUSE_FLAG_ALL] {
        let ctx = TestContext::new();
        match flag {
            f if f == PAUSE_FLAG_TIPS => ctx.client().pause_tips(&ctx.guardian),
            f if f == PAUSE_FLAG_WITHDRAWALS => ctx.client().pause_withdrawals(&ctx.guardian),
            _ => ctx.client().pause_all(&ctx.guardian),
        };
        assert_eq!(
            ctx.client().pause_flags(),
            flag,
            "guardian pause of flag {flag} took effect immediately"
        );
    }
}

#[test]
fn guardian_cannot_unpause_tips() {
    let ctx = TestContext::new();
    ctx.client().pause_tips(&ctx.guardian);
    let err = expect_error(ctx.client().try_unpause_tips(&ctx.guardian));
    assert_eq!(err, Error::Unauthorized.into());
    // Still paused: the unauthorized call had no effect.
    assert!(ctx.client().is_feature_paused(&PAUSE_FLAG_TIPS));
}

#[test]
fn guardian_cannot_unpause_withdrawals() {
    let ctx = TestContext::new();
    ctx.client().pause_withdrawals(&ctx.guardian);
    let err = expect_error(ctx.client().try_unpause_withdrawals(&ctx.guardian));
    assert_eq!(err, Error::Unauthorized.into());
    assert!(ctx.client().is_feature_paused(&PAUSE_FLAG_WITHDRAWALS));
}

#[test]
fn guardian_cannot_unpause_all() {
    let ctx = TestContext::new();
    ctx.client().pause_all(&ctx.guardian);
    let err = expect_error(ctx.client().try_unpause_all(&ctx.guardian));
    assert_eq!(err, Error::Unauthorized.into());
    assert_eq!(ctx.client().pause_flags(), PAUSE_FLAG_ALL);
}

#[test]
fn admin_can_unpause_a_guardian_initiated_pause() {
    let ctx = TestContext::new();
    ctx.client().pause_tips(&ctx.guardian);
    assert!(ctx.client().is_feature_paused(&PAUSE_FLAG_TIPS));

    ctx.client().unpause_tips(&ctx.admin);
    assert!(!ctx.client().is_feature_paused(&PAUSE_FLAG_TIPS));
}

// ── strangers can do neither ─────────────────────────────────────────────

#[test]
fn stranger_cannot_pause() {
    let ctx = TestContext::new();
    let stranger = ctx.create_user();
    let err = expect_error(ctx.client().try_pause_tips(&stranger));
    assert_eq!(err, Error::Unauthorized.into());
}

#[test]
fn stranger_cannot_unpause() {
    let ctx = TestContext::new();
    ctx.client().pause_tips(&ctx.admin);
    let stranger = ctx.create_user();
    let err = expect_error(ctx.client().try_unpause_tips(&stranger));
    assert_eq!(err, Error::Unauthorized.into());
}

// ── admin can only administer guardian on their own contract ────────────

#[test]
fn only_admin_can_set_guardian() {
    let ctx = TestContext::new();
    let stranger = ctx.create_user();
    let new_guardian = ctx.create_user();
    let err = expect_error(ctx.client().try_set_guardian(&stranger, &new_guardian));
    assert_eq!(err, Error::Unauthorized.into());
}

// ── auto-expiry: boundary tested exactly in ledger units ────────────────

#[test]
fn guardian_pause_auto_expires_at_the_boundary_ledger() {
    let ctx = TestContext::new();
    ctx.client().pause_tips(&ctx.guardian);

    let expiry = ctx.client().guardian_pause_expiry_ledger();
    assert!(expiry > ctx.ledger_sequence());

    // At expiry - 1: still paused.
    let target = expiry - 1;
    let delta = target - ctx.ledger_sequence();
    ctx.advance_ledger(delta);
    assert_eq!(ctx.ledger_sequence(), expiry - 1);
    assert!(
        ctx.client().is_feature_paused(&PAUSE_FLAG_TIPS),
        "must still be paused one ledger before expiry"
    );

    // At expiry: no longer paused.
    ctx.advance_ledger(1);
    assert_eq!(ctx.ledger_sequence(), expiry);
    assert!(
        !ctx.client().is_feature_paused(&PAUSE_FLAG_TIPS),
        "must have auto-expired exactly at the expiry ledger"
    );
}

#[test]
fn guardian_pause_auto_expiry_unblocks_tip_entrypoint() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, 1_000);

    ctx.client().pause_tips(&ctx.guardian);
    let expiry = ctx.client().guardian_pause_expiry_ledger();

    let err = expect_error(ctx.client().try_tip(&sender, &creator, &100));
    assert_eq!(err, Error::TipsPaused.into());

    let delta = expiry - ctx.ledger_sequence();
    ctx.advance_ledger(delta);

    // Now at the expiry ledger: tip must succeed, no admin action taken.
    ctx.client().tip(&sender, &creator, &100);
    assert_eq!(ctx.client().get_total_tips(&creator), 100);
}

#[test]
fn admin_confirming_a_guardian_pause_makes_it_persist_past_expiry() {
    let ctx = TestContext::new();
    ctx.client().pause_tips(&ctx.guardian);
    let expiry = ctx.client().guardian_pause_expiry_ledger();

    // Admin confirms it into a persistent pause before it would have expired.
    ctx.client().pause_tips(&ctx.admin);

    let delta = expiry - ctx.ledger_sequence();
    ctx.advance_ledger(delta);
    assert!(
        ctx.client().is_feature_paused(&PAUSE_FLAG_TIPS),
        "admin-confirmed pause must survive past the original guardian expiry"
    );

    // Only an explicit admin unpause clears it now.
    ctx.client().unpause_tips(&ctx.admin);
    assert!(!ctx.client().is_feature_paused(&PAUSE_FLAG_TIPS));
}

#[test]
fn configurable_guardian_pause_duration_is_respected() {
    let ctx = TestContext::new();
    ctx.client().set_guardian_pause_duration(&ctx.admin, &10);
    let before = ctx.ledger_sequence();
    ctx.client().pause_withdrawals(&ctx.guardian);
    assert_eq!(ctx.client().guardian_pause_expiry_ledger(), before + 10);
}

#[test]
fn setting_zero_guardian_pause_duration_is_rejected() {
    let ctx = TestContext::new();
    let err = expect_error(ctx.client().try_set_guardian_pause_duration(&ctx.admin, &0));
    assert_eq!(err, Error::InvalidDuration.into());
}

// ── pause-check ordering: no token movement even with a malicious token ─

#[test]
fn paused_tip_never_reaches_a_malicious_token_transfer() {
    let env = common::fresh_env();
    let malicious_token = env.register(MaliciousToken, ());
    let ctx = TestContext::with_token(env, malicious_token.clone());

    let sender = ctx.create_user();
    let creator = ctx.create_creator();

    ctx.client().pause_tips(&ctx.admin);

    let err = expect_error(ctx.client().try_tip(&sender, &creator, &100));
    assert_eq!(err, Error::TipsPaused.into());

    let malicious_client = MaliciousTokenClient::new(&ctx.env, &malicious_token);
    assert_eq!(
        malicious_client.transfer_calls(),
        0,
        "the pause check must short-circuit before the token transfer is ever attempted"
    );
}

#[test]
fn paused_withdraw_never_reaches_a_malicious_token_transfer() {
    let env = common::fresh_env();
    let malicious_token = env.register(MaliciousToken, ());
    let ctx = TestContext::with_token(env, malicious_token.clone());

    let creator = ctx.create_creator();

    ctx.client().pause_withdrawals(&ctx.admin);

    let err = expect_error(
        ctx.client()
            .try_withdraw(&creator, &creator, &creator, &None),
    );
    assert_eq!(err, Error::WithdrawalsPaused.into());

    let malicious_client = MaliciousTokenClient::new(&ctx.env, &malicious_token);
    assert_eq!(malicious_client.transfer_calls(), 0);
}

// ── events ────────────────────────────────────────────────────────────

#[test]
fn pause_emits_paused_event_with_flags_and_actor() {
    let ctx = TestContext::new();
    ctx.client().pause_tips(&ctx.admin);

    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (symbol_short!("paused"), ctx.admin.clone()).into_val(&ctx.env),
                (PAUSE_FLAG_TIPS,).into_val(&ctx.env),
            ),
        ]
    );
}

#[test]
fn guardian_pause_emits_paused_event_with_guardian_as_actor() {
    let ctx = TestContext::new();
    ctx.client().pause_withdrawals(&ctx.guardian);

    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (symbol_short!("paused"), ctx.guardian.clone()).into_val(&ctx.env),
                (PAUSE_FLAG_WITHDRAWALS,).into_val(&ctx.env),
            ),
        ]
    );
}

#[test]
fn unpause_emits_unpaused_event_with_flags_and_actor() {
    let ctx = TestContext::new();
    ctx.client().pause_all(&ctx.admin);
    ctx.client().unpause_all(&ctx.admin);

    // `events().all()` only reports events from the *last* invocation, so
    // only the `unpause_all` call's event is visible here.
    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (symbol_short!("unpaused"), ctx.admin.clone()).into_val(&ctx.env),
                (PAUSE_FLAG_ALL,).into_val(&ctx.env),
            ),
        ]
    );
}
