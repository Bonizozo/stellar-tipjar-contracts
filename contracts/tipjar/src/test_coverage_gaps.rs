//! Coverage-gap tests for the TipJar contract.
//!
//! This file was written as part of a coverage-matrix audit (issue #411).
//! Each test below is explicitly labelled with the entrypoint it targets and
//! which column of the matrix it fills (happy / auth-fail /
//! precondition-fail).  Tests are grouped by entrypoint for easy scanning.
//!
//! Entrypoints newly covered here:
//!   • `get_tokens`              — happy path
//!   • `add_token`               — happy, auth-fail, precondition-fail ×2 (already-exists, max-limit)
//!   • `remove_token`            — happy, auth-fail, precondition-fail (not-found)
//!   • `cancel_upgrade`          — precondition-fail (no pending proposal)
//!   • `set_guardian`            — auth-fail (non-admin)
//!   • `set_guardian_pause_duration` — auth-fail (non-admin)
//!   • `get_guardian`            — happy path (absent then present)
//!   • `get_fee_collector`       — happy path (absent then present)
//!   • `tip_legacy`              — happy path, precondition-fail (invalid amount)
//!   • `get_total_tips_legacy`   — happy path (zero for unknown, sum after tips)
//!   • `withdraw_legacy`         — happy path, precondition-fail (nothing to withdraw)

#![cfg(test)]

use crate::{Error, TipJar, TipJarClient};
use soroban_sdk::{testutils::Address as _, token, Address, Env};

const TEST_TIMELOCK: u32 = 1_000;
const MAX_ALLOWED_TOKENS: u32 = 50;

// ─── shared test fixture ─────────────────────────────────────────────────────

struct Ctx {
    env: Env,
    contract_id: Address,
    token: Address,
    admin: Address,
}

impl Ctx {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let token_admin = Address::generate(&env);
        let token = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();
        let admin = Address::generate(&env);
        let contract_id = env.register(TipJar, ());
        let client = TipJarClient::new(&env, &contract_id);
        client.init(&token, &admin, &TEST_TIMELOCK);

        Ctx {
            env,
            contract_id,
            token,
            admin,
        }
    }

    fn client(&self) -> TipJarClient<'_> {
        TipJarClient::new(&self.env, &self.contract_id)
    }

    fn token_client(&self) -> token::TokenClient<'_> {
        token::TokenClient::new(&self.env, &self.token)
    }

    fn fund(&self, amount: i128) -> Address {
        let holder = Address::generate(&self.env);
        token::StellarAssetClient::new(&self.env, &self.token).mint(&holder, &amount);
        holder
    }

    fn new_token(&self) -> Address {
        let token_admin = Address::generate(&self.env);
        self.env
            .register_stellar_asset_contract_v2(token_admin)
            .address()
    }
}

// ─── get_tokens ──────────────────────────────────────────────────────────────

/// Happy path: `get_tokens` returns the initial single-element list containing
/// the token supplied to `init`, and grows correctly as tokens are added.
#[test]
fn get_tokens_returns_init_token_then_grows_with_add_token() {
    let ctx = Ctx::new();
    let client = ctx.client();

    // Immediately after init: one token.
    let tokens = client.get_tokens();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens.first().unwrap(), ctx.token);

    // After adding a second token the list grows by one.
    let token_b = ctx.new_token();
    client.add_token(&ctx.admin, &token_b);

    let tokens = client.get_tokens();
    assert_eq!(tokens.len(), 2);
    assert!(tokens.iter().any(|t| t == ctx.token));
    assert!(tokens.iter().any(|t| t == token_b));
}

// ─── add_token ───────────────────────────────────────────────────────────────

/// Happy path: admin can add a new token; it becomes tippable.
#[test]
fn add_token_happy_path_new_token_becomes_tippable() {
    let ctx = Ctx::new();
    let client = ctx.client();
    let token_b = ctx.new_token();

    client.add_token(&ctx.admin, &token_b);

    // Token is in the allowlist.
    let tokens = client.get_tokens();
    assert!(tokens.iter().any(|t| t == token_b));

    // A tip using token_b now succeeds.
    let tipper = Address::generate(&ctx.env);
    token::StellarAssetClient::new(&ctx.env, &token_b).mint(&tipper, &500);
    let creator = Address::generate(&ctx.env);
    client.tip(&tipper, &creator, &token_b, &100);
    assert_eq!(client.get_balance(&creator, &token_b), 100);
}

