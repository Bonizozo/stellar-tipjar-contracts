//! Integration tests for the TipJar contract.
//!
//! Uses Soroban's in-process test environment — no testnet or env vars needed.
//! Run with: cargo test -p tipjar-integration-tests

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events as _, Ledger as _},
    token, vec, Address, Env, IntoVal,
};
use tipjar::{Error, TipJar, TipJarClient, PAUSE_FLAG_ALL};

/// Timelock value used to init the contract in these tests; the upgrade flow
/// itself is exercised in contracts/tipjar/src/test_upgrade.rs, so any
/// nonzero value works here.
const TIMELOCK: u32 = 1000;

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
        let token = env
            .register_stellar_asset_contract_v2(Address::generate(&env))
            .address();
        let admin = Address::generate(&env);
        let contract_id = env.register(TipJar, ());
        let c = TipJarClient::new(&env, &contract_id);
        c.init(&token, &admin, &TIMELOCK);
        Ctx {
            env,
            contract_id,
            token,
            admin,
        }
    }

    fn c(&self) -> TipJarClient<'_> {
        TipJarClient::new(&self.env, &self.contract_id)
    }

    fn tipper(&self) -> Address {
        self.funded_holder(&self.token, 10_000)
    }

    fn funded_holder(&self, token: &Address, amount: i128) -> Address {
        let a = Address::generate(&self.env);
        token::StellarAssetClient::new(&self.env, token).mint(&a, &amount);
        a
    }

    fn token_client(&self, token: &Address) -> token::TokenClient<'_> {
        token::TokenClient::new(&self.env, token)
    }

    fn add_token(&self) -> Address {
        let token = self
            .env
            .register_stellar_asset_contract_v2(Address::generate(&self.env))
            .address();
        self.c().add_token(&self.admin, &token);
        token
    }

    fn creator(&self) -> Address {
        Address::generate(&self.env)
    }
}

#[test]
fn test_contract_deployment() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(TipJar, ());
    let client = TipJarClient::new(&env, &contract_id);
    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    client.init(&token, &Address::generate(&env), &TIMELOCK); // must not panic
}

#[test]
fn test_send_tip() {
    let ctx = Ctx::new();
    ctx.c().tip(&ctx.tipper(), &ctx.creator(), &ctx.token, &100);
}

#[test]
fn test_total_after_tip() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &ctx.token, &100);
    assert_eq!(ctx.c().get_total_tips(&creator, &ctx.token), 100);
}

#[test]
fn test_total_accumulates_across_multiple_tips() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &ctx.token, &100);
    ctx.c().tip(&tipper, &creator, &ctx.token, &200);
    ctx.c().tip(&tipper, &creator, &ctx.token, &300);
    assert_eq!(ctx.c().get_total_tips(&creator, &ctx.token), 600);
}

#[test]
fn test_withdrawal() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &ctx.token, &500);
    ctx.c()
        .withdraw(&creator, &creator, &ctx.token, &creator, &None);
    assert_eq!(ctx.c().get_total_tips(&creator, &ctx.token), 500);
    assert_eq!(ctx.token_client(&ctx.token).balance(&creator), 500);
}

#[test]
fn test_full_withdrawal_then_nothing_left() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &ctx.token, &1_000);
    ctx.c()
        .withdraw(&creator, &creator, &ctx.token, &creator, &None);

    let err = ctx
        .c()
        .try_withdraw(&creator, &creator, &ctx.token, &creator, &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NothingToWithdraw.into());
}

#[test]
fn test_partial_withdrawal_keeps_remainder() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &ctx.token, &1_000);

    ctx.c()
        .withdraw(&creator, &creator, &ctx.token, &creator, &Some(400));

    assert_eq!(ctx.c().get_total_tips(&creator, &ctx.token), 1_000);
    assert_eq!(ctx.token_client(&ctx.token).balance(&creator), 400);

    // Remainder is still withdrawable.
    ctx.c()
        .withdraw(&creator, &creator, &ctx.token, &creator, &Some(600));
    assert_eq!(ctx.token_client(&ctx.token).balance(&creator), 1_000);
}

