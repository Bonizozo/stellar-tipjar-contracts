//! Meta-transaction support for gasless tipping.
//!
//! Allows a trusted relayer to submit a tip on behalf of a user who has
//! signed a `MetaTipRequest` off-chain.  The contract verifies the
//! user-supplied signature, enforces per-sender nonce ordering for replay
//! protection, and then executes the tip as if the user called it directly.
//!
//! # EIP-2771 analogy
//! On Stellar/Soroban there is no native `msg.sender` override, so we
//! replicate the EIP-2771 pattern by:
//!   1. Accepting a signed request struct from the relayer.
//!   2. Recovering / verifying the signer via `env.crypto().ed25519_verify`.
//!   3. Using the verified `from` address as the logical sender.
//!
//! # Replay protection
//! Each `from` address has a monotonically increasing nonce stored on-chain.
//! A request is only valid when `request.nonce == stored_nonce`, after which
//! the stored nonce is incremented.

use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{contracttype, symbol_short, Address, Bytes, BytesN, Env, String};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Maximum seconds a signed request remains valid after `valid_until`.
/// Requests with `valid_until < ledger_timestamp` are rejected.
pub const REQUEST_EXPIRY_BUFFER: u64 = 0; // strict: must not be expired

// ── Types ────────────────────────────────────────────────────────────────────

/// The action a meta-transaction may perform.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetaTipAction {
    /// Send a plain tip to `creator` of `amount` in `token`.
    Tip,
    /// Open a tip state channel with `creator` for `amount` deposit.
    OpenChannel,
}

/// A signed meta-transaction request submitted by a relayer on behalf of a user.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetaTipRequest {
    /// The user who authorised this request (logical sender).
    pub from: Address,
    /// Destination creator address.
    pub to: Address,
    /// Token contract address.
    pub token: Address,
    /// Amount (tip amount or channel deposit depending on `action`).
    pub amount: i128,
    /// Per-sender nonce for replay protection.
    pub nonce: u64,
    /// Ledger timestamp after which this request is invalid.
    pub valid_until: u64,
    /// Optional message attached to the tip.
    pub message: String,
    /// Action to perform.
    pub action: MetaTipAction,
    /// Ed25519 signature over the canonical request hash.
    pub signature: BytesN<64>,
    /// Ed25519 public key of `from` (used for verification).
    pub public_key: BytesN<32>,
}

/// Minimal record stored on-chain for audit / indexing.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetaTxRecord {
    /// Unique sequential ID.
    pub id: u64,
    /// Logical sender (signer).
    pub from: Address,
    /// Relayer that submitted the transaction.
    pub relayer: Address,
    /// Destination creator.
    pub to: Address,
    /// Token used.
    pub token: Address,
    /// Amount processed.
    pub amount: i128,
    /// Nonce consumed.
    pub nonce: u64,
    /// Ledger timestamp of execution.
    pub executed_at: u64,
    /// Action performed.
    pub action: MetaTipAction,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

/// Returns the current nonce for `sender`, defaulting to 0.
pub fn get_nonce(env: &Env, sender: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::MetaTxNonce(sender.clone()))
        .unwrap_or(0)
}

/// Increments and persists the nonce for `sender`.
fn bump_nonce(env: &Env, sender: &Address) {
    let next = get_nonce(env, sender) + 1;
    env.storage()
        .persistent()
        .set(&DataKey::MetaTxNonce(sender.clone()), &next);
}

/// Returns whether `relayer` is a registered trusted relayer.
pub fn is_trusted_relayer(env: &Env, relayer: &Address) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::MetaTxRelayer(relayer.clone()))
        .unwrap_or(false)
}

/// Registers `relayer` as trusted (admin only — enforced by caller).
pub fn register_relayer(env: &Env, relayer: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::MetaTxRelayer(relayer.clone()), &true);
}

/// Removes `relayer` from the trusted set.
pub fn remove_relayer(env: &Env, relayer: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::MetaTxRelayer(relayer.clone()), &false);
}

/// Returns whether `nullifier` has been consumed (replay protection).
pub fn is_consumed(env: &Env, nullifier: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::MetaTxNullifier(nullifier.clone()))
        .unwrap_or(false)
}

