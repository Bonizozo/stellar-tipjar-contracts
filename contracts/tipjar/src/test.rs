//! Unit tests for the TipJar contract.
//!
//! Lives at `src/test.rs` (wired in via `#[cfg(test)] mod test;` in `lib.rs`)
//! rather than a top-level `tests/` integration crate, since these tests only
//! need access to the contract's own client/types and exercising them as a
//! unit-test module avoids a second crate compilation unit for such a small
//! contract.

use crate::{DataKey, Error, TipJar, TipJarClient};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events as _},
    token, vec, Address, Env, IntoVal, Symbol,
};

/// Timelock used by unit tests that don't specifically exercise the upgrade
/// flow's timing; small enough to jump past with a couple of ledger bumps.
const TEST_UPGRADE_TIMELOCK: u32 = 1000;

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
        client.init(&token, &admin, &TEST_UPGRADE_TIMELOCK);

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

    // Helper method to get the default token for backward compatibility
    fn get_token(&self) -> Address {
        self.token.clone()
    }
}

#[test]
fn tip_escrows_tokens_and_updates_balance_and_total() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    ctx.client().tip(&sender, &creator, &ctx.get_token(), &400);

    // Tokens left the sender and landed in the contract's escrow.
    assert_eq!(ctx.token_client().balance(&sender), 600);
    assert_eq!(ctx.token_client().balance(&ctx.contract_id), 400);

    // Historical total rose by the tipped amount.
    assert_eq!(ctx.client().get_total_tips(&creator, &ctx.get_token()), 400);

    // Withdrawable balance rose by the same amount: with a single creator and
    // a single tip, the full escrowed amount must be exactly what withdraw()
    // pays out.
    ctx.client()
        .withdraw(&creator, &creator, &ctx.get_token(), &creator, &None);
    assert_eq!(ctx.token_client().balance(&creator), 400);
}

#[test]
fn multiple_tips_accumulate_for_the_same_creator() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    ctx.client().tip(&sender, &creator, &ctx.get_token(), &100);
    ctx.client().tip(&sender, &creator, &ctx.get_token(), &200);
    ctx.client().tip(&sender, &creator, &ctx.get_token(), &300);

    assert_eq!(ctx.client().get_total_tips(&creator, &ctx.get_token()), 600);
    assert_eq!(ctx.token_client().balance(&ctx.contract_id), 600);

    ctx.client()
        .withdraw(&creator, &creator, &ctx.get_token(), &creator, &None);
    assert_eq!(ctx.token_client().balance(&creator), 600);
    // Historical total survives the withdrawal.
    assert_eq!(ctx.client().get_total_tips(&creator, &ctx.get_token()), 600);
}

#[test]
fn get_total_tips_is_zero_for_unknown_creator_then_tracks_sum() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);

    assert_eq!(ctx.client().get_total_tips(&creator, &ctx.get_token()), 0);

    let sender = ctx.fund(500);
    ctx.client().tip(&sender, &creator, &ctx.get_token(), &150);
    ctx.client().tip(&sender, &creator, &ctx.get_token(), &50);

    assert_eq!(ctx.client().get_total_tips(&creator, &ctx.get_token()), 200);
}

#[test]
fn withdraw_pays_out_full_balance_resets_it_and_keeps_total() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    ctx.client().tip(&sender, &creator, &ctx.get_token(), &700);
    ctx.client()
        .withdraw(&creator, &creator, &ctx.get_token(), &creator, &None);

    assert_eq!(ctx.token_client().balance(&creator), 700);
    assert_eq!(ctx.token_client().balance(&ctx.contract_id), 0);
    assert_eq!(ctx.client().get_total_tips(&creator, &ctx.get_token()), 700);

    // Withdrawable balance is now zero: a second withdraw must fail.
    let err = ctx
        .client()
        .try_withdraw(&creator, &creator, &ctx.get_token(), &creator, &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NothingToWithdraw.into());
}

