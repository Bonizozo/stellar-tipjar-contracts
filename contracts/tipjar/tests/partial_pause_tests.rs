#![cfg(test)]

extern crate std;

use soroban_sdk::{testutils::Address as _, token, Address, Env, String as SorobanString};
use tipjar::{PauseScope, TipJarContract, TipJarContractClient, TipJarError};

// ── helpers ───────────────────────────────────────────────────────────────────

struct Ctx {
    env: Env,
    client: TipJarContractClient<'static>,
    admin: Address,
    token: Address,
}

impl Ctx {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let token_admin = Address::generate(&env);
        let token = env
            .register_stellar_asset_contract_v2(token_admin.clone())
            .address();

        let admin = Address::generate(&env);
        let contract_id = env.register(TipJarContract, ());
        let client = TipJarContractClient::new(&env, &contract_id);

        client.init(&admin);
        client.add_token(&admin, &token);

        Ctx {
            env,
            client,
            admin,
            token,
        }
    }

    fn mint(&self, to: &Address, amount: i128) {
        token::StellarAssetClient::new(&self.env, &self.token).mint(to, &amount);
    }

    fn reason(&self, s: &str) -> SorobanString {
        SorobanString::from_str(&self.env, s)
    }
}

// ── is_feature_paused defaults ────────────────────────────────────────────────

#[test]
fn test_feature_pause_false_by_default() {
    let ctx = Ctx::new();
    assert!(!ctx.client.is_feature_paused(&PauseScope::Tipping));
    assert!(!ctx.client.is_feature_paused(&PauseScope::Withdrawals));
    assert!(!ctx.client.is_feature_paused(&PauseScope::Subscriptions));
}

// ── admin controls ────────────────────────────────────────────────────────────

#[test]
fn test_pause_feature_requires_admin() {
    let ctx = Ctx::new();
    let non_admin = Address::generate(&ctx.env);
    let result =
        ctx.client
            .try_pause_feature(&non_admin, &PauseScope::Tipping, &ctx.reason("test"));
    assert_eq!(
        result.err().unwrap().unwrap(),
        TipJarError::Unauthorized.into()
    );
}

#[test]
fn test_unpause_feature_requires_admin() {
    let ctx = Ctx::new();
    ctx.client
        .pause_feature(&ctx.admin, &PauseScope::Tipping, &ctx.reason("test"));
    let non_admin = Address::generate(&ctx.env);
    let result = ctx
        .client
        .try_unpause_feature(&non_admin, &PauseScope::Tipping);
    assert_eq!(
        result.err().unwrap().unwrap(),
        TipJarError::Unauthorized.into()
    );
}

#[test]
fn test_pause_and_unpause_feature_roundtrip() {
    let ctx = Ctx::new();
    ctx.client.pause_feature(
        &ctx.admin,
        &PauseScope::Tipping,
        &ctx.reason("security review"),
    );
    assert!(ctx.client.is_feature_paused(&PauseScope::Tipping));
    ctx.client.unpause_feature(&ctx.admin, &PauseScope::Tipping);
    assert!(!ctx.client.is_feature_paused(&PauseScope::Tipping));
}

// ── scopes are independent ────────────────────────────────────────────────────

#[test]
fn test_scopes_are_independent() {
    let ctx = Ctx::new();
    ctx.client
        .pause_feature(&ctx.admin, &PauseScope::Withdrawals, &ctx.reason("test"));
    assert!(!ctx.client.is_feature_paused(&PauseScope::Tipping));
    assert!(ctx.client.is_feature_paused(&PauseScope::Withdrawals));
    assert!(!ctx.client.is_feature_paused(&PauseScope::Subscriptions));
}

// ── Tipping scope ─────────────────────────────────────────────────────────────

#[test]
fn test_tip_blocked_when_tipping_scope_paused() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 1_000);
    ctx.client
        .pause_feature(&ctx.admin, &PauseScope::Tipping, &ctx.reason("exploit"));
    let result = ctx.client.try_tip(&sender, &creator, &ctx.token, &100);
    assert_eq!(
        result.err().unwrap().unwrap(),
        TipJarError::FeaturePaused.into()
    );
}

#[test]
fn test_tip_works_after_tipping_scope_unpaused() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 1_000);
    ctx.client
        .pause_feature(&ctx.admin, &PauseScope::Tipping, &ctx.reason("test"));
    ctx.client.unpause_feature(&ctx.admin, &PauseScope::Tipping);
    ctx.client.tip(&sender, &creator, &ctx.token, &100);
    assert_eq!(
        ctx.client.get_withdrawable_balance(&creator, &ctx.token),
        100
    );
}

// ── Withdrawals scope ─────────────────────────────────────────────────────────

#[test]
fn test_withdraw_blocked_when_withdrawals_scope_paused() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 1_000);
    ctx.client.tip(&sender, &creator, &ctx.token, &100);
    ctx.client
        .pause_feature(&ctx.admin, &PauseScope::Withdrawals, &ctx.reason("audit"));
    let result = ctx.client.try_withdraw(&creator, &ctx.token);
    assert_eq!(
        result.err().unwrap().unwrap(),
        TipJarError::FeaturePaused.into()
    );
}

#[test]
fn test_tip_still_works_when_only_withdrawals_paused() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 1_000);
    ctx.client
        .pause_feature(&ctx.admin, &PauseScope::Withdrawals, &ctx.reason("audit"));
    ctx.client.tip(&sender, &creator, &ctx.token, &200);
    assert_eq!(
        ctx.client.get_withdrawable_balance(&creator, &ctx.token),
        200
    );
}

// ── Subscriptions scope ───────────────────────────────────────────────────────

#[test]
fn test_create_subscription_blocked_when_subscriptions_scope_paused() {
    let ctx = Ctx::new();
    let subscriber = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.client.pause_feature(
        &ctx.admin,
        &PauseScope::Subscriptions,
        &ctx.reason("maintenance"),
    );
    let result =
        ctx.client
            .try_create_subscription(&subscriber, &creator, &ctx.token, &100, &86_400);
    assert_eq!(
        result.err().unwrap().unwrap(),
        TipJarError::FeaturePaused.into()
    );
}

// ── global pause still blocks everything ─────────────────────────────────────

#[test]
fn test_global_pause_blocks_even_when_no_feature_paused() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 1_000);
    ctx.client.pause(&ctx.admin, &ctx.reason("emergency"));
    assert!(!ctx.client.is_feature_paused(&PauseScope::Tipping));
    let result = ctx.client.try_tip(&sender, &creator, &ctx.token, &100);
    assert_eq!(
        result.err().unwrap().unwrap(),
        TipJarError::ContractPaused.into()
    );
}
