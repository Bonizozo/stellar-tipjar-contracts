//! Decentralized Identity (DID) module (#308).
//!
//! Provides DID integration for creator verification and reputation.

use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Vec};

use crate::DataKey;

/// DID method supported by the contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DidMethod {
    Web,
    Key,
    Stellar,
}

/// A decentralized identity record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Did {
    pub id: u64,
    pub owner: Address,
    pub did_string: String,
    pub method: DidMethod,
    pub created_at: u64,
    pub updated_at: u64,
    pub active: bool,
}

/// An identity claim attached to a DID.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityClaim {
    pub claim_id: u64,
    pub did_id: u64,
    pub claim_type: String,
    pub claim_value: String,
    pub verified: bool,
    pub verifier: Option<Address>,
    pub created_at: u64,
}

/// DID storage sub-keys.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DidKey {
    Counter,
    Record(u64),
    OwnerDids(Address),
    ClaimCounter,
    Claim(u64),
    DidClaims(u64),
}

/// Register a new DID for an owner.
pub fn register_did(env: &Env, owner: &Address, did_string: String, method: DidMethod) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::Did(DidKey::Counter))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&DataKey::Did(DidKey::Counter), &(id + 1));

    let now = env.ledger().timestamp();
    let did = Did {
        id,
        owner: owner.clone(),
        did_string,
        method,
        created_at: now,
        updated_at: now,
        active: true,
    };

    env.storage()
        .persistent()
        .set(&DataKey::Did(DidKey::Record(id)), &did);

    let mut owner_dids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::Did(DidKey::OwnerDids(owner.clone())))
        .unwrap_or_else(|| Vec::new(env));
    owner_dids.push_back(id);
    env.storage()
        .persistent()
        .set(&DataKey::Did(DidKey::OwnerDids(owner.clone())), &owner_dids);

    env.events()
        .publish((symbol_short!("did_reg"),), (id, owner.clone()));

    id
}

/// Update a DID record.
pub fn update_did(env: &Env, owner: &Address, did_id: u64, did_string: String) {
    let mut did: Did = env
        .storage()
        .persistent()
        .get(&DataKey::Did(DidKey::Record(did_id)))
        .unwrap_or_else(|| panic!("DID not found"));

    assert!(did.owner == *owner, "unauthorized");
    assert!(did.active, "DID inactive");

    did.did_string = did_string;
    did.updated_at = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&DataKey::Did(DidKey::Record(did_id)), &did);

    env.events()
        .publish((symbol_short!("did_upd"),), (did_id, owner.clone()));
}

/// Add a claim to a DID.
pub fn add_claim(env: &Env, did_id: u64, claim_type: String, claim_value: String) -> u64 {
    let claim_id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::Did(DidKey::ClaimCounter))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&DataKey::Did(DidKey::ClaimCounter), &(claim_id + 1));

    let claim = IdentityClaim {
        claim_id,
        did_id,
        claim_type,
        claim_value,
        verified: false,
        verifier: None,
        created_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&DataKey::Did(DidKey::Claim(claim_id)), &claim);

    let mut did_claims: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::Did(DidKey::DidClaims(did_id)))
        .unwrap_or_else(|| Vec::new(env));
    did_claims.push_back(claim_id);
    env.storage()
        .persistent()
        .set(&DataKey::Did(DidKey::DidClaims(did_id)), &did_claims);

    env.events()
        .publish((symbol_short!("did_clm"),), (claim_id, did_id));

    claim_id
}

/// Verify a claim.
pub fn verify_claim(env: &Env, verifier: &Address, claim_id: u64) {
    let mut claim: IdentityClaim = env
        .storage()
        .persistent()
        .get(&DataKey::Did(DidKey::Claim(claim_id)))
        .unwrap_or_else(|| panic!("claim not found"));

    claim.verified = true;
    claim.verifier = Some(verifier.clone());

    env.storage()
        .persistent()
        .set(&DataKey::Did(DidKey::Claim(claim_id)), &claim);

    env.events()
        .publish((symbol_short!("clm_ver"),), (claim_id, verifier.clone()));
}

/// Get a DID record.
pub fn get_did(env: &Env, did_id: u64) -> Option<Did> {
    env.storage()
        .persistent()
        .get(&DataKey::Did(DidKey::Record(did_id)))
}

/// Get all DIDs for an owner.
pub fn get_owner_dids(env: &Env, owner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::Did(DidKey::OwnerDids(owner.clone())))
        .unwrap_or_else(|| Vec::new(env))
}

/// Get all claims for a DID.
pub fn get_did_claims(env: &Env, did_id: u64) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::Did(DidKey::DidClaims(did_id)))
        .unwrap_or_else(|| Vec::new(env))
}

/// Get a claim record.
pub fn get_claim(env: &Env, claim_id: u64) -> Option<IdentityClaim> {
    env.storage()
        .persistent()
        .get(&DataKey::Did(DidKey::Claim(claim_id)))
}