#[test]
fn tip_rejects_zero_and_negative_amounts() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    for bad_amount in [0i128, -1i128] {
        let err = ctx
            .client()
            .try_tip(&sender, &creator, &ctx.get_token(), &bad_amount)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, Error::InvalidAmount.into());
    }

    // No tokens moved and no balance recorded for the rejected attempts.
    assert_eq!(ctx.token_client().balance(&sender), 1_000);
    assert_eq!(ctx.client().get_total_tips(&creator, &ctx.get_token()), 0);
}

#[test]
fn double_init_is_rejected() {
    let ctx = Ctx::new();

    let other_admin = Address::generate(&ctx.env);
    let err = ctx
        .client()
        .try_init(&ctx.token, &other_admin, &TEST_UPGRADE_TIMELOCK)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::AlreadyInitialized.into());
}

#[test]
fn init_rejects_zero_timelock() {
    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    let admin = Address::generate(&env);

    let contract_id = env.register(TipJar, ());
    let client = TipJarClient::new(&env, &contract_id);

    let err = client.try_init(&token, &admin, &0).unwrap_err().unwrap();
    assert_eq!(err, Error::InvalidTimelock.into());
}

#[test]
fn init_requires_admin_auth() {
    // No `env.mock_all_auths()` here: this deliberately leaves every address
    // unauthorized, so `init` must fail closed on `admin.require_auth()`
    // rather than letting an unauthenticated caller seed the admin. Without
    // that check, anyone could front-run the real deployer's `init` call on
    // a live network and permanently lock in themselves as admin.
    let env = Env::default();

    let token_admin = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    let admin = Address::generate(&env);

    let contract_id = env.register(TipJar, ());
    let client = TipJarClient::new(&env, &contract_id);

    let result = client.try_init(&token, &admin, &TEST_UPGRADE_TIMELOCK);
    assert!(
        result.is_err(),
        "init must not succeed without the admin's authorization"
    );
}

#[test]
fn withdraw_with_nothing_to_withdraw_errors() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);

    let err = ctx
        .client()
        .try_withdraw(&creator, &creator, &ctx.get_token(), &creator, &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NothingToWithdraw.into());
}

#[test]
fn tip_emits_tip_event_with_creator_topic_and_sender_amount_data() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    ctx.client().tip(&sender, &creator, &ctx.get_token(), &250);

    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (symbol_short!("tip"), creator.clone()).into_val(&ctx.env),
                (ctx.get_token(), sender.clone(), 250i128).into_val(&ctx.env),
            ),
        ]
    );
}

#[test]
fn withdraw_emits_withdraw_event_with_creator_topic_and_amount_data() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    ctx.client().tip(&sender, &creator, &ctx.get_token(), &250);

    // Checked immediately after tip(): each top-level call gets its own event
    // buffer, so withdraw()'s event isn't visible yet.
    let tip_events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        tip_events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (symbol_short!("tip"), creator.clone()).into_val(&ctx.env),
                (ctx.get_token(), sender.clone(), 250i128).into_val(&ctx.env),
            ),
        ]
    );

    ctx.client()
        .withdraw(&creator, &creator, &ctx.get_token(), &creator, &None);

    let withdraw_events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        withdraw_events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (symbol_short!("withdraw"), creator.clone()).into_val(&ctx.env),
                (ctx.get_token(), 250i128, creator.clone()).into_val(&ctx.env),
            ),
        ]
    );
}

#[test]
fn zero_fee_is_a_true_noop() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    // No set_fee call at all: fee_bps defaults to 0.
    ctx.client().tip(&sender, &creator, &ctx.get_token(), &400);

    // Checked immediately after tip(), before any other invocation: each
    // top-level call gets its own event buffer.
    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (symbol_short!("tip"), creator.clone()).into_val(&ctx.env),
                (ctx.get_token(), sender.clone(), 400i128).into_val(&ctx.env),
            ),
        ]
    );

    assert_eq!(ctx.client().get_total_tips(&creator, &ctx.get_token()), 400);
    assert_eq!(ctx.client().get_fee_balance(&ctx.get_token()), 0);

    ctx.client()
        .withdraw(&creator, &creator, &ctx.get_token(), &creator, &None);
    assert_eq!(ctx.token_client().balance(&creator), 400);
}

