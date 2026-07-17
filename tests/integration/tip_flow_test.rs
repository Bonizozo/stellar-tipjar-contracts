//! Integration tests for the TipJar contract.
//!
//! Uses Soroban's in-process test environment — no testnet or env vars needed.
//! Run with: cargo test -p tipjar-integration-tests

use soroban_sdk::{
    testutils::{Address as _, Events as _},
    token, Address, Env, Symbol, TryFromVal,
};
use tipjar_legacy::{TipJarContract, TipJarContractClient, TipJarError};

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
        let contract_id = env.register(TipJarContract, ());
        let c = TipJarContractClient::new(&env, &contract_id);
        c.init(&admin);
        c.add_token(&admin, &token);
        Ctx {
            env,
            contract_id,
            token,
        }
    }

    fn c(&self) -> TipJarContractClient<'_> {
        TipJarContractClient::new(&self.env, &self.contract_id)
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
    let contract_id = env.register(TipJarContract, ());
    let client = TipJarContractClient::new(&env, &contract_id);
    client.init(&Address::generate(&env)); // must not panic
}

#[test]
fn test_send_tip() {
    let ctx = Ctx::new();
    ctx.c().tip(&ctx.tipper(), &ctx.creator(), &ctx.token, &100);
}

#[test]
fn test_balance_after_tip() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &ctx.token, &100);
    assert_eq!(ctx.c().get_withdrawable_balance(&creator, &ctx.token), 100);
}

#[test]
fn test_balance_accumulates_across_multiple_tips() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &ctx.token, &100);
    ctx.c().tip(&tipper, &creator, &ctx.token, &200);
    ctx.c().tip(&tipper, &creator, &ctx.token, &300);
    assert_eq!(ctx.c().get_withdrawable_balance(&creator, &ctx.token), 600);
    assert_eq!(ctx.c().get_total_tips(&creator, &ctx.token), 600);
}

#[test]
fn test_withdrawal() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &ctx.token, &500);
    ctx.c().withdraw(&creator, &ctx.token, &None);
    assert_eq!(ctx.c().get_withdrawable_balance(&creator, &ctx.token), 0);
    assert_eq!(ctx.c().get_total_tips(&creator, &ctx.token), 500);
    assert_eq!(
        token::Client::new(&ctx.env, &ctx.token).balance(&creator),
        500
    );
}

#[test]
fn test_full_withdrawal() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &ctx.token, &1_000);
    ctx.c().withdraw(&creator, &ctx.token, &None);
    assert_eq!(ctx.c().get_withdrawable_balance(&creator, &ctx.token), 0);
}

#[test]
fn test_partial_withdrawal_keeps_remainder() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &ctx.token, &1_000);

    ctx.c().withdraw(&creator, &ctx.token, &Some(400));

    assert_eq!(ctx.c().get_withdrawable_balance(&creator, &ctx.token), 600);
    assert_eq!(ctx.c().get_total_tips(&creator, &ctx.token), 1_000);
    assert_eq!(
        token::Client::new(&ctx.env, &ctx.token).balance(&creator),
        400
    );
}

#[test]
fn test_partial_withdrawal_rejects_amount_over_balance() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &ctx.token, &300);

    let err = ctx
        .c()
        .try_withdraw(&creator, &ctx.token, &Some(301))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, TipJarError::InsufficientBalance.into());
    assert_eq!(ctx.c().get_withdrawable_balance(&creator, &ctx.token), 300);
}

#[test]
fn test_insufficient_balance_rejected() {
    let ctx = Ctx::new();
    let creator = ctx.creator();
    let err = ctx
        .c()
        .try_withdraw(&creator, &ctx.token, &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, TipJarError::NothingToWithdraw.into());
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
        assert_eq!(err, TipJarError::InvalidAmount.into());
    }
}

#[test]
fn test_event_emission() {
    let ctx = Ctx::new();
    let (tipper, creator) = (ctx.tipper(), ctx.creator());
    ctx.c().tip(&tipper, &creator, &ctx.token, &100);
    let has_tip_event = ctx.env.events().all().iter().any(|(_, topics, _)| {
        topics
            .get(0)
            .and_then(|v| Symbol::try_from_val(&ctx.env, &v).ok())
            .map(|s| s == Symbol::new(&ctx.env, "tip"))
            .unwrap_or(false)
    });
    assert!(has_tip_event, "expected a 'tip' event");
}
