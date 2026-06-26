#![cfg(test)]

extern crate std;

use soroban_sdk::{testutils::Address as _, token, Address, Env};
use tipjar_legacy::{TipJarContract, TipJarContractClient, TipJarError};

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

    fn balance_of(&self, who: &Address) -> i128 {
        token::Client::new(&self.env, &self.token).balance(who)
    }
}

// ── default state ─────────────────────────────────────────────────────────────

#[test]
fn test_default_fee_bps_is_zero() {
    let ctx = Ctx::new();
    assert_eq!(ctx.client.get_fee_bps(), 0);
}

#[test]
fn test_default_platform_fees_is_zero() {
    let ctx = Ctx::new();
    assert_eq!(ctx.client.get_platform_fees(&ctx.token), 0);
}

// ── set_fee_bps ────────────────────────────────────────────────────────────────

#[test]
fn test_set_fee_bps_requires_admin() {
    let ctx = Ctx::new();
    let non_admin = Address::generate(&ctx.env);
    let result = ctx.client.try_set_fee_bps(&non_admin, &100);
    assert_eq!(
        result.err().unwrap().unwrap(),
        TipJarError::Unauthorized.into()
    );
    assert_eq!(ctx.client.get_fee_bps(), 0);
}

#[test]
fn test_set_fee_bps_rejects_above_max() {
    let ctx = Ctx::new();
    let result = ctx.client.try_set_fee_bps(&ctx.admin, &1001);
    assert_eq!(
        result.err().unwrap().unwrap(),
        TipJarError::FeeBpsTooHigh.into()
    );
    assert_eq!(ctx.client.get_fee_bps(), 0);
}

#[test]
fn test_set_fee_bps_updates_value() {
    let ctx = Ctx::new();
    ctx.client.set_fee_bps(&ctx.admin, &250);
    assert_eq!(ctx.client.get_fee_bps(), 250);
}

// ── tip() fee deduction ────────────────────────────────────────────────────────

#[test]
fn test_tip_with_zero_fee_credits_full_amount() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 1_000);
    ctx.client.tip(&sender, &creator, &ctx.token, &1_000);
    assert_eq!(
        ctx.client.get_withdrawable_balance(&creator, &ctx.token),
        1_000
    );
    assert_eq!(ctx.client.get_platform_fees(&ctx.token), 0);
}

#[test]
fn test_tip_deducts_configured_fee_and_credits_remainder() {
    let ctx = Ctx::new();
    ctx.client.set_fee_bps(&ctx.admin, &500); // 5%
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 1_000);
    ctx.client.tip(&sender, &creator, &ctx.token, &1_000);

    // 5% of 1000 = 50 fee, 950 to the creator.
    assert_eq!(
        ctx.client.get_withdrawable_balance(&creator, &ctx.token),
        950
    );
    assert_eq!(ctx.client.get_platform_fees(&ctx.token), 50);
}

#[test]
fn test_platform_fees_accumulate_across_tips() {
    let ctx = Ctx::new();
    ctx.client.set_fee_bps(&ctx.admin, &1000); // 10%
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 10_000);
    ctx.client.tip(&sender, &creator, &ctx.token, &1_000);
    ctx.client.tip(&sender, &creator, &ctx.token, &2_000);

    assert_eq!(ctx.client.get_platform_fees(&ctx.token), 300); // 100 + 200
    assert_eq!(
        ctx.client.get_withdrawable_balance(&creator, &ctx.token),
        2_700
    ); // 900 + 1800
}

// ── withdraw_fees ──────────────────────────────────────────────────────────────

#[test]
fn test_withdraw_fees_requires_admin() {
    let ctx = Ctx::new();
    let non_admin = Address::generate(&ctx.env);
    let result = ctx.client.try_withdraw_fees(&non_admin, &ctx.token);
    assert_eq!(
        result.err().unwrap().unwrap(),
        TipJarError::Unauthorized.into()
    );
}

#[test]
fn test_withdraw_fees_fails_when_nothing_accumulated() {
    let ctx = Ctx::new();
    let result = ctx.client.try_withdraw_fees(&ctx.admin, &ctx.token);
    assert_eq!(
        result.err().unwrap().unwrap(),
        TipJarError::NoFeesToWithdraw.into()
    );
}

#[test]
fn test_withdraw_fees_transfers_balance_and_resets_to_zero() {
    let ctx = Ctx::new();
    ctx.client.set_fee_bps(&ctx.admin, &500); // 5%
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 1_000);
    ctx.client.tip(&sender, &creator, &ctx.token, &1_000);
    assert_eq!(ctx.client.get_platform_fees(&ctx.token), 50);

    let admin_balance_before = ctx.balance_of(&ctx.admin);
    let withdrawn = ctx.client.withdraw_fees(&ctx.admin, &ctx.token);

    assert_eq!(withdrawn, 50);
    assert_eq!(ctx.balance_of(&ctx.admin), admin_balance_before + 50);
    assert_eq!(ctx.client.get_platform_fees(&ctx.token), 0);

    // Withdrawing again with nothing accumulated fails.
    let result = ctx.client.try_withdraw_fees(&ctx.admin, &ctx.token);
    assert_eq!(
        result.err().unwrap().unwrap(),
        TipJarError::NoFeesToWithdraw.into()
    );
}