/// Auth-fail: a non-admin caller must be rejected with `Unauthorized`.
#[test]
fn add_token_by_non_admin_panics_with_unauthorized() {
    let ctx = Ctx::new();
    let stranger = Address::generate(&ctx.env);
    let token_b = ctx.new_token();

    let err = ctx
        .client()
        .try_add_token(&stranger, &token_b)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized.into());
}

/// Precondition-fail: adding a token that already exists must return
/// `TokenAlreadyExists`.
#[test]
fn add_token_duplicate_panics_with_token_already_exists() {
    let ctx = Ctx::new();

    // ctx.token is already in the allowlist from init.
    let err = ctx
        .client()
        .try_add_token(&ctx.admin, &ctx.token)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::TokenAlreadyExists.into());
}

/// Precondition-fail: adding a 51st token must return `MaxTokensReached`.
#[test]
fn add_token_beyond_cap_panics_with_max_tokens_reached() {
    let ctx = Ctx::new();
    let client = ctx.client();

    // init seeds 1 token; add MAX_ALLOWED_TOKENS-1 more to reach the cap.
    for _ in 0..(MAX_ALLOWED_TOKENS - 1) {
        let t = ctx.new_token();
        client.add_token(&ctx.admin, &t);
    }

    // The list is now at the cap; one more must fail.
    let extra = ctx.new_token();
    let err = client
        .try_add_token(&ctx.admin, &extra)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::MaxTokensReached.into());
}

// ─── remove_token ────────────────────────────────────────────────────────────

/// Happy path: admin removes a token; it disappears from the allowlist and
/// further tips with it are rejected, but existing escrowed balances remain
/// withdrawable.
#[test]
fn remove_token_happy_path_removes_from_allowlist_but_balance_still_withdrawable() {
    let ctx = Ctx::new();
    let client = ctx.client();
    let token_b = ctx.new_token();

    client.add_token(&ctx.admin, &token_b);

    // Build some escrowed balance using token_b before removing it.
    let tipper = Address::generate(&ctx.env);
    token::StellarAssetClient::new(&ctx.env, &token_b).mint(&tipper, &1_000);
    let creator = Address::generate(&ctx.env);
    client.tip(&tipper, &creator, &token_b, &200);
    assert_eq!(client.get_balance(&creator, &token_b), 200);

    // Remove token_b.
    client.remove_token(&ctx.admin, &token_b);

    // No longer in the list.
    let tokens = client.get_tokens();
    assert!(!tokens.iter().any(|t| t == token_b));

    // Tips now rejected.
    let err = client
        .try_tip(&tipper, &creator, &token_b, &50)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::TokenNotAllowed.into());

    // Existing balance is still withdrawable (withdraw does NOT check the
    // allowlist — that is by design; see lib.rs remove_token doc comment).
    let token_b_client = token::TokenClient::new(&ctx.env, &token_b);
    let before = token_b_client.balance(&creator);
    client.withdraw(&creator, &creator, &token_b, &creator, &None);
    assert_eq!(token_b_client.balance(&creator) - before, 200);
    assert_eq!(client.get_balance(&creator, &token_b), 0);
}

/// Auth-fail: a non-admin caller must be rejected with `Unauthorized`.
#[test]
fn remove_token_by_non_admin_panics_with_unauthorized() {
    let ctx = Ctx::new();
    let stranger = Address::generate(&ctx.env);

    let err = ctx
        .client()
        .try_remove_token(&stranger, &ctx.token)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized.into());
}

/// Precondition-fail: removing a token that was never added must return
/// `TokenNotAllowed`.
#[test]
fn remove_token_not_in_list_panics_with_token_not_allowed() {
    let ctx = Ctx::new();
    let absent = ctx.new_token();

    let err = ctx
        .client()
        .try_remove_token(&ctx.admin, &absent)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::TokenNotAllowed.into());
}

// ─── cancel_upgrade ──────────────────────────────────────────────────────────

/// Precondition-fail: calling `cancel_upgrade` when no proposal is pending
/// must return `NoPendingUpgrade`.
#[test]
fn cancel_upgrade_with_no_pending_proposal_panics_with_no_pending_upgrade() {
    let ctx = Ctx::new();

    let err = ctx
        .client()
        .try_cancel_upgrade(&ctx.admin)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NoPendingUpgrade.into());
}

// ─── set_guardian ────────────────────────────────────────────────────────────

/// Auth-fail: a non-admin caller must be rejected with `Unauthorized`.
#[test]
fn set_guardian_by_non_admin_panics_with_unauthorized() {
    let ctx = Ctx::new();
    let stranger = Address::generate(&ctx.env);
    let guardian = Address::generate(&ctx.env);

    let err = ctx
        .client()
        .try_set_guardian(&stranger, &guardian)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized.into());
}

