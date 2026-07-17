#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};
use tipjar::{CreditConfig, TipJarContract, TipJarContractClient};

fn setup() -> (
    Env,
    TipJarContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TipJarContract);
    let client = TipJarContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);

    // Deploy a mock token.
    let token_id = env.register_stellar_asset_contract(token_admin.clone());

    // Init TipJar
    client.init(&admin);
    client.add_token(&admin, &token_id);

    // Fund sender with tokens.
    let sender = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&sender, &1_000_000i128);

    (env, client, admin, sender, token_id)
}

fn provide_lending_liquidity(
    env: &Env,
    client: &TipJarContractClient<'static>,
    admin: &Address,
    token: &Address,
    amount: i128,
) {
    // insurance_contribute routes premium into platform fee balance.
    client.insurance_set_config(
        admin,
        &1i128,
        &2_000_000i128,
        &500u32,
        &8_000u32,
        &0u64,
        &0u32,
        &0u32,
    );
    let funder = Address::generate(env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(env, token);
    token_client.mint(&funder, &(amount + 10_000));
    client.insurance_contribute(&funder, token, &amount);
}

#[test]
fn test_credit_limit_and_borrow() {
    let (env, client, admin, sender, token) = setup();
    let creator = Address::generate(&env);

    let config = CreditConfig {
        max_credit_ratio_bps: 2000, // 20%
        interest_rate_bps: 100,     // 1% per 30 days
        min_total_tips: 1000,
        repayment_share_bps: 5000, // 50%
        enabled: true,
    };
    client.set_credit_config(&admin, &config);

    client.tip(&sender, &creator, &token, &1000i128);
    assert_eq!(client.get_credit_limit(&creator, &token), 200i128);

    // Borrow fails first because there is no lending liquidity.
    assert!(client.try_borrow(&creator, &token, &100i128).is_err());

    provide_lending_liquidity(&env, &client, &admin, &token, 1_000i128);
    client.borrow(&creator, &token, &150i128);
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 1150i128);

    let account = client.get_credit_account(&creator, &token).unwrap();
    assert_eq!(account.principal, 150i128);
    assert_eq!(account.total_borrowed, 150i128);

    // 150 already borrowed, remaining limit is 50.
    assert!(client.try_borrow(&creator, &token, &51i128).is_err());
    client.borrow(&creator, &token, &50i128);
    let account2 = client.get_credit_account(&creator, &token).unwrap();
    assert_eq!(account2.principal, 200i128);
}

#[test]
fn test_interest_accrual_and_manual_repayment() {
    let (env, client, admin, sender, token) = setup();
    let creator = Address::generate(&env);

    let config = CreditConfig {
        max_credit_ratio_bps: 5000, // 50%
        interest_rate_bps: 1000,    // 10% per 30 days
        min_total_tips: 0,
        repayment_share_bps: 0, // disable auto repayment for this test
        enabled: true,
    };
    client.set_credit_config(&admin, &config);

    // Create tip history so there's available limit and creator balance for repayment.
    client.tip(&sender, &creator, &token, &2_000i128);
    provide_lending_liquidity(&env, &client, &admin, &token, 2_000i128);
    client.borrow(&creator, &token, &1_000i128);

    // Move time forward to accrue interest.
    env.ledger().with_mut(|li| li.timestamp += 15 * 24 * 3600);
    client.repay_credit(&creator, &token, &100i128);

    let account = client.get_credit_account(&creator, &token).unwrap();
    // 10% / 30d over 15d on 1000 principal => ~50 interest accrued.
    // First repayment portion should clear interest then principal.
    assert_eq!(account.interest_accrued, 0);
    assert_eq!(account.principal, 950i128);
    assert_eq!(account.total_repaid, 100i128);
}

#[test]
fn test_auto_repayment_and_credit_history() {
    let (env, client, admin, sender, token) = setup();
    let creator = Address::generate(&env);

    let config = CreditConfig {
        max_credit_ratio_bps: 5000, // 50%
        interest_rate_bps: 0,
        min_total_tips: 0,
        repayment_share_bps: 5000, // 50%
        enabled: true,
    };
    client.set_credit_config(&admin, &config);

    provide_lending_liquidity(&env, &client, &admin, &token, 2_000i128);
    client.borrow(&creator, &token, &400i128);

    // A 1000 tip credits creator by 1000, but auto repays 50% (500), capped at debt (400).
    client.tip(&sender, &creator, &token, &1_000i128);
    let account = client.get_credit_account(&creator, &token).unwrap();
    assert_eq!(account.principal, 0i128);
    assert_eq!(account.total_repaid, 400i128);

    // Creator should receive net 600 from the post-borrow tip.
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 1_000i128);

    let history = client.get_credit_history(&creator, &token);
    assert_eq!(history.len(), 2);
    assert!(!history.get(0u32).unwrap().is_repayment); // borrow
    assert!(history.get(1u32).unwrap().is_repayment); // auto-repay from tip
}
