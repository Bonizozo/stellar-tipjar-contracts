#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};
use tipjar::{
    composable_nft, did, reputation_tokens, verifiable_credentials,
};

// ── DID tests (#308) ──────────────────────────────────────────────────────────

#[test]
fn test_did_register() {
    let env = Env::default();
    let owner = Address::generate(&env);

    let id = did::register_did(
        &env,
        &owner,
        String::from_str(&env, "did:stellar:GABC"),
        did::DidMethod::Stellar,
    );

    assert_eq!(id, 0);
    let record = did::get_did(&env, id).unwrap();
    assert_eq!(record.owner, owner);
    assert!(record.active);
}

#[test]
fn test_did_register_multiple() {
    let env = Env::default();
    let owner = Address::generate(&env);

    let id0 = did::register_did(&env, &owner, String::from_str(&env, "did:web:example.com"), did::DidMethod::Web);
    let id1 = did::register_did(&env, &owner, String::from_str(&env, "did:key:z6Mk"), did::DidMethod::Key);

    assert_eq!(id0, 0);
    assert_eq!(id1, 1);

    let ids = did::get_owner_dids(&env, &owner);
    assert_eq!(ids.len(), 2);
}

#[test]
fn test_did_update() {
    let env = Env::default();
    let owner = Address::generate(&env);

    let id = did::register_did(&env, &owner, String::from_str(&env, "did:stellar:old"), did::DidMethod::Stellar);
    did::update_did(&env, &owner, id, String::from_str(&env, "did:stellar:new"));

    let record = did::get_did(&env, id).unwrap();
    assert_eq!(record.did_string, String::from_str(&env, "did:stellar:new"));
}

#[test]
fn test_did_add_and_get_claim() {
    let env = Env::default();
    let owner = Address::generate(&env);

    let did_id = did::register_did(&env, &owner, String::from_str(&env, "did:stellar:GABC"), did::DidMethod::Stellar);
    let claim_id = did::add_claim(
        &env,
        did_id,
        String::from_str(&env, "email"),
        String::from_str(&env, "user@example.com"),
    );

    let claim = did::get_claim(&env, claim_id).unwrap();
    assert_eq!(claim.did_id, did_id);
    assert!(!claim.verified);

    let claim_ids = did::get_did_claims(&env, did_id);
    assert_eq!(claim_ids.len(), 1);
}

#[test]
fn test_did_verify_claim() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let verifier = Address::generate(&env);

    let did_id = did::register_did(&env, &owner, String::from_str(&env, "did:stellar:GABC"), did::DidMethod::Stellar);
    let claim_id = did::add_claim(&env, did_id, String::from_str(&env, "kyc"), String::from_str(&env, "passed"));

    did::verify_claim(&env, &verifier, claim_id);

    let claim = did::get_claim(&env, claim_id).unwrap();
    assert!(claim.verified);
    assert_eq!(claim.verifier, Some(verifier));
}

// ── Verifiable Credentials tests (#309) ──────────────────────────────────────

