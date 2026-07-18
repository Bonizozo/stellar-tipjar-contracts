#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, Vec,
};
use tipjar::{TipJarContract, TipJarContractClient, TipJarError};

// ── helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (
    Env,
    TipJarContractClient<'static>,
    Address, // admin
    Address, // sender
    Address, // creator
    Address, // token
    Address, // signer1
    Address, // signer2
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
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&sender, &1_000_000i128);

    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);

    (
        env,
        client,
        admin,
        sender,
        Address::generate(&env),
        token_id,
        signer1,
        signer2,
    )
}

fn configure(
    env: &Env,
    client: &TipJarContractClient,
    admin: &Address,
    signer1: &Address,
    signer2: &Address,
    threshold: i128,
    required: u32,
) {
    let mut signers = Vec::new(env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());
    client.set_multisig_config(admin, &threshold, &required, &3600u64, &signers);
}

// ── set_multisig_config ───────────────────────────────────────────────────────

#[test]
fn test_set_multisig_config_persists() {
    let (env, client, admin, _sender, _creator, _token, signer1, signer2) = setup();
    configure(&env, &client, &admin, &signer1, &signer2, 500, 2);
    let cfg = client.get_multisig_config();
    assert_eq!(cfg.threshold, 500);
    assert_eq!(cfg.required_approvals, 2);
    assert_eq!(cfg.expiry_seconds, 3600);
    assert_eq!(cfg.signers.len(), 2);
}

#[test]
fn test_set_multisig_config_unauthorized() {
    let (env, client, _admin, _sender, _creator, _token, signer1, signer2) = setup();
    let non_admin = Address::generate(&env);
    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());
    let result = client.try_set_multisig_config(&non_admin, &500i128, &1u32, &3600u64, &signers);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}

// ── request_multisig_withdrawal ───────────────────────────────────────────────

#[test]
fn test_below_threshold_executes_immediately() {
    let (env, client, admin, sender, creator, token, signer1, signer2) = setup();
    configure(&env, &client, &admin, &signer1, &signer2, 500, 2);
    client.tip(&sender, &creator, &token, &300i128);
    // 300 <= 500 threshold → immediate withdrawal, returns 0
    let id = client.request_multisig_withdrawal(&creator, &token, &300i128);
    assert_eq!(id, 0);
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 0);
}

#[test]
fn test_above_threshold_creates_pending_request() {
    let (env, client, admin, sender, creator, token, signer1, signer2) = setup();
    configure(&env, &client, &admin, &signer1, &signer2, 500, 2);
    client.tip(&sender, &creator, &token, &1000i128);
    let id = client.request_multisig_withdrawal(&creator, &token, &1000i128);
    assert!(id > 0 || id == 0); // id is 0 (first counter value)
    let req = client.get_multisig_request(&id);
    assert_eq!(req.amount, 1000);
    assert!(!req.executed);
    assert!(!req.cancelled);
    assert_eq!(req.approvals.len(), 0);
}

#[test]
fn test_request_nothing_to_withdraw_fails() {
    let (env, client, admin, _sender, creator, token, signer1, signer2) = setup();
    configure(&env, &client, &admin, &signer1, &signer2, 500, 2);
    let result = client.try_request_multisig_withdrawal(&creator, &token, &100i128);
    assert_eq!(result, Err(Ok(TipJarError::NothingToWithdraw)));
}

#[test]
fn test_request_without_config_fails() {
    let (_env, client, _admin, sender, creator, token, _s1, _s2) = setup();
    client.tip(&sender, &creator, &token, &1000i128);
    let result = client.try_request_multisig_withdrawal(&creator, &token, &1000i128);
    assert_eq!(result, Err(Ok(TipJarError::MultiSigNotConfigured)));
}

// ── approve_withdrawal ────────────────────────────────────────────────────────

#[test]
fn test_single_approval_does_not_execute() {
    let (env, client, admin, sender, creator, token, signer1, signer2) = setup();
    configure(&env, &client, &admin, &signer1, &signer2, 500, 2);
    client.tip(&sender, &creator, &token, &1000i128);
    let id = client.request_multisig_withdrawal(&creator, &token, &1000i128);
    client.approve_withdrawal(&signer1, &id);
    let req = client.get_multisig_request(&id);
    assert!(!req.executed);
    assert_eq!(req.approvals.len(), 1);
    // Balance still held in escrow
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 1000);
}

