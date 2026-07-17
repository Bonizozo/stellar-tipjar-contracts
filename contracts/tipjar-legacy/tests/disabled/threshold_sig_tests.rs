#![cfg(test)]

extern crate std;

use soroban_sdk::{testutils::Address as _, vec, Address, Env, Vec};
use tipjar::{
    threshold_sig::ThresholdTipStatus, ThresholdError, TipJarContract, TipJarContractClient,
    TipJarError,
};

fn setup() -> (Env, TipJarContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TipJarContract);
    let client = TipJarContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin, &0u32, &0u64);
    client.add_token(&admin, &token_id);

    (env, client, admin, token_id)
}

fn make_signers(env: &Env, n: u32) -> Vec<Address> {
    let mut v = Vec::new(env);
    for _ in 0..n {
        v.push_back(Address::generate(env));
    }
    v
}

// ── policy ────────────────────────────────────────────────────────────────────

#[test]
fn test_create_policy() {
    let (env, client, _admin, _token) = setup();
    let signers = make_signers(&env, 3);
    let owner = signers.get(0).unwrap();

    let policy_id = client.create_threshold_policy(&owner, &signers, &2u32);
    assert_eq!(policy_id, 0);

    let policy = client.get_threshold_policy(&policy_id).unwrap();
    assert_eq!(policy.threshold, 2);
    assert_eq!(policy.signers.len(), 3);
}

#[test]
fn test_create_policy_invalid_threshold() {
    let (env, client, _admin, _token) = setup();
    let signers = make_signers(&env, 2);
    let owner = signers.get(0).unwrap();

    // threshold > signers
    let r = client.try_create_threshold_policy(&owner, &signers, &5u32);
    assert_eq!(r, Err(Ok(ThresholdError::InvalidThreshold)));

    // threshold = 0
    let r = client.try_create_threshold_policy(&owner, &signers, &0u32);
    assert_eq!(r, Err(Ok(ThresholdError::InvalidThreshold)));
}

#[test]
fn test_create_policy_empty_signers() {
    let (env, client, _admin, _token) = setup();
    let owner = Address::generate(&env);
    let empty: Vec<Address> = Vec::new(&env);

    let r = client.try_create_threshold_policy(&owner, &empty, &1u32);
    assert_eq!(r, Err(Ok(ThresholdError::EmptySigner)));
}

// ── propose ───────────────────────────────────────────────────────────────────

#[test]
fn test_propose_tip() {
    let (env, client, _admin, token) = setup();
    let signers = make_signers(&env, 3);
    let owner = signers.get(0).unwrap();
    let creator = Address::generate(&env);

    let policy_id = client.create_threshold_policy(&owner, &signers, &2u32);
    let tip_id = client.propose_threshold_tip(&owner, &policy_id, &creator, &token, &100i128);

    let tip = client.get_threshold_tip(&tip_id).unwrap();
    assert_eq!(tip.amount, 100);
    assert_eq!(tip.approvals.len(), 1); // proposer auto-signs
    assert_eq!(tip.status, ThresholdTipStatus::Pending);
}

#[test]
fn test_propose_tip_non_signer_rejected() {
    let (env, client, _admin, token) = setup();
    let signers = make_signers(&env, 2);
    let owner = signers.get(0).unwrap();
    let stranger = Address::generate(&env);
    let creator = Address::generate(&env);

    let policy_id = client.create_threshold_policy(&owner, &signers, &1u32);
    let r = client.try_propose_threshold_tip(&stranger, &policy_id, &creator, &token, &100i128);
    assert_eq!(r, Err(Ok(ThresholdError::NotASigner)));
}

// ── partial sig ───────────────────────────────────────────────────────────────

#[test]
fn test_submit_partial_sig_reaches_threshold() {
    let (env, client, _admin, token) = setup();
    let signers = make_signers(&env, 3);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let creator = Address::generate(&env);

    let policy_id = client.create_threshold_policy(&s0, &signers, &2u32);
    let tip_id = client.propose_threshold_tip(&s0, &policy_id, &creator, &token, &100i128);

    // s0 already signed; s1 pushes it to threshold
    client.submit_partial_sig(&s1, &tip_id);

    let tip = client.get_threshold_tip(&tip_id).unwrap();
    assert_eq!(tip.status, ThresholdTipStatus::Approved);
    assert_eq!(tip.approvals.len(), 2);
}