// ─── set_guardian_pause_duration ─────────────────────────────────────────────

/// Auth-fail: a non-admin caller must be rejected with `Unauthorized`.
#[test]
fn set_guardian_pause_duration_by_non_admin_panics_with_unauthorized() {
    let ctx = Ctx::new();
    let stranger = Address::generate(&ctx.env);

    let err = ctx
        .client()
        .try_set_guardian_pause_duration(&stranger, &10_000)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized.into());
}

// ─── get_guardian ────────────────────────────────────────────────────────────

/// Happy path: `get_guardian` returns `None` when no guardian has been set and
/// `Some(guardian)` once `set_guardian` is called.
#[test]
fn get_guardian_returns_none_before_set_and_address_after() {
    let ctx = Ctx::new();
    let client = ctx.client();

    // No guardian yet.
    assert_eq!(client.get_guardian(), None);

    let guardian = Address::generate(&ctx.env);
    client.set_guardian(&ctx.admin, &guardian);

    assert_eq!(client.get_guardian(), Some(guardian));
}

// ─── get_fee_collector ───────────────────────────────────────────────────────

/// Happy path: `get_fee_collector` returns `None` before `set_fee` is called
/// and `Some(collector)` afterwards.
#[test]
fn get_fee_collector_returns_none_before_set_fee_and_address_after() {
    let ctx = Ctx::new();
    let client = ctx.client();

    // No collector configured yet.
    assert_eq!(client.get_fee_collector(), None);

    let collector = Address::generate(&ctx.env);
    client.set_fee(&ctx.admin, &100, &collector);

    assert_eq!(client.get_fee_collector(), Some(collector));
}

// ─── legacy entrypoints ──────────────────────────────────────────────────────
//
// `tip_legacy` calls `sender.require_auth()` at the outer frame and then
// internally calls `Self::tip`, which calls `sender.require_auth()` again.
// In the Soroban test environment `mock_all_auths()` only allows root-level
// auth checks by default; the nested (non-root) re-check requires
// `mock_all_auths_allowing_non_root_auth()`.  `withdraw_legacy` delegates to
// `Self::withdraw` which calls `caller.require_auth()`, but `withdraw_legacy`
// itself does NOT call `require_auth` first, so that wrapper does not have the
// double-auth issue — it can use a standard `Ctx`.  We split the fixtures to
// keep the non-root mock isolated to the tests that genuinely need it.

/// A fixture that calls `mock_all_auths_allowing_non_root_auth()` so that
/// `tip_legacy`'s nested `require_auth` inside `Self::tip` is accepted.
struct LegacyCtx {
    env: Env,
    contract_id: Address,
    token: Address,
    admin: Address,
}

impl LegacyCtx {
    fn new() -> Self {
        let env = Env::default();
        // Allow the nested require_auth that tip_legacy triggers via Self::tip.
        env.mock_all_auths_allowing_non_root_auth();

        let token_admin = Address::generate(&env);
        let token = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();
        let admin = Address::generate(&env);
        let contract_id = env.register(TipJar, ());
        let client = TipJarClient::new(&env, &contract_id);
        client.init(&token, &admin, &TEST_TIMELOCK);

        LegacyCtx {
            env,
            contract_id,
            token,
            admin,
        }
    }

    fn client(&self) -> TipJarClient<'_> {
        TipJarClient::new(&self.env, &self.contract_id)
    }

    fn token_client(&self) -> token::TokenClient<'_> {
        token::TokenClient::new(&self.env, &self.token)
    }

    fn fund(&self, amount: i128) -> Address {
        let holder = Address::generate(&self.env);
        token::StellarAssetClient::new(&self.env, &self.token).mint(&holder, &amount);
        holder
    }
}

// ─── tip_legacy ──────────────────────────────────────────────────────────────

/// Happy path: `tip_legacy` escrows tokens and updates balance / total via the
/// primary token (first entry in the allowlist, i.e. the token from `init`).
#[test]
fn tip_legacy_happy_path_uses_primary_token() {
    let ctx = LegacyCtx::new();
    let client = ctx.client();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    client.tip_legacy(&sender, &creator, &300);

    // Tokens left sender, landed in contract.
    assert_eq!(ctx.token_client().balance(&sender), 700);
    assert_eq!(ctx.token_client().balance(&ctx.contract_id), 300);

    // Both the multi-token read and legacy read agree.
    assert_eq!(client.get_total_tips(&creator, &ctx.token), 300);
    assert_eq!(client.get_total_tips_legacy(&creator), 300);
    assert_eq!(client.get_balance(&creator, &ctx.token), 300);
}

