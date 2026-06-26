//! Royalty splitting for collaborative content and team tips.
//!
//! Supports:
//! - Split configurations (multiple recipients with basis-point shares)
//! - Automatic distribution when a tip is received
//! - Split modifications (owner can update recipients/shares)
//! - Nested splits (a recipient can itself be a split, resolved recursively)
//! - Split history (every distribution is appended to an on-chain log)

use soroban_sdk::{contracttype, symbol_short, Address, Env, Vec};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Maximum recipients in a single split config.
pub const MAX_RECIPIENTS: u32 = 20;
/// Maximum recursion depth for nested splits.
pub const MAX_NEST_DEPTH: u32 = 5;
/// Basis points denominator (10 000 = 100 %).
pub const BPS_DENOM: i128 = 10_000;

// ── Types ────────────────────────────────────────────────────────────────────

/// One entry in a split: a recipient address and their share in basis points.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitRecipient {
    pub recipient: Address,
    /// Share in basis points. All recipients in a config must sum to 10 000.
    pub share_bps: u32,
}

/// A named split configuration owned by `owner`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitConfig {
    pub owner: Address,
    pub recipients: Vec<SplitRecipient>,
}

/// One entry in the distribution history for a split.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitHistoryEntry {
    /// Total amount that was distributed.
    pub amount: i128,
    /// Ledger timestamp of the distribution.
    pub timestamp: u64,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

fn load_config(env: &Env, split_id: &Address) -> Option<SplitConfig> {
    env.storage()
        .persistent()
        .get(&DataKey::SplitConfig(split_id.clone()))
}

fn save_config(env: &Env, split_id: &Address, cfg: &SplitConfig) {
    env.storage()
        .persistent()
        .set(&DataKey::SplitConfig(split_id.clone()), cfg);
}

fn history_count(env: &Env, split_id: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::SplitHistoryCount(split_id.clone()))
        .unwrap_or(0u64)
}

fn append_history(env: &Env, split_id: &Address, entry: &SplitHistoryEntry) {
    let idx = history_count(env, split_id);
    env.storage()
        .persistent()
        .set(&DataKey::SplitHistory(split_id.clone(), idx), entry);
    env.storage()
        .persistent()
        .set(&DataKey::SplitHistoryCount(split_id.clone()), &(idx + 1));
}

// ── Validation ───────────────────────────────────────────────────────────────

fn validate_recipients(recipients: &Vec<SplitRecipient>) {
    assert!(!recipients.is_empty(), "no recipients");
    assert!(recipients.len() <= MAX_RECIPIENTS, "too many recipients");
    let total: u32 = (0..recipients.len())
        .map(|i| recipients.get(i).unwrap().share_bps)
        .sum();
    assert!(total == 10_000, "shares must sum to 10000 bps");
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Create or replace a split configuration keyed by `split_id`.
/// `split_id` is typically the collaborative content address or a team address.
/// `owner` must authorise; they are the only one who can later modify it.
pub fn set_split(env: &Env, split_id: &Address, owner: &Address, recipients: Vec<SplitRecipient>) {
    owner.require_auth();
    validate_recipients(&recipients);
    save_config(
        env,
        split_id,
        &SplitConfig {
            owner: owner.clone(),
            recipients,
        },
    );
}

/// Modify an existing split configuration. Only the original owner may do this.
pub fn modify_split(
    env: &Env,
    split_id: &Address,
    owner: &Address,
    recipients: Vec<SplitRecipient>,
) {
    owner.require_auth();
    let cfg = load_config(env, split_id).expect("split not found");
    assert!(cfg.owner == *owner, "not split owner");
    validate_recipients(&recipients);
    save_config(
        env,
        split_id,
        &SplitConfig {
            owner: owner.clone(),
            recipients,
        },
    );
}

/// Distribute `amount` according to the split config for `split_id`.
///
/// Nested splits are resolved recursively up to [`MAX_NEST_DEPTH`] levels:
/// if a recipient address itself has a split config, the portion owed to it
/// is further split among its own recipients.
///
/// Each leaf recipient's share is credited to `DataKey::SplitBalance`.
/// The distribution is appended to the split history.
/// Returns the total amount distributed (equals `amount` when shares sum to 10 000).
pub fn distribute(env: &Env, split_id: &Address, amount: i128) -> i128 {
    distribute_inner(env, split_id, amount, 0)
}

fn distribute_inner(env: &Env, split_id: &Address, amount: i128, depth: u32) -> i128 {
    if amount <= 0 {
        return 0;
    }
    let cfg = match load_config(env, split_id) {
        Some(c) => c,
        None => return 0,
    };

    let mut distributed: i128 = 0;

    for i in 0..cfg.recipients.len() {
        let r = cfg.recipients.get(i).unwrap();
        let share = amount * r.share_bps as i128 / BPS_DENOM;
        if share <= 0 {
            continue;
        }

        // Nested split: if the recipient itself has a split config, recurse.
        if depth < MAX_NEST_DEPTH && load_config(env, &r.recipient).is_some() {
            distribute_inner(env, &r.recipient, share, depth + 1);
        } else {
            // Leaf: credit balance.
            let key = DataKey::SplitBalance(r.recipient.clone());
            let bal: i128 = env.storage().persistent().get(&key).unwrap_or(0);
            env.storage().persistent().set(&key, &(bal + share));
        }

        distributed += share;
    }

    // Record history and emit event only at the top-level call.
    if depth == 0 {
        append_history(
            env,
            split_id,
            &SplitHistoryEntry {
                amount: distributed,
                timestamp: env.ledger().timestamp(),
            },
        );
        env.events()
            .publish((symbol_short!("split_dst"), split_id.clone()), distributed);
    }

    distributed
}

/// Returns the split config for `split_id`, or `None` if not set.
pub fn get_split(env: &Env, split_id: &Address) -> Option<SplitConfig> {
    load_config(env, split_id)
}

/// Returns a history entry by index, or `None` if out of range.
pub fn get_history_entry(env: &Env, split_id: &Address, index: u64) -> Option<SplitHistoryEntry> {
    env.storage()
        .persistent()
        .get(&DataKey::SplitHistory(split_id.clone(), index))
}

/// Returns the total number of history entries for `split_id`.
pub fn get_history_count(env: &Env, split_id: &Address) -> u64 {
    history_count(env, split_id)
}

/// Returns the accumulated (unclaimed) balance for `recipient`.
pub fn get_balance(env: &Env, recipient: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::SplitBalance(recipient.clone()))
        .unwrap_or(0)
}