#[test]
fn test_double_sign_rejected() {
    let (env, client, _admin, token) = setup();
    let signers = make_signers(&env, 3);
    let s0 = signers.get(0).unwrap();
    let creator = Address::generate(&env);

    let policy_id = client.create_threshold_policy(&s0, &signers, &3u32);
    let tip_id = client.propose_threshold_tip(&s0, &policy_id, &creator, &token, &100i128);

    let r = client.try_submit_partial_sig(&s0, &tip_id);
    assert_eq!(r, Err(Ok(ThresholdError::AlreadySigned)));
}

#[test]
fn test_non_signer_cannot_sign() {
    let (env, client, _admin, token) = setup();
    let signers = make_signers(&env, 2);
    let s0 = signers.get(0).unwrap();
    let stranger = Address::generate(&env);
    let creator = Address::generate(&env);

    let policy_id = client.create_threshold_policy(&s0, &signers, &2u32);
    let tip_id = client.propose_threshold_tip(&s0, &policy_id, &creator, &token, &100i128);

    let r = client.try_submit_partial_sig(&stranger, &tip_id);
    assert_eq!(r, Err(Ok(ThresholdError::NotASigner)));
}

// ── execute ───────────────────────────────────────────────────────────────────

#[test]
fn test_execute_approved_tip() {
    let (env, client, _admin, token) = setup();
    let signers = make_signers(&env, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let creator = Address::generate(&env);

    // Fund s0 so they can transfer on execute
    soroban_sdk::token::StellarAssetClient::new(&env, &token).mint(&s0, &500i128);

    let policy_id = client.create_threshold_policy(&s0, &signers, &2u32);
    let tip_id = client.propose_threshold_tip(&s0, &policy_id, &creator, &token, &100i128);
    client.submit_partial_sig(&s1, &tip_id);

    client.execute_threshold_tip(&s0, &tip_id);

    let tip = client.get_threshold_tip(&tip_id).unwrap();
    assert_eq!(tip.status, ThresholdTipStatus::Executed);

    assert_eq!(client.get_withdrawable_balance(&creator, &token), 100);
}

#[test]
fn test_execute_below_threshold_rejected() {
    let (env, client, _admin, token) = setup();
    let signers = make_signers(&env, 3);
    let s0 = signers.get(0).unwrap();
    let creator = Address::generate(&env);

    let policy_id = client.create_threshold_policy(&s0, &signers, &3u32);
    let tip_id = client.propose_threshold_tip(&s0, &policy_id, &creator, &token, &100i128);

    let r = client.try_execute_threshold_tip(&s0, &tip_id);
    assert_eq!(r, Err(Ok(ThresholdError::ThresholdNotMet)));
}

// ── cancel ────────────────────────────────────────────────────────────────────

#[test]
fn test_cancel_tip() {
    let (env, client, _admin, token) = setup();
    let signers = make_signers(&env, 2);
    let s0 = signers.get(0).unwrap();
    let creator = Address::generate(&env);

    let policy_id = client.create_threshold_policy(&s0, &signers, &2u32);
    let tip_id = client.propose_threshold_tip(&s0, &policy_id, &creator, &token, &100i128);

    client.cancel_threshold_tip(&s0, &tip_id);

    let tip = client.get_threshold_tip(&tip_id).unwrap();
    assert_eq!(tip.status, ThresholdTipStatus::Cancelled);
}

#[test]
fn test_non_proposer_cannot_cancel() {
    let (env, client, _admin, token) = setup();
    let signers = make_signers(&env, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let creator = Address::generate(&env);

    let policy_id = client.create_threshold_policy(&s0, &signers, &2u32);
    let tip_id = client.propose_threshold_tip(&s0, &policy_id, &creator, &token, &100i128);

    let r = client.try_cancel_threshold_tip(&s1, &tip_id);
    assert_eq!(r, Err(Ok(ThresholdError::Unauthorized)));
}