#[test]
fn explicit_zero_fee_config_is_also_a_true_noop() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);
    let collector = Address::generate(&ctx.env);

    ctx.client().set_fee(&ctx.admin, &0, &collector);
    ctx.client().tip(&sender, &creator, &ctx.get_token(), &400);

    // Checked immediately after tip(): no FeeCharged event, even though a
    // fee configuration (of 0 bps) is on record.
    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (symbol_short!("tip"), creator.clone()).into_val(&ctx.env),
                (ctx.get_token(), sender.clone(), 400i128).into_val(&ctx.env),
            ),
        ]
    );

    assert_eq!(ctx.client().get_total_tips(&creator, &ctx.get_token()), 400);
    assert_eq!(ctx.client().get_fee_balance(&ctx.get_token()), 0);
}

#[test]
fn tip_with_fee_credits_net_to_creator_and_accrues_fee_conserving_gross() {
    let ctx = Ctx::new();
    let sender = ctx.fund(100_000);
    let creator = Address::generate(&ctx.env);
    let collector = Address::generate(&ctx.env);

    ctx.client().set_fee(&ctx.admin, &250, &collector); // 2.5%
    ctx.client()
        .tip(&sender, &creator, &ctx.get_token(), &10_000);

    // Checked immediately after tip(): FeeCharged is published before Tip
    // within the same call.
    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (Symbol::new(&ctx.env, "fee_charged"), creator.clone()).into_val(&ctx.env),
                (10_000i128, 250i128, 9_750i128).into_val(&ctx.env),
            ),
            (
                ctx.contract_id.clone(),
                (symbol_short!("tip"), creator.clone()).into_val(&ctx.env),
                (ctx.get_token(), sender.clone(), 10_000i128).into_val(&ctx.env),
            ),
        ]
    );

    // fee = floor(10_000 * 250 / 10_000) = 250; net = 9_750.
    assert_eq!(ctx.client().get_fee_balance(&ctx.get_token()), 250);
    assert_eq!(
        ctx.client().get_total_tips(&creator, &ctx.get_token()),
        10_000
    ); // gross, historical
    assert_eq!(ctx.token_client().balance(&ctx.contract_id), 10_000);

    ctx.client()
        .withdraw(&creator, &creator, &ctx.get_token(), &creator, &None);
    assert_eq!(ctx.token_client().balance(&creator), 9_750);
}

#[test]
fn one_stroop_tip_at_max_fee_floors_to_zero_fee_but_still_conserves() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);
    let collector = Address::generate(&ctx.env);

    ctx.client().set_fee(&ctx.admin, &1_000, &collector); // 10%, the cap
    ctx.client().tip(&sender, &creator, &ctx.get_token(), &1);

    // floor(1 * 1_000 / 10_000) == 0: the creator gets the whole stroop.
    assert_eq!(ctx.client().get_fee_balance(&ctx.get_token()), 0);
    ctx.client()
        .withdraw(&creator, &creator, &ctx.get_token(), &creator, &None);
    assert_eq!(ctx.token_client().balance(&creator), 1);
}

#[test]
fn tip_fee_overflow_near_i128_max_panics_with_typed_error() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let collector = Address::generate(&ctx.env);

    let sender = Address::generate(&ctx.env);
    token::StellarAssetClient::new(&ctx.env, &ctx.token).mint(&sender, &i128::MAX);

    ctx.client().set_fee(&ctx.admin, &1_000, &collector);

    let err = ctx
        .client()
        .try_tip(&sender, &creator, &ctx.get_token(), &i128::MAX)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::FeeOverflow.into());

    // The failed fee computation must have reverted the whole call: no
    // tokens moved, no balance recorded.
    assert_eq!(ctx.token_client().balance(&sender), i128::MAX);
    assert_eq!(ctx.client().get_total_tips(&creator, &ctx.get_token()), 0);
}

#[test]
fn set_fee_by_non_admin_panics_with_typed_error() {
    let ctx = Ctx::new();
    let stranger = Address::generate(&ctx.env);
    let collector = Address::generate(&ctx.env);

    let err = ctx
        .client()
        .try_set_fee(&stranger, &250, &collector)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized.into());
}

