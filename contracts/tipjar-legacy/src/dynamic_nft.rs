//! Dynamic NFT module (#250).
//!
//! NFTs that evolve based on tipping activity and creator milestones.
//! Supports trait updates, rarity score calculation, and evolution history.

use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, String, Vec};

use crate::DataKey;

// ── Types ────────────────────────────────────────────────────────────────────

/// Rarity tier derived from the rarity score.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RarityTier {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

/// A dynamic NFT record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicNft {
    pub id: u64,
    pub owner: Address,
    /// Current evolution level (starts at 0).
    pub level: u32,
    /// Trait key→value map (e.g. "color" → "gold").
    pub traits: Map<String, String>,
    /// Rarity score in basis points (0–10 000).
    pub rarity_score: u32,
    pub rarity_tier: RarityTier,
    /// Cumulative tips that have contributed to this NFT's evolution.
    pub total_tips_contributed: i128,
    pub metadata_uri: String,
    pub minted_at: u64,
    pub updated_at: u64,
}

/// A single evolution history entry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvolutionEvent {
    pub nft_id: u64,
    pub old_level: u32,
    pub new_level: u32,
    pub old_rarity_score: u32,
    pub new_rarity_score: u32,
    pub timestamp: u64,
    pub trigger: String,
}

// ── DataKey sub-keys ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DynNftKey {
    Counter,
    Nft(u64),
    /// List of NFT IDs owned by an address.
    OwnerNfts(Address),
    /// Evolution history list for an NFT.
    History(u64),
}

// ── Rarity helpers ────────────────────────────────────────────────────────────

/// Compute rarity score from level and total tips.
///
/// Score = min(10_000, level * 500 + tips_bps)
/// where tips_bps = min(5_000, total_tips / 1_000_000) (1 XLM = 1 point, capped at 5 000).
pub fn compute_rarity_score(level: u32, total_tips: i128) -> u32 {
    let level_pts = (level as u32).saturating_mul(500);
    let tip_pts = (total_tips / 1_000_000).min(5_000) as u32;
    (level_pts.saturating_add(tip_pts)).min(10_000)
}

pub fn rarity_tier_from_score(score: u32) -> RarityTier {
    match score {
        0..=1999 => RarityTier::Common,
        2000..=3999 => RarityTier::Uncommon,
        4000..=5999 => RarityTier::Rare,
        6000..=7999 => RarityTier::Epic,
        _ => RarityTier::Legendary,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn load_nft(env: &Env, id: u64) -> DynamicNft {
    env.storage()
        .persistent()
        .get(&DataKey::DynamicNft(DynNftKey::Nft(id)))
        .unwrap_or_else(|| panic!("NFT not found"))
}

fn save_nft(env: &Env, nft: &DynamicNft) {
    env.storage()
        .persistent()
        .set(&DataKey::DynamicNft(DynNftKey::Nft(nft.id)), nft);
}

fn append_history(env: &Env, event: &EvolutionEvent) {
    let key = DataKey::DynamicNft(DynNftKey::History(event.nft_id));
    let mut history: Vec<EvolutionEvent> = env.storage().persistent().get(&key).unwrap_or_else(|| Vec::new(env));
    history.push_back(event.clone());
    env.storage().persistent().set(&key, &history);
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Mint a new dynamic NFT.
///
/// Returns the NFT ID.
/// Emits `("dnft_mint",)` with `(id, owner, level, rarity_score)`.
pub fn mint(
    env: &Env,
    owner: &Address,
    metadata_uri: String,
    initial_traits: Map<String, String>,
) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::DynamicNft(DynNftKey::Counter))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&DataKey::DynamicNft(DynNftKey::Counter), &(id + 1));

    let now = env.ledger().timestamp();
    let rarity_score = compute_rarity_score(0, 0);
    let nft = DynamicNft {
        id,
        owner: owner.clone(),
        level: 0,
        traits: initial_traits,
        rarity_score,
        rarity_tier: rarity_tier_from_score(rarity_score),
        total_tips_contributed: 0,
        metadata_uri,
        minted_at: now,
        updated_at: now,
    };

    save_nft(env, &nft);

    let mut owned: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::DynamicNft(DynNftKey::OwnerNfts(owner.clone())))
        .unwrap_or_else(|| Vec::new(env));
    owned.push_back(id);
    env.storage()
        .persistent()
        .set(&DataKey::DynamicNft(DynNftKey::OwnerNfts(owner.clone())), &owned);

    env.events().publish(
        (symbol_short!("dnft_mnt"),),
        (id, owner.clone(), 0u32, rarity_score),
    );

    id
}

