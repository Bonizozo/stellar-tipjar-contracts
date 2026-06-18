#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};
use tipjar::{DataKey, Delegation, TipJarContract, TipJarContractClient, TipJarError};

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
    let token_id = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin, &0u32, &0u64);
    client.add_token(&admin, &token_id);

    let sender = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&sender, &1_000_000i128);

    (env, client, sender, Address::generate(&env), token_id)
}

#[test]
fn test_delegate_withdrawal_and_limit_tracking() {
    let (env, client, sender, creator, token) = setup();
    let delegate = Address::generate(&env);

    client.tip(&sender, &creator, &token, &500i128);
    client.delegate_withdrawal(&creator, &delegate, &300i128, &1_000u64);

    let delegation: Delegation = client.get_delegation(&creator, &delegate).unwrap();
    assert_eq!(delegation.max_amount, 300i128);
    assert!(delegation.active);
    assert_eq!(delegation.used_amount, 0);

    client.withdraw_as_delegate(&delegate, &creator, &token, &200i128);

    assert_eq!(client.get_withdrawable_balance(&creator, &token), 300i128);
    assert_eq!(
        soroban_sdk::token::StellarAssetClient::new(&env, &token).balance(&delegate),
        200i128
    );

    let delegation = client.get_delegation(&creator, &delegate).unwrap();
    assert_eq!(delegation.used_amount, 200i128);
    assert!(delegation.active);

    client.withdraw_as_delegate(&delegate, &creator, &token, &100i128);
    let delegation = client.get_delegation(&creator, &delegate).unwrap();
    assert_eq!(delegation.used_amount, 300i128);
    assert!(!delegation.active);

    let delegates = client.get_delegates(&creator);
    assert_eq!(delegates.len(), 0);

    let history = client.get_delegation_history(&creator);
    assert_eq!(history.len(), 3); // creation + two updates
}

#[test]
fn test_delegate_revocation_blocks_withdrawal() {
    let (env, client, sender, creator, token) = setup();
    let delegate = Address::generate(&env);

    client.tip(&sender, &creator, &token, &500i128);
    client.delegate_withdrawal(&creator, &delegate, &300i128, &1_000u64);
    client.revoke_delegation(&creator, &delegate);

    let result = client.try_withdraw_as_delegate(&delegate, &creator, &token, &100i128);
    assert_eq!(result, Err(Ok(TipJarError::DelegationInactive)));
}

#[test]
fn test_delegate_expiry_rejects_after_duration() {
    let (env, client, sender, creator, token) = setup();
    let delegate = Address::generate(&env);

    client.tip(&sender, &creator, &token, &500i128);
    client.delegate_withdrawal(&creator, &delegate, &300i128, &100u64);

    env.ledger().with_mut(|ledger| ledger.timestamp += 101);

    let result = client.try_withdraw_as_delegate(&delegate, &creator, &token, &100i128);
    assert_eq!(result, Err(Ok(TipJarError::DelegationExpired)));
}