#[test]
fn set_fee_above_cap_panics_with_typed_error() {
    let ctx = Ctx::new();
    let collector = Address::generate(&ctx.env);

    let err = ctx
        .client()
        .try_set_fee(&ctx.admin, &1_001, &collector)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::InvalidFee.into());

    // The cap itself must still be settable.
    ctx.client().set_fee(&ctx.admin, &1_000, &collector);
    assert_eq!(ctx.client().get_fee_bps(), 1_000);
}

#[test]
fn withdraw_fees_pays_out_collector_and_resets_balance() {
    let ctx = Ctx::new();
    let sender = ctx.fund(10_000);
    let creator = Address::generate(&ctx.env);
    let collector = Address::generate(&ctx.env);

    ctx.client().set_fee(&ctx.admin, &500, &collector); // 5%
    ctx.client()
        .tip(&sender, &creator, &ctx.get_token(), &2_000); // fee = 100

    ctx.client()
        .withdraw_fees(&collector, &ctx.get_token(), &None);

    assert_eq!(ctx.token_client().balance(&collector), 100);
    assert_eq!(ctx.client().get_fee_balance(&ctx.get_token()), 0);

    let err = ctx
        .client()
        .try_withdraw_fees(&collector, &ctx.get_token(), &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NothingToWithdraw.into());
}

#[test]
fn withdraw_fees_by_non_collector_panics_with_typed_error() {
    let ctx = Ctx::new();
    let sender = ctx.fund(10_000);
    let creator = Address::generate(&ctx.env);
    let collector = Address::generate(&ctx.env);
    let stranger = Address::generate(&ctx.env);

    ctx.client().set_fee(&ctx.admin, &500, &collector);
    ctx.client()
        .tip(&sender, &creator, &ctx.get_token(), &2_000);

    let err = ctx
        .client()
        .try_withdraw_fees(&stranger, &ctx.get_token(), &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotFeeCollector.into());
}

#[test]
fn withdraw_fees_with_nothing_configured_errors() {
    let ctx = Ctx::new();
    let anyone = Address::generate(&ctx.env);

    let err = ctx
        .client()
        .try_withdraw_fees(&anyone, &ctx.get_token(), &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NothingToWithdraw.into());
}

#[test]
fn withdraw_fees_is_isolated_per_token() {
    let ctx = Ctx::new();
    let token_b_admin = Address::generate(&ctx.env);
    let token_b = ctx
        .env
        .register_stellar_asset_contract_v2(token_b_admin)
        .address();
    let token_b_client = token::TokenClient::new(&ctx.env, &token_b);

    ctx.client().add_token(&ctx.admin, &token_b);

    let sender_a = ctx.fund(10_000);
    let sender_b = Address::generate(&ctx.env);
    token::StellarAssetClient::new(&ctx.env, &token_b).mint(&sender_b, &10_000);
    let creator = Address::generate(&ctx.env);
    let collector = Address::generate(&ctx.env);

    ctx.client().set_fee(&ctx.admin, &1_000, &collector); // 10%
    ctx.client()
        .tip(&sender_a, &creator, &ctx.get_token(), &2_000); // fee = 200
    ctx.client().tip(&sender_b, &creator, &token_b, &5_000); // fee = 500

    assert_eq!(ctx.client().get_fee_balance(&ctx.get_token()), 200);
    assert_eq!(ctx.client().get_fee_balance(&token_b), 500);

    ctx.client()
        .withdraw_fees(&collector, &ctx.get_token(), &None);
    assert_eq!(ctx.token_client().balance(&collector), 200);
    assert_eq!(token_b_client.balance(&collector), 0);
    assert_eq!(ctx.client().get_fee_balance(&ctx.get_token()), 0);
    assert_eq!(ctx.client().get_fee_balance(&token_b), 500);

    ctx.client().withdraw_fees(&collector, &token_b, &None);
    assert_eq!(token_b_client.balance(&collector), 500);
    assert_eq!(ctx.token_client().balance(&collector), 200);
    assert_eq!(ctx.client().get_fee_balance(&token_b), 0);

    let err = ctx
        .client()
        .try_withdraw_fees(&collector, &token_b, &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NothingToWithdraw.into());

    // Creator escrow in token A is untouched by the token-B fee withdrawal.
    assert_eq!(ctx.client().get_balance(&creator, &ctx.get_token()), 1_800);
    assert_eq!(ctx.client().get_balance(&creator, &token_b), 4_500);
}