fn mark_consumed(env: &Env, nullifier: &BytesN<32>) {
    env.storage()
        .persistent()
        .set(&DataKey::MetaTxNullifier(nullifier.clone()), &true);
}

fn next_record_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::MetaTxCounter)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::MetaTxCounter, &(id + 1));
    id
}

// ── Canonical message hash ───────────────────────────────────────────────────

/// Builds the canonical byte payload that the user must sign.
///
/// Layout (all big-endian):
///   "METATIP" (7 bytes) | from (32 bytes) | to (32 bytes) | token (32 bytes)
///   | amount (16 bytes i128) | nonce (8 bytes u64) | valid_until (8 bytes u64)
///
/// The `from`, `to`, and `token` addresses are serialised as their 32-byte
/// Stellar account ID (G-address) XDR representation via `to_xdr`.
pub fn canonical_hash(env: &Env, req: &MetaTipRequest) -> BytesN<32> {
    let mut payload = Bytes::new(env);

    // Domain separator
    payload.extend_from_array(&[b'M', b'E', b'T', b'A', b'T', b'I', b'P']);

    // Addresses as XDR bytes
    payload.append(&req.from.clone().to_xdr(env));
    payload.append(&req.to.clone().to_xdr(env));
    payload.append(&req.token.clone().to_xdr(env));

    // amount as 16-byte big-endian i128
    payload.extend_from_array(&req.amount.to_be_bytes());

    // nonce as 8-byte big-endian u64
    payload.extend_from_array(&req.nonce.to_be_bytes());

    // valid_until as 8-byte big-endian u64
    payload.extend_from_array(&req.valid_until.to_be_bytes());

    env.crypto().sha256(&payload).to_bytes()
}

// ── Core verification ────────────────────────────────────────────────────────

/// Verifies a `MetaTipRequest` and returns the canonical hash on success.
///
/// Checks (in order):
///   1. Relayer is trusted.
///   2. Request has not expired (`valid_until >= now`).
///   3. Nonce matches the stored per-sender nonce.
///   4. Nullifier (hash) has not been consumed.
///   5. Ed25519 signature is valid over the canonical hash.
pub fn verify(env: &Env, relayer: &Address, req: &MetaTipRequest) -> BytesN<32> {
    assert!(is_trusted_relayer(env, relayer), "Untrusted relayer");

    let now = env.ledger().timestamp();
    assert!(req.valid_until >= now, "Request expired");

    let stored_nonce = get_nonce(env, &req.from);
    assert!(req.nonce == stored_nonce, "Invalid nonce");

    let hash = canonical_hash(env, req);

    assert!(!is_consumed(env, &hash), "Request already executed");

    // Ed25519 signature verification
    env.crypto()
        .ed25519_verify(&req.public_key, &hash.clone().into(), &req.signature);

    hash
}

// ── Execution ────────────────────────────────────────────────────────────────

/// Marks the request as consumed and bumps the sender nonce.
/// Returns the new `MetaTxRecord` ID.
///
/// Must be called *after* the tip/channel action has been applied so that
/// state changes follow the CEI pattern.
pub fn finalize(env: &Env, relayer: &Address, req: &MetaTipRequest, hash: &BytesN<32>) -> u64 {
    mark_consumed(env, hash);
    bump_nonce(env, &req.from);

    let id = next_record_id(env);
    let record = MetaTxRecord {
        id,
        from: req.from.clone(),
        relayer: relayer.clone(),
        to: req.to.clone(),
        token: req.token.clone(),
        amount: req.amount,
        nonce: req.nonce,
        executed_at: env.ledger().timestamp(),
        action: req.action.clone(),
    };

    env.storage()
        .persistent()
        .set(&DataKey::MetaTxRecord(id), &record);

    env.events().publish(
        (symbol_short!("mtx_exec"),),
        (
            id,
            req.from.clone(),
            relayer.clone(),
            req.to.clone(),
            req.amount,
            req.nonce,
        ),
    );

    id
}

/// Returns a stored meta-tx record by ID.
pub fn get_record(env: &Env, id: u64) -> Option<MetaTxRecord> {
    env.storage().persistent().get(&DataKey::MetaTxRecord(id))
}
