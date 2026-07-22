//! Integration tests for the TipJar contract.
//!
//! Uses Soroban's in-process test environment — no testnet or env vars needed.
//! Run with: cargo test -p tipjar-integration-tests

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events as _},
    token, vec, Address, Env, IntoVal,
};
use tipjar::{Error, TipJar, TipJarClient};

/// Timelock value used to init the contract in these tests; the upgrade flow
/// itself is exercised in contracts/tipjar/src/test_upgrade.rs, so any
/// nonzero value works here.
const TIMELOCK: u32 = 1000;

struct Ctx {
    env: Env,
    contract_id: Address,
    token: Address,
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
        }
    }

    fn c(&self) -> TipJarClient<'_> {
        TipJarClient::new(&self.env, &self.contract_id)
    }

    fn tipper(&self) -> Address {
        let a = Address::generate(&self.env);
        token::StellarAssetClient::new(&self.env, &self.token).mint(&a, &10_000);
        a
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
    ctx.c().tip(&ctx.tipper(), &ctx.creator(), &100);
}

#[test]
fn test_total_after_tip() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &100);
    assert_eq!(ctx.c().get_total_tips(&creator), 100);
}

#[test]
fn test_total_accumulates_across_multiple_tips() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &100);
    ctx.c().tip(&tipper, &creator, &200);
    ctx.c().tip(&tipper, &creator, &300);
    assert_eq!(ctx.c().get_total_tips(&creator), 600);
}

#[test]
fn test_withdrawal() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &500);
    ctx.c().withdraw(&creator, &creator, &creator, &None);
    assert_eq!(ctx.c().get_total_tips(&creator), 500);
    assert_eq!(
        token::Client::new(&ctx.env, &ctx.token).balance(&creator),
        500
    );
}

#[test]
fn test_full_withdrawal_then_nothing_left() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &1_000);
    ctx.c().withdraw(&creator, &creator, &creator, &None);

    let err = ctx
        .c()
        .try_withdraw(&creator, &creator, &creator, &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NothingToWithdraw.into());
}

#[test]
fn test_partial_withdrawal_keeps_remainder() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &1_000);

    ctx.c().withdraw(&creator, &creator, &creator, &Some(400));

    assert_eq!(ctx.c().get_total_tips(&creator), 1_000);
    assert_eq!(
        token::Client::new(&ctx.env, &ctx.token).balance(&creator),
        400
    );

    // Remainder is still withdrawable.
    ctx.c().withdraw(&creator, &creator, &creator, &Some(600));
    assert_eq!(
        token::Client::new(&ctx.env, &ctx.token).balance(&creator),
        1_000
    );
}

#[test]
fn test_partial_withdrawal_rejects_amount_over_balance() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &300);

    let err = ctx
        .c()
        .try_withdraw(&creator, &creator, &creator, &Some(301))
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
        .try_withdraw(&creator, &creator, &creator, &None)
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
            .try_tip(&tipper, &creator, &bad)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, Error::InvalidAmount.into());
    }
}

#[test]
fn test_event_emission() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &100);

    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (symbol_short!("tip"), creator.clone()).into_val(&ctx.env),
                (tipper.clone(), 100i128).into_val(&ctx.env),
            ),
        ]
    );
}