#[test]
fn legacy_unparameterized_fee_balance_migrates_to_primary_token() {
    let ctx = Ctx::new();
    let collector = Address::generate(&ctx.env);
    ctx.client().set_fee(&ctx.admin, &500, &collector);

    ctx.env.as_contract(&ctx.contract_id, || {
        ctx.env
            .storage()
            .persistent()
            .set(&DataKey::FeeBalance, &100i128);
    });

    let donor = ctx.fund(100);
    ctx.token_client().transfer(&donor, &ctx.contract_id, &100);

    assert_eq!(ctx.client().get_fee_balance(&ctx.get_token()), 100);

    ctx.env.as_contract(&ctx.contract_id, || {
        let leftover: Option<i128> = ctx.env.storage().persistent().get(&DataKey::FeeBalance);
        assert!(leftover.is_none());
        let keyed: i128 = ctx
            .env
            .storage()
            .persistent()
            .get(&DataKey::FeeBalanceToken(ctx.get_token()))
            .unwrap();
        assert_eq!(keyed, 100);
    });

    ctx.client()
        .withdraw_fees(&collector, &ctx.get_token(), &None);
    assert_eq!(ctx.token_client().balance(&collector), 100);
    assert_eq!(ctx.client().get_fee_balance(&ctx.get_token()), 0);
}

#[test]
fn two_step_admin_transfer_completes_and_moves_governance() {
    let ctx = Ctx::new();
    let new_admin = Address::generate(&ctx.env);
    let collector = Address::generate(&ctx.env);

    ctx.client().propose_admin(&ctx.admin, &new_admin);
    assert_eq!(ctx.client().get_pending_admin(), Some(new_admin.clone()));
    assert_eq!(ctx.client().get_admin(), ctx.admin);

    ctx.client().accept_admin(&new_admin);
    assert_eq!(ctx.client().get_admin(), new_admin);
    assert_eq!(ctx.client().get_pending_admin(), None);

    // The old admin has lost governance...
    let err = ctx
        .client()
        .try_set_fee(&ctx.admin, &100, &collector)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized.into());

    // ...and the new admin now holds it.
    ctx.client().set_fee(&new_admin, &100, &collector);
}

#[test]
fn two_step_admin_transfer_can_be_abandoned() {
    let ctx = Ctx::new();
    let proposed = Address::generate(&ctx.env);

    ctx.client().propose_admin(&ctx.admin, &proposed);
    ctx.client().cancel_admin_transfer(&ctx.admin);

    assert_eq!(ctx.client().get_pending_admin(), None);
    assert_eq!(ctx.client().get_admin(), ctx.admin);

    // The abandoned proposal can no longer be accepted.
    let err = ctx
        .client()
        .try_accept_admin(&proposed)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NoPendingAdmin.into());
}

#[test]
fn propose_admin_by_non_admin_panics_with_typed_error() {
    let ctx = Ctx::new();
    let stranger = Address::generate(&ctx.env);
    let target = Address::generate(&ctx.env);

    let err = ctx
        .client()
        .try_propose_admin(&stranger, &target)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized.into());
}

#[test]
fn accept_admin_by_address_other_than_pending_panics_with_typed_error() {
    let ctx = Ctx::new();
    let proposed = Address::generate(&ctx.env);
    let impostor = Address::generate(&ctx.env);

    ctx.client().propose_admin(&ctx.admin, &proposed);

    let err = ctx
        .client()
        .try_accept_admin(&impostor)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NoPendingAdmin.into());
}

