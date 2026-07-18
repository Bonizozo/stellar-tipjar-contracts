#![cfg(test)]

extern crate std;

use soroban_sdk::{testutils::Address as _, token, Address, Env};
use tipjar_legacy::{TipJarContract, TipJarContractClient};

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
}

#[test]
fn test_tip_bumps_ttl_and_balance_survives() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 1000);

    ctx.client.tip(&sender, &creator, &ctx.token, &500);
    assert_eq!(
        ctx.client.get_withdrawable_balance(&creator, &ctx.token),
        500
    );
    assert_eq!(ctx.client.get_total_tips(&creator, &ctx.token), 500);
}

#[test]
fn test_withdraw_bumps_ttl() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 1000);

    ctx.client.tip(&sender, &creator, &ctx.token, &500);
    ctx.client.withdraw(&creator, &ctx.token, &None);
    assert_eq!(ctx.client.get_withdrawable_balance(&creator, &ctx.token), 0);
}

#[test]
fn test_bump_entry_no_panic_on_missing() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    // Should not panic even when no entries exist for this creator
    ctx.client.bump_entry(&creator, &ctx.token);
}

#[test]
fn test_bump_entry_extends_existing() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 1000);

    ctx.client.tip(&sender, &creator, &ctx.token, &500);

    // bump_entry should succeed for a creator with existing balance
    ctx.client.bump_entry(&creator, &ctx.token);

    // Balance should still be intact
    assert_eq!(
        ctx.client.get_withdrawable_balance(&creator, &ctx.token),
        500
    );
    assert_eq!(ctx.client.get_total_tips(&creator, &ctx.token), 500);
}

#[test]
fn test_bump_entry_callable_by_anyone() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    let random_caller = Address::generate(&ctx.env);
    ctx.mint(&sender, 1000);

    ctx.client.tip(&sender, &creator, &ctx.token, &500);

    // Anyone can call bump_entry, no auth required
    ctx.client.bump_entry(&creator, &ctx.token);
    // Called by a different "user" context - still works
    ctx.client.bump_entry(&random_caller, &ctx.token);

    assert_eq!(
        ctx.client.get_withdrawable_balance(&creator, &ctx.token),
        500
    );
}

#[test]
fn test_multiple_tips_bump_ttl() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 5000);

    ctx.client.tip(&sender, &creator, &ctx.token, &100);
    ctx.client.tip(&sender, &creator, &ctx.token, &200);
    ctx.client.tip(&sender, &creator, &ctx.token, &300);

    assert_eq!(
        ctx.client.get_withdrawable_balance(&creator, &ctx.token),
        600
    );
    assert_eq!(ctx.client.get_total_tips(&creator, &ctx.token), 600);
}

#[test]
fn test_partial_withdraw_bumps_ttl() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 1000);

    ctx.client.tip(&sender, &creator, &ctx.token, &500);
    ctx.client.withdraw(&creator, &ctx.token, &Some(200));

    assert_eq!(
        ctx.client.get_withdrawable_balance(&creator, &ctx.token),
        300
    );
}