// ── Legacy helpers (kept for backward compatibility) ─────────────────────────

/// Royalty configuration for a piece of content (legacy).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyConfig {
    pub original_creator: Address,
    pub rate_bps: u32,
    pub max_depth: u32,
}

/// A content lineage record linking derivative to original (legacy).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentLineage {
    pub creator: Address,
    pub parent_creator: Address,
    pub royalty_config: RoyaltyConfig,
}

pub const MAX_DEPTH: u32 = 5;
pub const MAX_ROYALTY_BPS: u32 = 3_000;

/// Register a royalty configuration for a creator's content (legacy).
pub fn register_royalty(
    env: &Env,
    creator: &Address,
    original_creator: &Address,
    rate_bps: u32,
    max_depth: u32,
) {
    let depth = if max_depth == 0 {
        MAX_DEPTH
    } else {
        max_depth.min(MAX_DEPTH)
    };
    let config = RoyaltyConfig {
        original_creator: original_creator.clone(),
        rate_bps,
        max_depth: depth,
    };
    env.storage()
        .persistent()
        .set(&DataKey::RoyaltyConfig(creator.clone()), &config);
    let lineage = ContentLineage {
        creator: creator.clone(),
        parent_creator: original_creator.clone(),
        royalty_config: config,
    };
    env.storage()
        .persistent()
        .set(&DataKey::ContentLineage(creator.clone()), &lineage);
}

/// Distribute royalties along the lineage chain (legacy).
pub fn distribute_royalties(
    env: &Env,
    creator: &Address,
    token_addr: &Address,
    tip_amount: i128,
) -> i128 {
    let mut remaining = tip_amount;
    let mut current = creator.clone();
    let mut depth = 0u32;
    loop {
        let lineage: Option<ContentLineage> = env
            .storage()
            .persistent()
            .get(&DataKey::ContentLineage(current.clone()));
        let lineage = match lineage {
            Some(l) => l,
            None => break,
        };
        if depth >= lineage.royalty_config.max_depth {
            break;
        }
        let royalty = (remaining * lineage.royalty_config.rate_bps as i128) / 10_000;
        if royalty <= 0 {
            break;
        }
        let bal_key = DataKey::RoyaltyBalance(
            lineage.royalty_config.original_creator.clone(),
            token_addr.clone(),
        );
        let current_bal: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&bal_key, &(current_bal + royalty));
        env.events().publish(
            (symbol_short!("royalty"),),
            (
                lineage.royalty_config.original_creator.clone(),
                creator.clone(),
                royalty,
                depth,
            ),
        );
        remaining -= royalty;
        current = lineage.royalty_config.original_creator.clone();
        depth += 1;
    }
    remaining
}
