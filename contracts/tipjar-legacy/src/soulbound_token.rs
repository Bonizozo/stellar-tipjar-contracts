//! Soulbound token (SBT) module (#249).
//!
//! Non-transferable tokens representing tip achievements and milestones.
//! Supports minting, achievement tracking, revocation, and metadata storage.

use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Vec};

use crate::DataKey;

// ── Types ────────────────────────────────────────────────────────────────────

/// A soulbound token record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoulboundToken {
    pub id: u64,
    /// The address this SBT is bound to (non-transferable).
    pub owner: Address,
    /// Achievement category (e.g. "top_tipper", "milestone_100").
    pub achievement: String,
    /// Optional metadata URI or JSON string.
    pub metadata: String,
    pub minted_at: u64,
    pub revoked: bool,
}

// ── DataKey sub-keys ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SbtKey {
    Counter,
    Token(u64),
    /// List of SBT IDs owned by an address.
    OwnerTokens(Address),
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn load_token(env: &Env, id: u64) -> SoulboundToken {
    env.storage()
        .persistent()
        .get(&DataKey::SoulboundToken(SbtKey::Token(id)))
        .unwrap_or_else(|| panic!("SBT not found"))
}

fn save_token(env: &Env, token: &SoulboundToken) {
    env.storage()
        .persistent()
        .set(&DataKey::SoulboundToken(SbtKey::Token(token.id)), token);
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Mint a new soulbound token to `owner`.
///
/// Returns the token ID.
/// Emits `("sbt_mint",)` with `(id, owner, achievement)`.
pub fn mint(
    env: &Env,
    owner: &Address,
    achievement: String,
    metadata: String,
) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::SoulboundToken(SbtKey::Counter))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&DataKey::SoulboundToken(SbtKey::Counter), &(id + 1));

    let sbt = SoulboundToken {
        id,
        owner: owner.clone(),
        achievement: achievement.clone(),
        metadata,
        minted_at: env.ledger().timestamp(),
        revoked: false,
    };

    save_token(env, &sbt);

    // Track owner's tokens.
    let mut tokens: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::SoulboundToken(SbtKey::OwnerTokens(owner.clone())))
        .unwrap_or_else(|| Vec::new(env));
    tokens.push_back(id);
    env.storage()
        .persistent()
        .set(&DataKey::SoulboundToken(SbtKey::OwnerTokens(owner.clone())), &tokens);

    env.events().publish(
        (symbol_short!("sbt_mnt"),),
        (id, owner.clone(), achievement),
    );

    id
}

/// Revoke a soulbound token. Only callable by the contract admin (enforced in lib.rs).
///
/// Emits `("sbt_rev",)` with `(id, owner)`.
pub fn revoke(env: &Env, token_id: u64) {
    let mut sbt = load_token(env, token_id);
    assert!(!sbt.revoked, "already revoked");
    sbt.revoked = true;
    save_token(env, &sbt);
    env.events().publish(
        (symbol_short!("sbt_rev"),),
        (token_id, sbt.owner),
    );
}

/// Returns the SBT record.
pub fn get_token(env: &Env, token_id: u64) -> SoulboundToken {
    load_token(env, token_id)
}

/// Returns all SBT IDs owned by `owner`.
pub fn get_owner_tokens(env: &Env, owner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::SoulboundToken(SbtKey::OwnerTokens(owner.clone())))
        .unwrap_or_else(|| Vec::new(env))
}

/// Returns `true` if `owner` holds a non-revoked SBT for `achievement`.
pub fn has_achievement(env: &Env, owner: &Address, achievement: &String) -> bool {
    let ids = get_owner_tokens(env, owner);
    for id in ids.iter() {
        let sbt = load_token(env, id);
        if !sbt.revoked && &sbt.achievement == achievement {
            return true;
        }
    }
    false
}