#[test]
fn accept_admin_with_no_pending_proposal_errors() {
    let ctx = Ctx::new();
    let anyone = Address::generate(&ctx.env);

    let err = ctx.client().try_accept_admin(&anyone).unwrap_err().unwrap();
    assert_eq!(err, Error::NoPendingAdmin.into());
}

#[test]
fn cancel_admin_transfer_with_nothing_pending_errors() {
    let ctx = Ctx::new();

    let err = ctx
        .client()
        .try_cancel_admin_transfer(&ctx.admin)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NoPendingAdmin.into());
}

#[cfg(test)]
mod fixtures {
    extern crate std;
    use super::*;
    use soroban_sdk::{
        testutils::Events,
        xdr::{ScVal, WriteXdr},
        Address, Env,
    };
    use std::{fs, path::PathBuf};

    #[test]
    fn event_schema_golden_fixtures() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(TipJar, ());
        let client = TipJarClient::new(&env, &contract_id);

        let token_admin = Address::generate(&env);
        let token = env
            .register_stellar_asset_contract_v2(token_admin.clone())
            .address();
        let admin = Address::generate(&env);
        client.init(&token, &admin, &TEST_UPGRADE_TIMELOCK);

        let creator = Address::generate(&env);
        let sender = Address::generate(&env);

        token::StellarAssetClient::new(&env, &token).mint(&sender, &1_000);

        client.tip(&sender, &creator, &token, &250);
        let events_after_tip = env.events().all().filter_by_contract(&contract_id);
        let tip_event = events_after_tip.events().last().unwrap().clone();

        client.withdraw(&creator, &creator, &token, &creator, &None);
        let events_after_withdraw = env.events().all().filter_by_contract(&contract_id);
        let withdraw_event = events_after_withdraw.events().last().unwrap().clone();

        let (tip_topics_val, tip_data_val) = match &tip_event.body {
            soroban_sdk::xdr::ContractEventBody::V0(v0) => (&v0.topics, &v0.data),
        };
        let (withdraw_topics_val, withdraw_data_val) = match &withdraw_event.body {
            soroban_sdk::xdr::ContractEventBody::V0(v0) => (&v0.topics, &v0.data),
        };

