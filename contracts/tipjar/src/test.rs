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
    token, vec, Address, Env, IntoVal,
};

struct Ctx {
    env: Env,
    contract_id: Address,
    token: Address,
}

impl Ctx {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let token_admin = Address::generate(&env);
        let token = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();

        let contract_id = env.register(TipJar, ());
        let client = TipJarClient::new(&env, &contract_id);
        client.init(&token);

        Ctx {
            env,
            contract_id,
            token,
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
    ctx.client().withdraw(&creator);
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

    ctx.client().withdraw(&creator);
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
    ctx.client().withdraw(&creator);

    assert_eq!(ctx.token_client().balance(&creator), 700);
    assert_eq!(ctx.token_client().balance(&ctx.contract_id), 0);
    assert_eq!(ctx.client().get_total_tips(&creator), 700);

    // Withdrawable balance is now zero: a second withdraw must fail.
    let err = ctx.client().try_withdraw(&creator).unwrap_err().unwrap();
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

    let err = ctx.client().try_init(&ctx.token).unwrap_err().unwrap();
    assert_eq!(err, Error::AlreadyInitialized.into());
}

#[test]
fn withdraw_with_nothing_to_withdraw_errors() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);

    let err = ctx.client().try_withdraw(&creator).unwrap_err().unwrap();
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
    ctx.client().withdraw(&creator);

    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (symbol_short!("withdraw"), creator.clone()).into_val(&ctx.env),
                (250i128,).into_val(&ctx.env),
            ),
        ]
    );
}

#[cfg(test)]
mod fixtures {
    use super::*;
    use soroban_sdk::{xdr::ToXdr, Address, Env, IntoVal, String as SorobanString};
    use std::{fs, path::PathBuf};

    #[test]
    fn event_schema_golden_fixtures() {
        let env = Env::default();
        env.mock_all_auths();
        
        let contract_id = env.register(TipJar, ());
        let client = TipJarClient::new(&env, &contract_id);
        
        let token_admin = Address::generate(&env);
        let token = env.register_stellar_asset_contract_v2(token_admin).address();
        client.init(&token);

        let creator = Address::from_string(&SorobanString::from_str(&env, "GBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWH"));
        let sender = Address::from_string(&SorobanString::from_str(&env, "GCAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWG"));
        
        token::StellarAssetClient::new(&env, &token).mint(&sender, &1_000);

        client.tip(&sender, &creator, &250);
        client.withdraw(&creator);

        let events = env.events().all().filter_by_contract(&contract_id);
        let tip_event = events.get(0).unwrap();
        let withdraw_event = events.get(1).unwrap();

        let tip_topics_val: soroban_sdk::xdr::ScVec = tip_event.1.try_into_val(&env).unwrap();
        let tip_data_val: soroban_sdk::xdr::ScVal = tip_event.2.try_into_val(&env).unwrap();

        let withdraw_topics_val: soroban_sdk::xdr::ScVec = withdraw_event.1.try_into_val(&env).unwrap();
        let withdraw_data_val: soroban_sdk::xdr::ScVal = withdraw_event.2.try_into_val(&env).unwrap();

        assert_fixture("tip_topics", &tip_topics_val.to_xdr().unwrap());
        assert_fixture("tip_data", &tip_data_val.to_xdr().unwrap());
        assert_fixture("withdraw_topics", &withdraw_topics_val.to_xdr().unwrap());
        assert_fixture("withdraw_data", &withdraw_data_val.to_xdr().unwrap());
    }

    fn assert_fixture(name: &str, actual_xdr: &[u8]) {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let fixture_path = PathBuf::from(manifest_dir).join("tests").join("fixtures").join(format!("{}.xdr", name));

        if std::env::var("UPDATE_FIXTURES").is_ok() {
            fs::create_dir_all(fixture_path.parent().unwrap()).unwrap();
            fs::write(&fixture_path, actual_xdr).unwrap();
        } else {
            let expected_xdr = fs::read(&fixture_path).expect(&format!("Fixture missing: {:?}. Run with UPDATE_FIXTURES=1", fixture_path));
            assert_eq!(expected_xdr, actual_xdr, "Fixture {} mismatch! Run with UPDATE_FIXTURES=1 to update.", name);
        }
    }
}
