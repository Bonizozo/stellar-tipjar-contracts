//! Verifiable Credentials module (#309).
//!
//! Credential issuance, verification, revocation, and lifecycle tracking.

use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Vec};

use crate::DataKey;

/// Status of a verifiable credential.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum CredentialStatus {
    Active,
    Revoked,
    Expired,
}

/// A verifiable credential record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiableCredential {
    pub id: u64,
    pub issuer: Address,
    pub subject: Address,
    pub schema: String,
    pub credential_data: String,
    pub issued_at: u64,
    pub expires_at: Option<u64>,
    pub status: CredentialStatus,
}

/// VC storage sub-keys.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VcKey {
    Counter,
    Record(u64),
    SubjectCredentials(Address),
    IssuerCredentials(Address),
}

/// Issue a new verifiable credential.
pub fn issue_credential(
    env: &Env,
    issuer: &Address,
    subject: &Address,
    schema: String,
    credential_data: String,
    expires_at: Option<u64>,
) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::Vc(VcKey::Counter))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&DataKey::Vc(VcKey::Counter), &(id + 1));

    let vc = VerifiableCredential {
        id,
        issuer: issuer.clone(),
        subject: subject.clone(),
        schema,
        credential_data,
        issued_at: env.ledger().timestamp(),
        expires_at,
        status: CredentialStatus::Active,
    };

    env.storage()
        .persistent()
        .set(&DataKey::Vc(VcKey::Record(id)), &vc);

    let mut subject_creds: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::Vc(VcKey::SubjectCredentials(subject.clone())))
        .unwrap_or_else(|| Vec::new(env));
    subject_creds.push_back(id);
    env.storage().persistent().set(
        &DataKey::Vc(VcKey::SubjectCredentials(subject.clone())),
        &subject_creds,
    );

    let mut issuer_creds: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::Vc(VcKey::IssuerCredentials(issuer.clone())))
        .unwrap_or_else(|| Vec::new(env));
    issuer_creds.push_back(id);
    env.storage().persistent().set(
        &DataKey::Vc(VcKey::IssuerCredentials(issuer.clone())),
        &issuer_creds,
    );

    env.events().publish(
        (symbol_short!("vc_issue"),),
        (id, issuer.clone(), subject.clone()),
    );

    id
}

/// Verify a credential is active and not expired.
pub fn verify_credential(env: &Env, credential_id: u64) -> bool {
    let vc: VerifiableCredential = match env
        .storage()
        .persistent()
        .get(&DataKey::Vc(VcKey::Record(credential_id)))
    {
        Some(v) => v,
        None => return false,
    };

    if vc.status != CredentialStatus::Active {
        return false;
    }

    if let Some(expires_at) = vc.expires_at {
        if env.ledger().timestamp() > expires_at {
            return false;
        }
    }

    true
}

/// Revoke a credential. Only the issuer may revoke.
pub fn revoke_credential(env: &Env, issuer: &Address, credential_id: u64) {
    let mut vc: VerifiableCredential = env
        .storage()
        .persistent()
        .get(&DataKey::Vc(VcKey::Record(credential_id)))
        .unwrap_or_else(|| panic!("credential not found"));

    assert!(vc.issuer == *issuer, "unauthorized");
    assert!(vc.status == CredentialStatus::Active, "not active");

    vc.status = CredentialStatus::Revoked;
    env.storage()
        .persistent()
        .set(&DataKey::Vc(VcKey::Record(credential_id)), &vc);

    env.events()
        .publish((symbol_short!("vc_rev"),), (credential_id, issuer.clone()));
}

/// Get a credential record.
pub fn get_credential(env: &Env, credential_id: u64) -> Option<VerifiableCredential> {
    env.storage()
        .persistent()
        .get(&DataKey::Vc(VcKey::Record(credential_id)))
}

/// Get all credential IDs for a subject.
pub fn get_subject_credentials(env: &Env, subject: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::Vc(VcKey::SubjectCredentials(subject.clone())))
        .unwrap_or_else(|| Vec::new(env))
}

/// Get all credential IDs issued by an issuer.
pub fn get_issuer_credentials(env: &Env, issuer: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::Vc(VcKey::IssuerCredentials(issuer.clone())))
        .unwrap_or_else(|| Vec::new(env))
}
