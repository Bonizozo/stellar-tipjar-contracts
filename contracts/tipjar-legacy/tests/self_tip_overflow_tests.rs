#![cfg(test)]

extern crate std;

use soroban_sdk::{testutils::Address as _, token, Address, Env};
use tipjar_legacy::{DataKey, TipJarContract, TipJarContractClient, TipJarError};

// ── helpers ───────────────────────────────────────────────────────────────────

struct Ctx {
    env: Env,
    client: TipJarContractClient<'static>,
    contract_id: Address,
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
            contract_id,
            token,
        }
    }

    fn mint(&self, to: &Address, amount: i128) {
        token::StellarAssetClient::new(&self.env, &self.token).mint(to, &amount);
    }

    /// Seeds the creator's escrowed balance directly via contract storage,
    /// bypassing `tip()` so the setup itself can't trip unrelated arithmetic
    /// elsewhere in the contract (e.g. reputation scoring) at extreme amounts.
    fn seed_creator_balance(&self, creator: &Address, amount: i128) {
        self.env.as_contract(&self.contract_id, || {
            self.env.storage().persistent().set(
                &DataKey::CreatorBalance(creator.clone(), self.token.clone()),
                &amount,
            );
        });
    }
}

// ── self-tip rejection ──────────────────────────────────────────────────────────

#[test]
fn test_self_tip_rejected() {
    let ctx = Ctx::new();
    let user = Address::generate(&ctx.env);
    ctx.mint(&user, 1_000);
    let result = ctx.client.try_tip(&user, &user, &ctx.token, &100);
    assert_eq!(result.err().unwrap().unwrap(), TipJarError::SelfTip.into());
}

#[test]
#[should_panic]
fn test_self_tip_panics() {
    let ctx = Ctx::new();
    let user = Address::generate(&ctx.env);
    ctx.mint(&user, 1_000);
    ctx.client.tip(&user, &user, &ctx.token, &100);
}

#[test]
fn test_self_tip_rejected_before_any_state_change() {
    let ctx = Ctx::new();
    let user = Address::generate(&ctx.env);
    ctx.mint(&user, 1_000);
    let _ = ctx.client.try_tip(&user, &user, &ctx.token, &100);
    // No balance should have been credited to the would-be self-tipper.
    assert_eq!(ctx.client.get_withdrawable_balance(&user, &ctx.token), 0);
}

// ── normal tipping is unaffected ─────────────────────────────────────────────

#[test]
fn test_tip_to_different_creator_still_works() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 1_000);
    ctx.client.tip(&sender, &creator, &ctx.token, &500);
    assert_eq!(
        ctx.client.get_withdrawable_balance(&creator, &ctx.token),
        500
    );
}

#[test]
fn test_zero_amount_still_rejected_as_invalid_amount() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    let result = ctx.client.try_tip(&sender, &creator, &ctx.token, &0);
    assert_eq!(
        result.err().unwrap().unwrap(),
        TipJarError::InvalidAmount.into()
    );
}

#[test]
fn test_negative_amount_still_rejected_as_invalid_amount() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    let result = ctx.client.try_tip(&sender, &creator, &ctx.token, &-100);
    assert_eq!(
        result.err().unwrap().unwrap(),
        TipJarError::InvalidAmount.into()
    );
}

// ── overflow-safe balance accumulation ───────────────────────────────────────

#[test]
fn test_creator_balance_overflow_rejected() {
    let ctx = Ctx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);

    // Seed the creator's balance right at the overflow boundary directly via
    // storage (going through `tip()` with an amount this large would itself
    // trip unrelated unchecked arithmetic in the reputation-scoring module).
    ctx.seed_creator_balance(&creator, i128::MAX - 1);
    ctx.mint(&sender, 10);

    // A tip of 2 or more must overflow when added to the seeded balance.
    let result = ctx.client.try_tip(&sender, &creator, &ctx.token, &2);
    assert_eq!(
        result.err().unwrap().unwrap(),
        TipJarError::BalanceOverflow.into()
    );
}
