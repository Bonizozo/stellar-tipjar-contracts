#![cfg(test)]

use crate::{Error, TipJar, TipJarClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token, Address, Env,
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
        let admin = Address::generate(&env);
        let contract_id = env.register(TipJar, ());
        let client = TipJarClient::new(&env, &contract_id);
        client.init(&token, &admin, &1000);
        Ctx {
            env,
            contract_id,
            token,
        }
    }

    fn client(&self) -> TipJarClient<'_> {
        TipJarClient::new(&self.env, &self.contract_id)
    }

    fn fund(&self, amount: i128) -> Address {
        let holder = Address::generate(&self.env);
        token::StellarAssetClient::new(&self.env, &self.token).mint(&holder, &amount);
        holder
    }
}

#[test]
fn test_exhaustive_auth_matrix() {
    let ctx = Ctx::new();
    let client = ctx.client();
    let creator = Address::generate(&ctx.env);
    let operator = Address::generate(&ctx.env);
    let stranger = Address::generate(&ctx.env);
    let payout = Address::generate(&ctx.env);
    let other_address = Address::generate(&ctx.env);

    // Tip creator
    let sender = ctx.fund(1000);
    client.tip(&sender, &creator, &1000);

    // Set payout address
    client.set_payout_address(&creator, &payout);

    // Jump past delay
    ctx.env.ledger().with_mut(|li| li.sequence_number += 17280);

    // Matrix tests:
    // {creator, operator-within-allowance, operator-over-allowance, operator-after-expiry, operator-after-revocation, stranger}
    // × {withdraw-to-payout, attempt-withdraw-to-other-address}

    client.authorize_operator(
        &creator,
        &operator,
        &500,
        &(ctx.env.ledger().sequence() + 100),
    );

    // 1. stranger
    assert_eq!(
        client
            .try_withdraw(&stranger, &creator, &payout, &Some(10))
            .unwrap_err()
            .unwrap(),
        Error::InvalidOperator.into()
    );
    assert_eq!(
        client
            .try_withdraw(&stranger, &creator, &other_address, &Some(10))
            .unwrap_err()
            .unwrap(),
        Error::InvalidOperator.into()
    );

    // 2. operator-over-allowance
    assert_eq!(
        client
            .try_withdraw(&operator, &creator, &payout, &Some(600))
            .unwrap_err()
            .unwrap(),
        Error::InsufficientAllowance.into()
    );
    assert_eq!(
        client
            .try_withdraw(&operator, &creator, &other_address, &Some(600))
            .unwrap_err()
            .unwrap(),
        Error::InsufficientAllowance.into()
    );

    // 3. operator-within-allowance but to other address
    assert_eq!(
        client
            .try_withdraw(&operator, &creator, &other_address, &Some(100))
            .unwrap_err()
            .unwrap(),
        Error::InvalidTarget.into()
    );

    // 4. operator-within-allowance to payout
    client.withdraw(&operator, &creator, &payout, &Some(100)); // Success!

    // 5. operator-after-expiry
    ctx.env.ledger().with_mut(|li| li.sequence_number += 101);
    assert_eq!(
        client
            .try_withdraw(&operator, &creator, &payout, &Some(10))
            .unwrap_err()
            .unwrap(),
        Error::OperatorExpired.into()
    );
    assert_eq!(
        client
            .try_withdraw(&operator, &creator, &other_address, &Some(10))
            .unwrap_err()
            .unwrap(),
        Error::OperatorExpired.into()
    );

    // 6. operator-after-revocation
    ctx.env.ledger().with_mut(|li| li.sequence_number -= 101); // Go back to not be expired
    client.revoke_operator(&creator, &operator);
    assert_eq!(
        client
            .try_withdraw(&operator, &creator, &payout, &Some(10))
            .unwrap_err()
            .unwrap(),
        Error::InvalidOperator.into()
    );
    assert_eq!(
        client
            .try_withdraw(&operator, &creator, &other_address, &Some(10))
            .unwrap_err()
            .unwrap(),
        Error::InvalidOperator.into()
    );

    // 7. creator to other address
    assert_eq!(
        client
            .try_withdraw(&creator, &creator, &other_address, &Some(10))
            .unwrap_err()
            .unwrap(),
        Error::InvalidTarget.into()
    );

    // 8. creator to payout
    client.withdraw(&creator, &creator, &payout, &Some(10)); // Success!
}

