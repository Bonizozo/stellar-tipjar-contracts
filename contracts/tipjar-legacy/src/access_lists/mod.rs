//! Address whitelists and blacklists for compliance and creator preferences.
//!
//! Two scopes are supported:
//! - `Global`: compliance-wide lists managed by the contract admin. A globally
//!   blacklisted address is blocked from interacting with every creator.
//! - `Creator`: per-creator preference lists managed by the creator themselves.
//!
//! Each scope independently supports:
//! - A blacklist with optional **temporary** blocks (an expiry timestamp).
//! - A whitelist plus an optional "whitelist-only" mode that, when enabled,
//!   rejects any address that is not explicitly whitelisted.
//!
//! Enforcement precedence (see [`check_allowed`]): an active blacklist entry in
//! either scope always rejects; otherwise, if whitelist-only mode is enabled in
//! a scope, the subject must be whitelisted in that scope.
//!
//! Every mutation emits an event so off-chain indexers can track list changes.

use soroban_sdk::{contracttype, panic_with_error, symbol_short, Address, Env, String, Vec};

use crate::DataKey;

// ── Types ────────────────────────────────────────────────────────────────────

/// Selects which list a management or query operation applies to.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListScope {
    /// Compliance-wide lists, managed by the contract admin.
    Global,
    /// Per-creator preference lists, managed by `creator`.
    Creator(Address),
}

/// A blacklist entry. Supports permanent and temporary (expiring) blocks.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockEntry {
    /// The blocked address.
    pub subject: Address,
    /// Human-readable reason for the block.
    pub reason: String,
    /// Ledger timestamp at which the block was created.
    pub created_at: u64,
    /// Expiry timestamp (unix seconds). `0` means the block is permanent.
    pub expires_at: u64,
}

/// Storage sub-keys for access-list data.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessListKey {
    /// Global blacklist entry keyed by subject address.
    GlobalBlacklist(Address),
    /// Global whitelist flag keyed by subject address.
    GlobalWhitelist(Address),
    /// Whether global whitelist-only mode is enforced (bool).
    GlobalWhitelistMode,
    /// List of all globally blacklisted subjects.
    GlobalBlacklistMembers,
    /// List of all globally whitelisted subjects.
    GlobalWhitelistMembers,
    /// Creator blacklist entry keyed by (creator, subject).
    CreatorBlacklist(Address, Address),
    /// Creator whitelist flag keyed by (creator, subject).
    CreatorWhitelist(Address, Address),
    /// Whether a creator enforces whitelist-only mode (bool).
    CreatorWhitelistMode(Address),
    /// List of subjects blacklisted by a creator.
    CreatorBlacklistMembers(Address),
    /// List of subjects whitelisted by a creator.
    CreatorWhitelistMembers(Address),
}

// ── Errors ───────────────────────────────────────────────────────────────────

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AccessListError {
    /// Caller is not authorized to manage the requested scope's lists.
    Unauthorized = 900,
    /// The contract has no admin configured (cannot manage global lists).
    NoAdmin = 901,
    /// The subject address is on the (global or creator) blacklist.
    Blacklisted = 902,
    /// Whitelist-only mode is on and the subject is not whitelisted.
    NotWhitelisted = 903,
    /// A temporary-block duration was supplied that is not positive.
    InvalidDuration = 904,
    /// No blacklist entry exists for the subject in this scope.
    EntryNotFound = 905,
}

// ── Authorization ──────────────────────────────────────────────────────────────

/// Requires `caller` to be authorized to manage `scope` and verifies auth.
fn authorize_scope(env: &Env, caller: &Address, scope: &ListScope) {
    caller.require_auth();
    match scope {
        ListScope::Global => {
            let admin: Address = env
                .storage()
                .instance()
                .get(&DataKey::Admin)
                .unwrap_or_else(|| panic_with_error!(env, AccessListError::NoAdmin));
            if admin != *caller {
                panic_with_error!(env, AccessListError::Unauthorized);
            }
        }
        ListScope::Creator(creator) => {
            if creator != caller {
                panic_with_error!(env, AccessListError::Unauthorized);
            }
        }
    }
}

// ── Storage key helpers ─────────────────────────────────────────────────────

fn blacklist_key(scope: &ListScope, subject: &Address) -> AccessListKey {
    match scope {
        ListScope::Global => AccessListKey::GlobalBlacklist(subject.clone()),
        ListScope::Creator(c) => AccessListKey::CreatorBlacklist(c.clone(), subject.clone()),
    }
}

fn whitelist_key(scope: &ListScope, subject: &Address) -> AccessListKey {
    match scope {
        ListScope::Global => AccessListKey::GlobalWhitelist(subject.clone()),
        ListScope::Creator(c) => AccessListKey::CreatorWhitelist(c.clone(), subject.clone()),
    }
}