#[test]
fn test_partial_withdrawal_rejects_amount_over_balance() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &ctx.token, &300);

    let err = ctx
        .c()
        .try_withdraw(&creator, &creator, &ctx.token, &creator, &Some(301))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::InvalidAmount.into());
}

#[test]
fn test_insufficient_balance_rejected() {
    let ctx = Ctx::new();
    let creator = ctx.creator();
    let err = ctx
        .c()
        .try_withdraw(&creator, &creator, &ctx.token, &creator, &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NothingToWithdraw.into());
}

#[test]
fn test_invalid_amount_rejected() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    for bad in [0i128, -1i128] {
        let err = ctx
            .c()
            .try_tip(&tipper, &creator, &ctx.token, &bad)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, Error::InvalidAmount.into());
    }
}

#[test]
fn test_event_emission() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &ctx.token, &100);

    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (symbol_short!("tip"), creator.clone()).into_val(&ctx.env),
                (ctx.token.clone(), tipper.clone(), 100i128).into_val(&ctx.env),
            ),
        ]
    );
}

#[test]
fn test_multifeature_non_primary_token_fee_operator_after_guardian_pause_expires() {
    let ctx = Ctx::new();
    let secondary_token = ctx.add_token();
    let tipper = ctx.funded_holder(&secondary_token, 10_000);
    let creator = ctx.creator();
    let operator = Address::generate(&ctx.env);
    let fee_collector = Address::generate(&ctx.env);
    let guardian = Address::generate(&ctx.env);

    ctx.c().set_fee(&ctx.admin, &250);
    ctx.c().propose_fee_collector(&ctx.admin, &fee_collector);
    ctx.c().accept_fee_collector(&fee_collector);
    ctx.c().set_guardian(&ctx.admin, &guardian);
    ctx.c().set_guardian_pause_duration(&ctx.admin, &5);
    ctx.c().pause_all(&guardian);

    assert_eq!(ctx.c().pause_flags(), PAUSE_FLAG_ALL);
    let paused_err = ctx
        .c()
        .try_tip(&tipper, &creator, &secondary_token, &1_000)
        .unwrap_err()
        .unwrap();
    assert_eq!(paused_err, Error::TipsPaused.into());

    let expiry = ctx.c().guardian_pause_expiry_ledger();
    ctx.env.ledger().with_mut(|ledger| {
        ledger.sequence_number = expiry;
    });
    assert_eq!(ctx.c().pause_flags(), 0);

    ctx.c().tip(&tipper, &creator, &secondary_token, &1_000);

    assert_eq!(ctx.c().get_total_tips(&creator, &secondary_token), 1_000);
    assert_eq!(ctx.c().get_balance(&creator, &secondary_token), 975);
    assert_eq!(ctx.c().get_fee_balance(&secondary_token), 25);
    assert_eq!(
        ctx.token_client(&secondary_token).balance(&ctx.contract_id),
        1_000
    );

    ctx.c()
        .authorize_operator(&creator, &operator, &975, &(expiry + 10));
    ctx.c()
        .withdraw(&operator, &creator, &secondary_token, &creator, &None);
    ctx.c()
        .withdraw_fees(&fee_collector, &secondary_token, &None);

    assert_eq!(ctx.c().get_balance(&creator, &secondary_token), 0);
    assert_eq!(ctx.c().get_fee_balance(&secondary_token), 0);
    assert_eq!(ctx.token_client(&secondary_token).balance(&creator), 975);
    assert_eq!(
        ctx.token_client(&secondary_token).balance(&fee_collector),
        25
    );
    assert_eq!(
        ctx.token_client(&secondary_token).balance(&ctx.contract_id),
        0
    );
    assert_eq!(ctx.token_client(&ctx.token).balance(&creator), 0);
}