#[test]
fn test_payout_delay_and_cancellation() {
    let ctx = Ctx::new();
    let client = ctx.client();
    let creator = Address::generate(&ctx.env);
    let payout = Address::generate(&ctx.env);
    let sender = ctx.fund(1000);

    client.tip(&sender, &creator, &1000);
    client.set_payout_address(&creator, &payout);

    // Before delay, old address (creator) is used
    client.withdraw(&creator, &creator, &creator, &Some(100));
    assert_eq!(
        token::TokenClient::new(&ctx.env, &ctx.token).balance(&creator),
        100
    );

    // Cancel payout change
    client.cancel_payout_address(&creator);

    // Jump past delay
    ctx.env.ledger().with_mut(|li| li.sequence_number += 17280);

    // It should still go to creator, since change was cancelled
    client.withdraw(&creator, &creator, &creator, &Some(100));
    assert_eq!(
        token::TokenClient::new(&ctx.env, &ctx.token).balance(&creator),
        200
    );

    // Set again and let it mature
    let new_payout = Address::generate(&ctx.env);
    client.set_payout_address(&creator, &new_payout);
    ctx.env.ledger().with_mut(|li| li.sequence_number += 17280);

    client.withdraw(&creator, &creator, &new_payout, &Some(100));
    assert_eq!(
        token::TokenClient::new(&ctx.env, &ctx.token).balance(&new_payout),
        100
    );
}

#[test]
fn test_partial_withdraw_edges() {
    let ctx = Ctx::new();
    let client = ctx.client();
    let creator = Address::generate(&ctx.env);
    let sender = ctx.fund(100);

    client.tip(&sender, &creator, &100);

    // 1 stroop
    client.withdraw(&creator, &creator, &creator, &Some(1));
    // amount == balance + 1
    assert_eq!(
        client
            .try_withdraw(&creator, &creator, &creator, &Some(100))
            .unwrap_err()
            .unwrap(),
        Error::InvalidAmount.into()
    );
    // amount == balance
    client.withdraw(&creator, &creator, &creator, &Some(99));

    // Test repeated partials
    let creator2 = Address::generate(&ctx.env);
    let sender2 = ctx.fund(100);
    client.tip(&sender2, &creator2, &100);
    client.withdraw(&creator2, &creator2, &creator2, &Some(20));
    client.withdraw(&creator2, &creator2, &creator2, &Some(30));
    client.withdraw(&creator2, &creator2, &creator2, &Some(50));

    assert_eq!(
        client
            .try_withdraw(&creator2, &creator2, &creator2, &Some(1))
            .unwrap_err()
            .unwrap(),
        Error::NothingToWithdraw.into()
    );
}

#[test]
fn test_ttl_resurrection_prevention() {
    // We can't strictly test TTL games without advanced testutils, but we can verify that the key is removed.
    let ctx = Ctx::new();
    let client = ctx.client();
    let creator = Address::generate(&ctx.env);
    let operator = Address::generate(&ctx.env);

    client.authorize_operator(&creator, &operator, &100, &1000);
    client.revoke_operator(&creator, &operator);

    // Key should be gone, meaning any try_withdraw as operator should fail with InvalidOperator
    let sender = ctx.fund(1000);
    client.tip(&sender, &creator, &1000);
    assert_eq!(
        client
            .try_withdraw(&operator, &creator, &creator, &Some(10))
            .unwrap_err()
            .unwrap(),
        Error::InvalidOperator.into()
    );
}