fn whitelist_mode_key(scope: &ListScope) -> AccessListKey {
    match scope {
        ListScope::Global => AccessListKey::GlobalWhitelistMode,
        ListScope::Creator(c) => AccessListKey::CreatorWhitelistMode(c.clone()),
    }
}

fn blacklist_members_key(scope: &ListScope) -> AccessListKey {
    match scope {
        ListScope::Global => AccessListKey::GlobalBlacklistMembers,
        ListScope::Creator(c) => AccessListKey::CreatorBlacklistMembers(c.clone()),
    }
}

fn whitelist_members_key(scope: &ListScope) -> AccessListKey {
    match scope {
        ListScope::Global => AccessListKey::GlobalWhitelistMembers,
        ListScope::Creator(c) => AccessListKey::CreatorWhitelistMembers(c.clone()),
    }
}

/// Short event topic tag identifying the scope (`"global"` / `"creator"`).
fn scope_tag(env: &Env, scope: &ListScope) -> String {
    match scope {
        ListScope::Global => String::from_str(env, "global"),
        ListScope::Creator(_) => String::from_str(env, "creator"),
    }
}

// ── Member-list maintenance ─────────────────────────────────────────────────

fn add_member(env: &Env, key: &AccessListKey, subject: &Address) {
    let dk = DataKey::AccessList(key.clone());
    let mut members: Vec<Address> = env
        .storage()
        .persistent()
        .get(&dk)
        .unwrap_or_else(|| Vec::new(env));
    if !members.contains(subject) {
        members.push_back(subject.clone());
        env.storage().persistent().set(&dk, &members);
    }
}

fn remove_member(env: &Env, key: &AccessListKey, subject: &Address) {
    let dk = DataKey::AccessList(key.clone());
    let members: Vec<Address> = env
        .storage()
        .persistent()
        .get(&dk)
        .unwrap_or_else(|| Vec::new(env));
    let mut remaining = Vec::new(env);
    for addr in members.iter() {
        if addr != *subject {
            remaining.push_back(addr);
        }
    }
    env.storage().persistent().set(&dk, &remaining);
}

// ── Blacklist management ────────────────────────────────────────────────────

/// Adds `subject` to the blacklist for `scope`.
///
/// `duration` is the number of seconds the block stays in effect; pass `0` for
/// a permanent block. Re-blocking an existing subject overwrites the entry.
///
/// Emits `("al_block", scope_tag)` with `(subject, expires_at, reason)`.
pub fn add_to_blacklist(
    env: &Env,
    caller: &Address,
    scope: &ListScope,
    subject: &Address,
    reason: String,
    duration: u64,
) {
    authorize_scope(env, caller, scope);

    let now = env.ledger().timestamp();
    let expires_at = if duration == 0 {
        0
    } else {
        now.checked_add(duration)
            .unwrap_or_else(|| panic_with_error!(env, AccessListError::InvalidDuration))
    };

    let entry = BlockEntry {
        subject: subject.clone(),
        reason: reason.clone(),
        created_at: now,
        expires_at,
    };
    env.storage()
        .persistent()
        .set(&DataKey::AccessList(blacklist_key(scope, subject)), &entry);
    add_member(env, &blacklist_members_key(scope), subject);

    env.events().publish(
        (symbol_short!("al_block"), scope_tag(env, scope)),
        (subject.clone(), expires_at, reason),
    );
}

/// Removes `subject` from the blacklist for `scope` (unblock).
///
/// Emits `("al_unblk", scope_tag)` with `(subject,)`.
pub fn remove_from_blacklist(env: &Env, caller: &Address, scope: &ListScope, subject: &Address) {
    authorize_scope(env, caller, scope);

    let key = DataKey::AccessList(blacklist_key(scope, subject));
    if !env.storage().persistent().has(&key) {
        panic_with_error!(env, AccessListError::EntryNotFound);
    }
    env.storage().persistent().remove(&key);
    remove_member(env, &blacklist_members_key(scope), subject);

    env.events().publish(
        (symbol_short!("al_unblk"), scope_tag(env, scope)),
        (subject.clone(),),
    );
}

// ── Whitelist management ────────────────────────────────────────────────────

/// Adds `subject` to the whitelist for `scope`.
///
/// Emits `("al_allow", scope_tag)` with `(subject,)`.
pub fn add_to_whitelist(env: &Env, caller: &Address, scope: &ListScope, subject: &Address) {
    authorize_scope(env, caller, scope);

    env.storage()
        .persistent()
        .set(&DataKey::AccessList(whitelist_key(scope, subject)), &true);
    add_member(env, &whitelist_members_key(scope), subject);

    env.events().publish(
        (symbol_short!("al_allow"), scope_tag(env, scope)),
        (subject.clone(),),
    );
}

