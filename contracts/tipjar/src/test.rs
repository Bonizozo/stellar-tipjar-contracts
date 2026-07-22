//! Unit tests for the TipJar contract.
//!
//! Lives at `src/test.rs` (wired in via `#[cfg(test)] mod test;` in `lib.rs`)
//! rather than a top-level `tests/` integration crate, since these tests only
//! need access to the contract's own client/types and exercising them as a
//! unit-test module avoids a second crate compilation unit for such a small
//! contract.

use crate::{Error, TipJar, TipJarClient};
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
}

#[test]
fn tip_escrows_tokens_and_updates_balance_and_total() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    ctx.client().tip(&sender, &creator, &400);

    // Tokens left the sender and landed in the contract's escrow.
    assert_eq!(ctx.token_client().balance(&sender), 600);
    assert_eq!(ctx.token_client().balance(&ctx.contract_id), 400);

    // Historical total rose by the tipped amount.
    assert_eq!(ctx.client().get_total_tips(&creator), 400);

    // Withdrawable balance rose by the same amount: with a single creator and
    // a single tip, the full escrowed amount must be exactly what withdraw()
    // pays out.
    ctx.client().withdraw(&creator, &creator, &creator, &None);
    assert_eq!(ctx.token_client().balance(&creator), 400);
}

#[test]
fn multiple_tips_accumulate_for_the_same_creator() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    ctx.client().tip(&sender, &creator, &100);
    ctx.client().tip(&sender, &creator, &200);
    ctx.client().tip(&sender, &creator, &300);

    assert_eq!(ctx.client().get_total_tips(&creator), 600);
    assert_eq!(ctx.token_client().balance(&ctx.contract_id), 600);

    ctx.client().withdraw(&creator, &creator, &creator, &None);
    assert_eq!(ctx.token_client().balance(&creator), 600);
    // Historical total survives the withdrawal.
    assert_eq!(ctx.client().get_total_tips(&creator), 600);
}

#[test]
fn get_total_tips_is_zero_for_unknown_creator_then_tracks_sum() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);

    assert_eq!(ctx.client().get_total_tips(&creator), 0);

    let sender = ctx.fund(500);
    ctx.client().tip(&sender, &creator, &150);
    ctx.client().tip(&sender, &creator, &50);

    assert_eq!(ctx.client().get_total_tips(&creator), 200);
}

#[test]
fn withdraw_pays_out_full_balance_resets_it_and_keeps_total() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    ctx.client().tip(&sender, &creator, &700);
    ctx.client().withdraw(&creator, &creator, &creator, &None);

    assert_eq!(ctx.token_client().balance(&creator), 700);
    assert_eq!(ctx.token_client().balance(&ctx.contract_id), 0);
    assert_eq!(ctx.client().get_total_tips(&creator), 700);

    // Withdrawable balance is now zero: a second withdraw must fail.
    let err = ctx
        .client()
        .try_withdraw(&creator, &creator, &creator, &None)
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
            .try_tip(&sender, &creator, &bad_amount)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, Error::InvalidAmount.into());
    }

    // No tokens moved and no balance recorded for the rejected attempts.
    assert_eq!(ctx.token_client().balance(&sender), 1_000);
    assert_eq!(ctx.client().get_total_tips(&creator), 0);
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
        .try_withdraw(&creator, &creator, &creator, &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NothingToWithdraw.into());
}

#[test]
fn tip_emits_tip_event_with_creator_topic_and_sender_amount_data() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    ctx.client().tip(&sender, &creator, &250);

    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (symbol_short!("tip"), creator.clone()).into_val(&ctx.env),
                (sender.clone(), 250i128).into_val(&ctx.env),
            ),
        ]
    );
}

#[test]
fn withdraw_emits_withdraw_event_with_creator_topic_and_amount_data() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    ctx.client().tip(&sender, &creator, &250);
    ctx.client().withdraw(&creator, &creator, &creator, &None);

    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (symbol_short!("withdraw"), creator.clone()).into_val(&ctx.env),
                (250i128, creator.clone()).into_val(&ctx.env),
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
    ctx.client().tip(&sender, &creator, &400);

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
                (sender.clone(), 400i128).into_val(&ctx.env),
            ),
        ]
    );

    assert_eq!(ctx.client().get_total_tips(&creator), 400);
    assert_eq!(ctx.client().get_fee_balance(), 0);

    ctx.client().withdraw(&creator, &creator, &creator, &None);
    assert_eq!(ctx.token_client().balance(&creator), 400);
}