#[test]
fn test_vc_issue() {
    let env = Env::default();
    let issuer = Address::generate(&env);
    let subject = Address::generate(&env);

    let id = verifiable_credentials::issue_credential(
        &env,
        &issuer,
        &subject,
        String::from_str(&env, "TipAchievement"),
        String::from_str(&env, r#"{"level":"gold"}"#),
        None,
    );

    assert_eq!(id, 0);
    let vc = verifiable_credentials::get_credential(&env, id).unwrap();
    assert_eq!(vc.issuer, issuer);
    assert_eq!(vc.subject, subject);
    assert_eq!(vc.status, verifiable_credentials::CredentialStatus::Active);
}

#[test]
fn test_vc_verify_active() {
    let env = Env::default();
    let issuer = Address::generate(&env);
    let subject = Address::generate(&env);

    let id = verifiable_credentials::issue_credential(
        &env,
        &issuer,
        &subject,
        String::from_str(&env, "schema"),
        String::from_str(&env, "data"),
        None,
    );

    assert!(verifiable_credentials::verify_credential(&env, id));
}

#[test]
fn test_vc_verify_expired() {
    let env = Env::default();
    let issuer = Address::generate(&env);
    let subject = Address::generate(&env);

    // Issue with expiry in the past
    let id = verifiable_credentials::issue_credential(
        &env,
        &issuer,
        &subject,
        String::from_str(&env, "schema"),
        String::from_str(&env, "data"),
        Some(1), // expires at timestamp 1 (already past)
    );

    // Advance time
    env.ledger().with_mut(|l| l.timestamp = 1000);

    assert!(!verifiable_credentials::verify_credential(&env, id));
}

#[test]
fn test_vc_revoke() {
    let env = Env::default();
    let issuer = Address::generate(&env);
    let subject = Address::generate(&env);

    let id = verifiable_credentials::issue_credential(
        &env,
        &issuer,
        &subject,
        String::from_str(&env, "schema"),
        String::from_str(&env, "data"),
        None,
    );

    verifiable_credentials::revoke_credential(&env, &issuer, id);

    let vc = verifiable_credentials::get_credential(&env, id).unwrap();
    assert_eq!(vc.status, verifiable_credentials::CredentialStatus::Revoked);
    assert!(!verifiable_credentials::verify_credential(&env, id));
}

#[test]
fn test_vc_subject_and_issuer_lists() {
    let env = Env::default();
    let issuer = Address::generate(&env);
    let subject = Address::generate(&env);

    verifiable_credentials::issue_credential(&env, &issuer, &subject, String::from_str(&env, "s"), String::from_str(&env, "d"), None);
    verifiable_credentials::issue_credential(&env, &issuer, &subject, String::from_str(&env, "s2"), String::from_str(&env, "d2"), None);

    let subject_creds = verifiable_credentials::get_subject_credentials(&env, &subject);
    assert_eq!(subject_creds.len(), 2);

    let issuer_creds = verifiable_credentials::get_issuer_credentials(&env, &issuer);
    assert_eq!(issuer_creds.len(), 2);
}

// ── Reputation Tokens tests (#310) ───────────────────────────────────────────

#[test]
fn test_rep_mint_increases_score() {
    let env = Env::default();
    let account = Address::generate(&env);

    reputation_tokens::mint(&env, &account, 1_000_000);

    let score = reputation_tokens::get_score(&env, &account);
    assert!(score > 0);
}

#[test]
fn test_rep_mint_accumulates() {
    let env = Env::default();
    let account = Address::generate(&env);

    reputation_tokens::mint(&env, &account, 1_000_000);
    reputation_tokens::mint(&env, &account, 1_000_000);

    let token = reputation_tokens::get_token(&env, &account);
    assert!(token.total_minted > 0);
}

#[test]
fn test_rep_decay_reduces_score() {
    let env = Env::default();
    let account = Address::generate(&env);

    reputation_tokens::mint(&env, &account, 10_000_000);
    let score_before = reputation_tokens::get_score(&env, &account);

    // Advance time by one half-life
    env.ledger().with_mut(|l| l.timestamp = reputation_tokens::HALF_LIFE_SECS);

    reputation_tokens::decay(&env, &account);
    let score_after = reputation_tokens::get_score(&env, &account);

    assert!(score_after < score_before);
}

#[test]
fn test_rep_apply_decay_zero_elapsed() {
    assert_eq!(reputation_tokens::apply_decay(1_000_000, 0), 1_000_000);
}

#[test]
fn test_rep_apply_decay_one_half_life() {
    let decayed = reputation_tokens::apply_decay(2_000_000, reputation_tokens::HALF_LIFE_SECS);
    assert_eq!(decayed, 1_000_000);
}

#[test]
fn test_rep_history_recorded() {
    let env = Env::default();
    let account = Address::generate(&env);

    reputation_tokens::mint(&env, &account, 1_000_000);

    let history = reputation_tokens::get_history(&env, &account);
    assert!(history.len() > 0);
}

#[test]
fn test_rep_no_transfer() {
    // Reputation tokens are non-transferable: there is no transfer function.
    // This test verifies that minting one account does not affect another.
    let env = Env::default();
    let a = Address::generate(&env);
    let b = Address::generate(&env);

    reputation_tokens::mint(&env, &a, 5_000_000);

    let score_b = reputation_tokens::get_score(&env, &b);
    assert_eq!(score_b, 0);
}

// ── Composable NFTs tests (#311) ──────────────────────────────────────────────

#[test]
fn test_nft_mint() {
    let env = Env::default();
    let owner = Address::generate(&env);

    let id = composable_nft::mint(&env, &owner, String::from_str(&env, r#"{"tip":100}"#));

    assert_eq!(id, 0);
    let nft = composable_nft::get_nft(&env, id).unwrap();
    assert_eq!(nft.owner, owner);
    assert!(nft.active);
    assert!(nft.parent_id.is_none());
}

#[test]
fn test_nft_owner_list() {
    let env = Env::default();
    let owner = Address::generate(&env);

    composable_nft::mint(&env, &owner, String::from_str(&env, "a"));
    composable_nft::mint(&env, &owner, String::from_str(&env, "b"));

    let ids = composable_nft::get_owner_nfts(&env, &owner);
    assert_eq!(ids.len(), 2);
}

#[test]
fn test_nft_compose() {
    let env = Env::default();
    let owner = Address::generate(&env);

    let parent_id = composable_nft::mint(&env, &owner, String::from_str(&env, "parent"));
    let child_id = composable_nft::mint(&env, &owner, String::from_str(&env, "child"));

    composable_nft::compose(&env, &owner, parent_id, child_id);

    let child = composable_nft::get_nft(&env, child_id).unwrap();
    assert_eq!(child.parent_id, Some(parent_id));

    let children = composable_nft::get_children(&env, parent_id);
    assert_eq!(children.len(), 1);
    assert_eq!(children.get(0).unwrap(), child_id);
}

#[test]
fn test_nft_decompose() {
    let env = Env::default();
    let owner = Address::generate(&env);

    let parent_id = composable_nft::mint(&env, &owner, String::from_str(&env, "parent"));
    let child_id = composable_nft::mint(&env, &owner, String::from_str(&env, "child"));

    composable_nft::compose(&env, &owner, parent_id, child_id);
    composable_nft::decompose(&env, &owner, child_id);

    let child = composable_nft::get_nft(&env, child_id).unwrap();
    assert!(child.parent_id.is_none());

    let children = composable_nft::get_children(&env, parent_id);
    assert_eq!(children.len(), 0);
}

#[test]
fn test_nft_composition_history() {
    let env = Env::default();
    let owner = Address::generate(&env);

    let parent_id = composable_nft::mint(&env, &owner, String::from_str(&env, "parent"));
    let child_id = composable_nft::mint(&env, &owner, String::from_str(&env, "child"));

    composable_nft::compose(&env, &owner, parent_id, child_id);
    composable_nft::decompose(&env, &owner, child_id);

    let history = composable_nft::get_composition_history(&env, parent_id);
    assert_eq!(history.len(), 2);
    assert!(!history.get(0).unwrap().is_decompose);
    assert!(history.get(1).unwrap().is_decompose);
}

#[test]
fn test_nft_nested_composition() {
    let env = Env::default();
    let owner = Address::generate(&env);

    let grandparent = composable_nft::mint(&env, &owner, String::from_str(&env, "gp"));
    let parent = composable_nft::mint(&env, &owner, String::from_str(&env, "p"));
    let child = composable_nft::mint(&env, &owner, String::from_str(&env, "c"));

    composable_nft::compose(&env, &owner, grandparent, parent);
    composable_nft::compose(&env, &owner, parent, child);

    let p = composable_nft::get_nft(&env, parent).unwrap();
    assert_eq!(p.parent_id, Some(grandparent));

    let c = composable_nft::get_nft(&env, child).unwrap();
    assert_eq!(c.parent_id, Some(parent));
}