/// Removes `subject` from the whitelist for `scope`.
///
/// Emits `("al_unallw", scope_tag)` with `(subject,)`.
pub fn remove_from_whitelist(env: &Env, caller: &Address, scope: &ListScope, subject: &Address) {
    authorize_scope(env, caller, scope);

    env.storage()
        .persistent()
        .remove(&DataKey::AccessList(whitelist_key(scope, subject)));
    remove_member(env, &whitelist_members_key(scope), subject);

    env.events().publish(
        (symbol_short!("al_unallw"), scope_tag(env, scope)),
        (subject.clone(),),
    );
}

/// Enables or disables whitelist-only mode for `scope`.
///
/// When enabled, [`check_allowed`] rejects any subject not on the scope's
/// whitelist (unless overridden by a higher-precedence rule).
///
/// Emits `("al_mode", scope_tag)` with `(enabled,)`.
pub fn set_whitelist_mode(env: &Env, caller: &Address, scope: &ListScope, enabled: bool) {
    authorize_scope(env, caller, scope);

    env.storage()
        .persistent()
        .set(&DataKey::AccessList(whitelist_mode_key(scope)), &enabled);

    env.events().publish(
        (symbol_short!("al_mode"), scope_tag(env, scope)),
        (enabled,),
    );
}

// ── Queries ─────────────────────────────────────────────────────────────────

/// Returns the raw blacklist entry for `subject` in `scope`, if any.
///
/// Note: the entry may already be expired; use [`is_blacklisted`] to account
/// for temporary blocks.
pub fn get_block_entry(env: &Env, scope: &ListScope, subject: &Address) -> Option<BlockEntry> {
    env.storage()
        .persistent()
        .get(&DataKey::AccessList(blacklist_key(scope, subject)))
}

/// Returns `true` if `subject` is currently blacklisted in `scope`.
///
/// A temporary block whose `expires_at` has passed is treated as inactive.
pub fn is_blacklisted(env: &Env, scope: &ListScope, subject: &Address) -> bool {
    match get_block_entry(env, scope, subject) {
        Some(entry) => entry.expires_at == 0 || env.ledger().timestamp() < entry.expires_at,
        None => false,
    }
}

/// Returns `true` if `subject` is on the whitelist for `scope`.
pub fn is_whitelisted(env: &Env, scope: &ListScope, subject: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::AccessList(whitelist_key(scope, subject)))
        .unwrap_or(false)
}

/// Returns `true` if whitelist-only mode is enabled for `scope`.
pub fn is_whitelist_mode(env: &Env, scope: &ListScope) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::AccessList(whitelist_mode_key(scope)))
        .unwrap_or(false)
}

/// Returns all subjects currently on the blacklist for `scope`.
pub fn get_blacklist(env: &Env, scope: &ListScope) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::AccessList(blacklist_members_key(scope)))
        .unwrap_or_else(|| Vec::new(env))
}

/// Returns all subjects currently on the whitelist for `scope`.
pub fn get_whitelist(env: &Env, scope: &ListScope) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::AccessList(whitelist_members_key(scope)))
        .unwrap_or_else(|| Vec::new(env))
}

// ── Enforcement ─────────────────────────────────────────────────────────────

/// Enforces access-list restrictions for `subject` interacting with `creator`.
///
/// Panics with [`AccessListError::Blacklisted`] if the subject is blacklisted
/// at either the global or creator scope, or with
/// [`AccessListError::NotWhitelisted`] if whitelist-only mode is active in a
/// scope and the subject is not whitelisted there.
///
/// Precedence: blacklist (global then creator) is checked before whitelist
/// mode, so an explicit block always wins over a whitelist entry.
pub fn check_allowed(env: &Env, creator: &Address, subject: &Address) {
    let creator_scope = ListScope::Creator(creator.clone());

    // 1. Blacklist always rejects.
    if is_blacklisted(env, &ListScope::Global, subject)
        || is_blacklisted(env, &creator_scope, subject)
    {
        panic_with_error!(env, AccessListError::Blacklisted);
    }

    // 2. Whitelist-only mode (global, then creator).
    if is_whitelist_mode(env, &ListScope::Global)
        && !is_whitelisted(env, &ListScope::Global, subject)
    {
        panic_with_error!(env, AccessListError::NotWhitelisted);
    }
    if is_whitelist_mode(env, &creator_scope) && !is_whitelisted(env, &creator_scope, subject) {
        panic_with_error!(env, AccessListError::NotWhitelisted);
    }
}

/// Non-panicking variant of [`check_allowed`]; returns `true` if allowed.
pub fn is_allowed(env: &Env, creator: &Address, subject: &Address) -> bool {
    let creator_scope = ListScope::Creator(creator.clone());

    if is_blacklisted(env, &ListScope::Global, subject)
        || is_blacklisted(env, &creator_scope, subject)
    {
        return false;
    }
    if is_whitelist_mode(env, &ListScope::Global)
        && !is_whitelisted(env, &ListScope::Global, subject)
    {
        return false;
    }
    if is_whitelist_mode(env, &creator_scope) && !is_whitelisted(env, &creator_scope, subject) {
        return false;
    }
    true
}