        assert_fixture(
            "tip_topics",
            &WriteXdr::to_xdr(tip_topics_val, soroban_sdk::xdr::Limits::none()).unwrap(),
        );
        assert_fixture(
            "tip_data",
            &WriteXdr::to_xdr(tip_data_val, soroban_sdk::xdr::Limits::none()).unwrap(),
        );
        assert_fixture(
            "withdraw_topics",
            &WriteXdr::to_xdr(withdraw_topics_val, soroban_sdk::xdr::Limits::none()).unwrap(),
        );
        assert_fixture(
            "withdraw_data",
            &WriteXdr::to_xdr(withdraw_data_val, soroban_sdk::xdr::Limits::none()).unwrap(),
        );
    }

    /// Decode raw XDR bytes to a JSON string that is human-readable in a PR
    /// diff. The XDR payload is always a single `ScVal` (for event `data`) or
    /// a `ScVec` (for event `topics`). We attempt both: first a `ScVec`
    /// (topics form), then a bare `ScVal` (data form). The resulting JSON is
    /// pretty-printed and deterministic.
    fn xdr_to_json(xdr_bytes: &[u8]) -> String {
        use soroban_sdk::xdr::{Limits, ReadXdr, ScVal, ScVec};

        // Try decoding as ScVec (topics form) first, then as a bare ScVal
        // (data form). This matches how the indexer decodes the same bytes.
        let json_value = if let Ok(vec_sc) = ScVec::from_xdr(xdr_bytes, Limits::none()) {
            let items: Vec<serde_json::Value> = vec_sc.iter().map(scval_to_json).collect();
            serde_json::Value::Array(items)
        } else {
            let sc_val = ScVal::from_xdr(xdr_bytes, Limits::none())
                .unwrap_or_else(|e| {
                    panic!(
                        "Failed to decode XDR as ScVal or ScVec: {:?}",
                        e
                    )
                });
            scval_to_json(&sc_val)
        };

        // Two-space indent for readable diffs; trailing newline for clean
        // `diff` output.
        let mut out = serde_json::to_string_pretty(&json_value)
            .expect("serde_json serialization is infallible for Value");
        out.push('\n');
        out
    }

    /// Recursively convert an XDR `ScVal` to a `serde_json::Value`.
    ///
    /// The representation is chosen to be maximally readable in a git diff:
    /// - Every variant is wrapped in a one-key object so the type is
    ///   immediately visible without looking at the surrounding context.
    /// - Large integers that exceed JS's safe-integer range are emitted as
    ///   JSON strings to avoid precision loss in tools that parse the JSON.
    /// - Raw byte sequences (e.g. `Bytes`, `BytesN`) are hex-encoded.
    /// - `Address` is emitted as a `{"AccountId": "<hex>"}` object (the raw
    ///   XDR public-key bytes; the strkey form is not available in test-utils
    ///   without extra dependencies).
    fn scval_to_json(val: &ScVal) -> serde_json::Value {
        use soroban_sdk::xdr::{AccountId, Hash, PublicKey, ScAddress, Uint256};
        use serde_json::{json, Value};

        match val {
            ScVal::Bool(b) => json!({"Bool": b}),
            ScVal::Void => json!({"Void": null}),
            ScVal::Error(e) => json!({"Error": std::format!("{:?}", e)}),
            ScVal::U32(n) => json!({"U32": n}),
            ScVal::I32(n) => json!({"I32": n}),
            // U64/I64: values outside JS's safe integer range are strings.
            ScVal::U64(n) => {
                if *n <= 9_007_199_254_740_991u64 {
                    json!({"U64": n})
                } else {
                    json!({"U64": n.to_string()})
                }
            }
            ScVal::I64(n) => {
                if *n >= -9_007_199_254_740_991i64 && *n <= 9_007_199_254_740_991i64 {
                    json!({"I64": n})
                } else {
                    json!({"I64": n.to_string()})
                }
            }
            ScVal::Timepoint(tp) => json!({"Timepoint": tp.0.to_string()}),
            ScVal::Duration(d) => json!({"Duration": d.0.to_string()}),
            // U128/I128: always emit as strings to avoid precision loss.
            ScVal::U128(parts) => {
                let hi = parts.hi as u128;
                let lo = parts.lo as u128;
                let v = (hi << 64) | lo;
                json!({"U128": v.to_string()})
            }
            ScVal::I128(parts) => {
                // hi is i64, lo is u64 — reconstruct the i128 value.
                let hi = parts.hi as i128;
                let lo = parts.lo as u128 as i128;
                let v = (hi << 64) | lo;
                json!({"I128": v.to_string()})
            }
            ScVal::U256(parts) => {
                json!({"U256": std::format!(
                    "{:016x}:{:016x}:{:016x}:{:016x}",
                    parts.hi_hi, parts.hi_lo, parts.lo_hi, parts.lo_lo
                )})
            }
            ScVal::I256(parts) => {
                json!({"I256": std::format!(
                    "{:016x}:{:016x}:{:016x}:{:016x}",
                    parts.hi_hi, parts.hi_lo, parts.lo_hi, parts.lo_lo
                )})
            }
            ScVal::Bytes(b) => {
                // ScBytes is BytesM<N> — deref to &[u8] for iteration.
                let hex: String = b.iter().map(|byte| std::format!("{:02x}", byte)).collect();
                json!({"Bytes": hex})
            }
            ScVal::String(s) => {
                // ScString is StringM<N> — use as_slice() for bytes.
                let text = std::str::from_utf8(s.as_slice())
                    .map(|t| Value::String(t.to_string()))
                    .unwrap_or_else(|_| {
                        let hex: String =
                            s.as_slice().iter().map(|b| std::format!("{:02x}", b)).collect();
                        Value::String(std::format!("0x{}", hex))
                    });
                json!({"String": text})
            }
            ScVal::Symbol(sym) => {
                // ScSymbol has a to_string() impl that gives the symbol text.
                json!({"Symbol": sym.to_string()})
            }
            ScVal::Vec(Some(vec)) => {
                // ScVec implements Deref<Target=[ScVal]>.
                let items: Vec<Value> = vec.iter().map(scval_to_json).collect();
                json!({"Vec": items})
            }
            ScVal::Vec(None) => json!({"Vec": null}),
            ScVal::Map(Some(map)) => {
                // ScMap is a sorted VecM<ScMapEntry>. Represent as ordered
                // array of {key, val} pairs — preserves sort order and avoids
                // the JSON-object restriction of string-only keys.
                let pairs: Vec<Value> = map.iter()
                    .map(|entry| json!({
                        "key": scval_to_json(&entry.key),
                        "val": scval_to_json(&entry.val)
                    }))
                    .collect();
                json!({"Map": pairs})
            }
            ScVal::Map(None) => json!({"Map": null}),
            ScVal::Address(addr) => {
                match addr {
                    ScAddress::Account(AccountId(
                        PublicKey::PublicKeyTypeEd25519(Uint256(bytes)),
                    )) => {
                        let hex: String =
                            bytes.iter().map(|b| std::format!("{:02x}", b)).collect();
                        json!({"Address": {"Account": hex}})
                    }
                    ScAddress::Contract(Hash(bytes)) => {
                        let hex: String =
                            bytes.iter().map(|b| std::format!("{:02x}", b)).collect();
                        json!({"Address": {"Contract": hex}})
                    }
                }
            }
            ScVal::LedgerKeyContractInstance => json!({"LedgerKeyContractInstance": null}),
            ScVal::LedgerKeyNonce(n) => json!({"LedgerKeyNonce": n.nonce}),
            ScVal::ContractInstance(_inst) => {
                json!({"ContractInstance": "<opaque>"})
            }
        }
    }

    /// Assert that `actual_xdr` matches the committed `.xdr` golden file and
    /// that the decoded `.json` companion matches as well.
    ///
    /// When `UPDATE_FIXTURES=1` is set both files are written (or overwritten)
    /// from the current run. The `.json` file exists solely so that PR diffs
    /// show a human-readable representation of what changed — reviewers should
    /// look at the JSON diff, not the binary XDR diff.
    fn assert_fixture(name: &str, actual_xdr: &[u8]) {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let fixture_dir = PathBuf::from(manifest_dir)
            .join("tests")
            .join("fixtures");
        let xdr_path = fixture_dir.join(std::format!("{}.xdr", name));
        let json_path = fixture_dir.join(std::format!("{}.json", name));

        let actual_json = xdr_to_json(actual_xdr);

        if std::env::var("UPDATE_FIXTURES").is_ok() {
            fs::create_dir_all(&fixture_dir).unwrap();
            fs::write(&xdr_path, actual_xdr).unwrap();
            fs::write(&json_path, actual_json.as_bytes()).unwrap();
        } else {
            // --- XDR assertion ---
            let expected_xdr = fs::read(&xdr_path).unwrap_or_else(|_| {
                panic!(
                    "XDR fixture missing: {:?}. Run with UPDATE_FIXTURES=1",
                    xdr_path
                )
            });
            assert_eq!(
                expected_xdr, actual_xdr,
                "XDR fixture '{}' mismatch — run with UPDATE_FIXTURES=1 to regenerate, \
                 then review the companion .json diff carefully before committing.",
                name
            );

            // --- JSON companion assertion ---
            // This is the human-readable counterpart: its diff is what
            // reviewers must inspect when UPDATE_FIXTURES=1 was used in a PR.
            let expected_json = fs::read_to_string(&json_path).unwrap_or_else(|_| {
                panic!(
                    "JSON companion fixture missing: {:?}. Run with UPDATE_FIXTURES=1",
                    json_path
                )
            });
            assert_eq!(
                expected_json, actual_json,
                "JSON companion fixture '{}' mismatch — if you ran UPDATE_FIXTURES=1, \
                 the .json file was also regenerated. Review it carefully: the diff \
                 shows exactly which event fields, types, or ordering changed.",
                name
            );
        }
    }
}