#[test]
fn explicit_zero_fee_config_is_also_a_true_noop() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);
    let collector = Address::generate(&ctx.env);

    ctx.client().set_fee(&ctx.admin, &0, &collector);
    ctx.client().tip(&sender, &creator, &400);

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
                (sender.clone(), 400i128).into_val(&ctx.env),
            ),
        ]
    );

    assert_eq!(ctx.client().get_total_tips(&creator), 400);
    assert_eq!(ctx.client().get_fee_balance(), 0);
}

#[test]
fn tip_with_fee_credits_net_to_creator_and_accrues_fee_conserving_gross() {
    let ctx = Ctx::new();
    let sender = ctx.fund(100_000);
    let creator = Address::generate(&ctx.env);
    let collector = Address::generate(&ctx.env);

    ctx.client().set_fee(&ctx.admin, &250, &collector); // 2.5%
    ctx.client().tip(&sender, &creator, &10_000);

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
                (sender.clone(), 10_000i128).into_val(&ctx.env),
            ),
        ]
    );

    // fee = floor(10_000 * 250 / 10_000) = 250; net = 9_750.
    assert_eq!(ctx.client().get_fee_balance(), 250);
    assert_eq!(ctx.client().get_total_tips(&creator), 10_000); // gross, historical
    assert_eq!(ctx.token_client().balance(&ctx.contract_id), 10_000);

    ctx.client().withdraw(&creator, &creator, &creator, &None);
    assert_eq!(ctx.token_client().balance(&creator), 9_750);
}

#[test]
fn one_stroop_tip_at_max_fee_floors_to_zero_fee_but_still_conserves() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);
    let collector = Address::generate(&ctx.env);

    ctx.client().set_fee(&ctx.admin, &1_000, &collector); // 10%, the cap
    ctx.client().tip(&sender, &creator, &1);

    // floor(1 * 1_000 / 10_000) == 0: the creator gets the whole stroop.
    assert_eq!(ctx.client().get_fee_balance(), 0);
    ctx.client().withdraw(&creator, &creator, &creator, &None);
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
        .try_tip(&sender, &creator, &i128::MAX)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::FeeOverflow.into());

    // The failed fee computation must have reverted the whole call: no
    // tokens moved, no balance recorded.
    assert_eq!(ctx.token_client().balance(&sender), i128::MAX);
    assert_eq!(ctx.client().get_total_tips(&creator), 0);
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
    ctx.client().tip(&sender, &creator, &2_000); // fee = 100

    ctx.client().withdraw_fees(&collector, &None);

    assert_eq!(ctx.token_client().balance(&collector), 100);
    assert_eq!(ctx.client().get_fee_balance(), 0);

    let err = ctx
        .client()
        .try_withdraw_fees(&collector, &None)
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
    ctx.client().tip(&sender, &creator, &2_000);

    let err = ctx
        .client()
        .try_withdraw_fees(&stranger, &None)
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
        .try_withdraw_fees(&anyone, &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NothingToWithdraw.into());
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
    use soroban_sdk::{testutils::Events, xdr::WriteXdr, Address, Env};
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

        client.tip(&sender, &creator, &250);
        let events_after_tip = env.events().all().filter_by_contract(&contract_id);
        let tip_event = events_after_tip.events().last().unwrap().clone();

        client.withdraw(&creator, &creator, &creator, &None);
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

    fn assert_fixture(name: &str, actual_xdr: &[u8]) {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let fixture_path = PathBuf::from(manifest_dir)
            .join("tests")
            .join("fixtures")
            .join(std::format!("{}.xdr", name));

        if std::env::var("UPDATE_FIXTURES").is_ok() {
            fs::create_dir_all(fixture_path.parent().unwrap()).unwrap();
            fs::write(&fixture_path, actual_xdr).unwrap();
        } else {
            let expected_xdr = fs::read(&fixture_path).unwrap_or_else(|_| {
                panic!(
                    "Fixture missing: {:?}. Run with UPDATE_FIXTURES=1",
                    fixture_path
                )
            });
            assert_eq!(
                expected_xdr, actual_xdr,
                "Fixture {} mismatch! Run with UPDATE_FIXTURES=1 to update.",
                name
            );
        }
    }
}