/// Record a tip contribution to an NFT and trigger evolution if thresholds are met.
///
/// Evolution thresholds (cumulative tips in stroops):
/// - Level 1: 100 XLM (1_000_000_000)
/// - Level 2: 500 XLM
/// - Level 3: 1 000 XLM
/// - Level 4: 5 000 XLM
/// - Level 5: 10 000 XLM (max)
///
/// Emits `("dnft_tip",)` with `(id, tip_amount, new_total)`.
/// Emits `("dnft_evo",)` with `(id, old_level, new_level, new_rarity_score)` on evolution.
pub fn record_tip(env: &Env, nft_id: u64, tip_amount: i128) {
    let mut nft = load_nft(env, nft_id);
    nft.total_tips_contributed = nft.total_tips_contributed.saturating_add(tip_amount);

    let new_level = level_for_tips(nft.total_tips_contributed);
    let new_score = compute_rarity_score(new_level, nft.total_tips_contributed);

    if new_level > nft.level {
        let event = EvolutionEvent {
            nft_id,
            old_level: nft.level,
            new_level,
            old_rarity_score: nft.rarity_score,
            new_rarity_score: new_score,
            timestamp: env.ledger().timestamp(),
            trigger: String::from_str(env, "tip"),
        };
        append_history(env, &event);

        env.events().publish(
            (symbol_short!("dnft_evo"),),
            (nft_id, nft.level, new_level, new_score),
        );

        nft.level = new_level;
    }

    nft.rarity_score = new_score;
    nft.rarity_tier = rarity_tier_from_score(new_score);
    nft.updated_at = env.ledger().timestamp();
    save_nft(env, &nft);

    env.events().publish(
        (symbol_short!("dnft_tip"),),
        (nft_id, tip_amount, nft.total_tips_contributed),
    );
}

fn level_for_tips(total: i128) -> u32 {
    const XLM: i128 = 10_000_000; // 1 XLM in stroops
    if total >= 10_000 * XLM { 5 }
    else if total >= 5_000 * XLM { 4 }
    else if total >= 1_000 * XLM { 3 }
    else if total >= 500 * XLM { 2 }
    else if total >= 100 * XLM { 1 }
    else { 0 }
}

/// Update a trait on an NFT. Only the owner may call (enforced in lib.rs).
///
/// Emits `("dnft_trait",)` with `(id, key, value)`.
pub fn update_trait(env: &Env, nft_id: u64, key: String, value: String) {
    let mut nft = load_nft(env, nft_id);
    nft.traits.set(key.clone(), value.clone());
    nft.updated_at = env.ledger().timestamp();
    save_nft(env, &nft);
    env.events().publish(
        (symbol_short!("dnft_trt"),),
        (nft_id, key, value),
    );
}

/// Update the metadata URI. Only the owner may call (enforced in lib.rs).
pub fn update_metadata(env: &Env, nft_id: u64, metadata_uri: String) {
    let mut nft = load_nft(env, nft_id);
    nft.metadata_uri = metadata_uri;
    nft.updated_at = env.ledger().timestamp();
    save_nft(env, &nft);
}

/// Returns the NFT record.
pub fn get_nft(env: &Env, nft_id: u64) -> DynamicNft {
    load_nft(env, nft_id)
}

/// Returns all NFT IDs owned by `owner`.
pub fn get_owner_nfts(env: &Env, owner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::DynamicNft(DynNftKey::OwnerNfts(owner.clone())))
        .unwrap_or_else(|| Vec::new(env))
}

/// Returns the evolution history for an NFT.
pub fn get_evolution_history(env: &Env, nft_id: u64) -> Vec<EvolutionEvent> {
    env.storage()
        .persistent()
        .get(&DataKey::DynamicNft(DynNftKey::History(nft_id)))
        .unwrap_or_else(|| Vec::new(env))
}