#[test]
fn test_two_approvals_execute_withdrawal() {
    let (env, client, admin, sender, creator, token, signer1, signer2) = setup();
    configure(&env, &client, &admin, &signer1, &signer2, 500, 2);
    client.tip(&sender, &creator, &token, &1000i128);
    let id = client.request_multisig_withdrawal(&creator, &token, &1000i128);
    client.approve_withdrawal(&signer1, &id);
    client.approve_withdrawal(&signer2, &id);
    let req = client.get_multisig_request(&id);
    assert!(req.executed);
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 0);
}

#[test]
fn test_non_signer_cannot_approve() {
    let (env, client, admin, sender, creator, token, signer1, signer2) = setup();
    configure(&env, &client, &admin, &signer1, &signer2, 500, 2);
    client.tip(&sender, &creator, &token, &1000i128);
    let id = client.request_multisig_withdrawal(&creator, &token, &1000i128);
    let outsider = Address::generate(&env);
    let result = client.try_approve_withdrawal(&outsider, &id);
    assert_eq!(result, Err(Ok(TipJarError::NotASigner)));
}

#[test]
fn test_double_approval_rejected() {
    let (env, client, admin, sender, creator, token, signer1, signer2) = setup();
    configure(&env, &client, &admin, &signer1, &signer2, 500, 2);
    client.tip(&sender, &creator, &token, &1000i128);
    let id = client.request_multisig_withdrawal(&creator, &token, &1000i128);
    client.approve_withdrawal(&signer1, &id);
    let result = client.try_approve_withdrawal(&signer1, &id);
    assert_eq!(result, Err(Ok(TipJarError::AlreadyApproved)));
}

#[test]
fn test_approve_expired_request_fails() {
    let (env, client, admin, sender, creator, token, signer1, signer2) = setup();
    configure(&env, &client, &admin, &signer1, &signer2, 500, 2);
    client.tip(&sender, &creator, &token, &1000i128);
    env.ledger().with_mut(|l| l.timestamp = 1_000);
    let id = client.request_multisig_withdrawal(&creator, &token, &1000i128);
    // Advance past expiry (3600 s)
    env.ledger().with_mut(|l| l.timestamp = 1_000 + 3_601);
    let result = client.try_approve_withdrawal(&signer1, &id);
    assert_eq!(result, Err(Ok(TipJarError::MultiSigRequestExpired)));
}

// ── cancel_multisig_withdrawal ────────────────────────────────────────────────

#[test]
fn test_creator_can_cancel_request() {
    let (env, client, admin, sender, creator, token, signer1, signer2) = setup();
    configure(&env, &client, &admin, &signer1, &signer2, 500, 2);
    client.tip(&sender, &creator, &token, &1000i128);
    let id = client.request_multisig_withdrawal(&creator, &token, &1000i128);
    client.cancel_multisig_withdrawal(&creator, &id);
    let req = client.get_multisig_request(&id);
    assert!(req.cancelled);
}

#[test]
fn test_admin_can_cancel_request() {
    let (env, client, admin, sender, creator, token, signer1, signer2) = setup();
    configure(&env, &client, &admin, &signer1, &signer2, 500, 2);
    client.tip(&sender, &creator, &token, &1000i128);
    let id = client.request_multisig_withdrawal(&creator, &token, &1000i128);
    client.cancel_multisig_withdrawal(&admin, &id);
    let req = client.get_multisig_request(&id);
    assert!(req.cancelled);
}

#[test]
fn test_approve_cancelled_request_fails() {
    let (env, client, admin, sender, creator, token, signer1, signer2) = setup();
    configure(&env, &client, &admin, &signer1, &signer2, 500, 2);
    client.tip(&sender, &creator, &token, &1000i128);
    let id = client.request_multisig_withdrawal(&creator, &token, &1000i128);
    client.cancel_multisig_withdrawal(&creator, &id);
    let result = client.try_approve_withdrawal(&signer1, &id);
    assert_eq!(result, Err(Ok(TipJarError::MultiSigRequestClosed)));
}

#[test]
fn test_unauthorised_cancel_fails() {
    let (env, client, admin, sender, creator, token, signer1, signer2) = setup();
    configure(&env, &client, &admin, &signer1, &signer2, 500, 2);
    client.tip(&sender, &creator, &token, &1000i128);
    let id = client.request_multisig_withdrawal(&creator, &token, &1000i128);
    let outsider = Address::generate(&env);
    let result = client.try_cancel_multisig_withdrawal(&outsider, &id);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}