/// Precondition-fail: `tip_legacy` with zero or negative amount returns
/// `InvalidAmount`, consistent with the canonical `tip` entrypoint.
#[test]
fn tip_legacy_rejects_zero_and_negative_amounts() {
    // Zero/negative amounts are rejected before the token transfer, so the
    // nested require_auth in Self::tip is never reached.  The standard
    // mock_all_auths() in Ctx is therefore sufficient here.
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    for bad in [0i128, -1i128] {
        let err = ctx
            .client()
            .try_tip_legacy(&sender, &creator, &bad)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, Error::InvalidAmount.into());
    }

    // No tokens moved.
    assert_eq!(ctx.token_client().balance(&sender), 1_000);
    assert_eq!(ctx.client().get_total_tips_legacy(&creator), 0);
}

// ─── get_total_tips_legacy ───────────────────────────────────────────────────

/// Happy path: returns 0 for an unknown creator, then the running sum of all
/// tips placed through the legacy entrypoint.
#[test]
fn get_total_tips_legacy_is_zero_for_unknown_then_accumulates() {
    let ctx = LegacyCtx::new();
    let client = ctx.client();
    let creator = Address::generate(&ctx.env);

    assert_eq!(client.get_total_tips_legacy(&creator), 0);

    let sender = ctx.fund(1_000);
    client.tip_legacy(&sender, &creator, &100);
    client.tip_legacy(&sender, &creator, &250);

    assert_eq!(client.get_total_tips_legacy(&creator), 350);
    // Cross-check with canonical multi-token getter.
    assert_eq!(client.get_total_tips(&creator, &ctx.token), 350);
}

/// Happy path: totals from `tip` and `tip_legacy` accumulate into the same
/// per-(creator,token) bucket, because `tip_legacy` is just a thin wrapper
/// that resolves to the primary token.
#[test]
fn tip_and_tip_legacy_accumulate_into_the_same_bucket() {
    let ctx = LegacyCtx::new();
    let client = ctx.client();
    let sender = ctx.fund(2_000);
    let creator = Address::generate(&ctx.env);

    // Mix canonical tip and legacy tip for the same creator.
    client.tip(&sender, &creator, &ctx.token, &400);
    client.tip_legacy(&sender, &creator, &600);

    // Both readers see the combined total.
    assert_eq!(client.get_total_tips(&creator, &ctx.token), 1_000);
    assert_eq!(client.get_total_tips_legacy(&creator), 1_000);
    assert_eq!(client.get_balance(&creator, &ctx.token), 1_000);
}

// ─── withdraw_legacy ─────────────────────────────────────────────────────────

/// Happy path: `withdraw_legacy` pays out the escrowed balance in the primary
/// token and reduces the withdrawable balance to zero.
/// Uses `LegacyCtx` to fund via `tip_legacy`.
#[test]
fn withdraw_legacy_happy_path_pays_out_primary_token_balance() {
    let ctx = LegacyCtx::new();
    let client = ctx.client();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    client.tip_legacy(&sender, &creator, &500);

    // withdraw_legacy itself does not double-require_auth; standard mock is fine.
    client.withdraw_legacy(&creator, &creator, &creator, &None);

    assert_eq!(ctx.token_client().balance(&creator), 500);
    assert_eq!(client.get_balance(&creator, &ctx.token), 0);
    // Historical total is preserved after withdrawal.
    assert_eq!(client.get_total_tips_legacy(&creator), 500);
}

/// Precondition-fail: `withdraw_legacy` when the balance is zero must return
/// `NothingToWithdraw`.
#[test]
fn withdraw_legacy_with_no_balance_returns_nothing_to_withdraw() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);

    let err = ctx
        .client()
        .try_withdraw_legacy(&creator, &creator, &creator, &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NothingToWithdraw.into());
}

/// Happy path: `withdraw_legacy` with an explicit partial amount works and
/// leaves the remainder intact.
#[test]
fn withdraw_legacy_partial_amount_leaves_remainder() {
    let ctx = LegacyCtx::new();
    let client = ctx.client();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    client.tip_legacy(&sender, &creator, &800);

    client.withdraw_legacy(&creator, &creator, &creator, &Some(300));
    assert_eq!(ctx.token_client().balance(&creator), 300);
    assert_eq!(client.get_balance(&creator, &ctx.token), 500);

    // Withdraw the rest.
    client.withdraw_legacy(&creator, &creator, &creator, &None);
    assert_eq!(ctx.token_client().balance(&creator), 800);
    assert_eq!(client.get_balance(&creator, &ctx.token), 0);
}
