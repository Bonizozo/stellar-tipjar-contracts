//! Composable NFTs module (#311).
//!
//! Tip receipts that can be combined and nested with parent-child relationships.

use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Vec};

use crate::DataKey;

/// A composable NFT record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposableNft {
    pub id: u64,
    pub owner: Address,
    pub metadata: String,
    pub parent_id: Option<u64>,
    pub created_at: u64,
    pub active: bool,
}

/// A composition history entry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionEvent {
    pub parent_id: u64,
    pub child_id: u64,
    pub timestamp: u64,
    pub is_decompose: bool,
}

/// Composable NFT storage sub-keys.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NftKey {
    Counter,
    Record(u64),
    OwnerNfts(Address),
    Children(u64),
    History(u64),
}

/// Mint a new composable NFT.
pub fn mint(env: &Env, owner: &Address, metadata: String) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::Nft(NftKey::Counter))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&DataKey::Nft(NftKey::Counter), &(id + 1));

    let nft = ComposableNft {
        id,
        owner: owner.clone(),
        metadata,
        parent_id: None,
        created_at: env.ledger().timestamp(),
        active: true,
    };

    env.storage()
        .persistent()
        .set(&DataKey::Nft(NftKey::Record(id)), &nft);

    let mut owner_nfts: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::Nft(NftKey::OwnerNfts(owner.clone())))
        .unwrap_or_else(|| Vec::new(env));
    owner_nfts.push_back(id);
    env.storage()
        .persistent()
        .set(&DataKey::Nft(NftKey::OwnerNfts(owner.clone())), &owner_nfts);

    env.events()
        .publish((symbol_short!("nft_mint"),), (id, owner.clone()));

    id
}

/// Compose a child NFT into a parent (nested ownership).
///
/// Both NFTs must be owned by the same address.
pub fn compose(env: &Env, owner: &Address, parent_id: u64, child_id: u64) {
    assert!(parent_id != child_id, "cannot compose with self");

    let parent: ComposableNft = env
        .storage()
        .persistent()
        .get(&DataKey::Nft(NftKey::Record(parent_id)))
        .unwrap_or_else(|| panic!("parent NFT not found"));
    let mut child: ComposableNft = env
        .storage()
        .persistent()
        .get(&DataKey::Nft(NftKey::Record(child_id)))
        .unwrap_or_else(|| panic!("child NFT not found"));

    assert!(parent.owner == *owner, "not parent owner");
    assert!(child.owner == *owner, "not child owner");
    assert!(parent.active, "parent inactive");
    assert!(child.active, "child inactive");
    assert!(child.parent_id.is_none(), "child already composed");

    child.parent_id = Some(parent_id);
    env.storage()
        .persistent()
        .set(&DataKey::Nft(NftKey::Record(child_id)), &child);

    let mut children: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::Nft(NftKey::Children(parent_id)))
        .unwrap_or_else(|| Vec::new(env));
    children.push_back(child_id);
    env.storage()
        .persistent()
        .set(&DataKey::Nft(NftKey::Children(parent_id)), &children);

    push_history(
        env,
        parent_id,
        CompositionEvent {
            parent_id,
            child_id,
            timestamp: env.ledger().timestamp(),
            is_decompose: false,
        },
    );

    env.events().publish(
        (symbol_short!("nft_comp"),),
        (parent_id, child_id, owner.clone()),
    );
}

/// Decompose a child NFT from its parent.
pub fn decompose(env: &Env, owner: &Address, child_id: u64) {
    let mut child: ComposableNft = env
        .storage()
        .persistent()
        .get(&DataKey::Nft(NftKey::Record(child_id)))
        .unwrap_or_else(|| panic!("child NFT not found"));

    assert!(child.owner == *owner, "not owner");
    let parent_id = child.parent_id.unwrap_or_else(|| panic!("not composed"));

    child.parent_id = None;
    env.storage()
        .persistent()
        .set(&DataKey::Nft(NftKey::Record(child_id)), &child);

    let children: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::Nft(NftKey::Children(parent_id)))
        .unwrap_or_else(|| Vec::new(env));
    let mut remaining: Vec<u64> = Vec::new(env);
    for c in children.iter() {
        if c != child_id {
            remaining.push_back(c);
        }
    }
    env.storage()
        .persistent()
        .set(&DataKey::Nft(NftKey::Children(parent_id)), &remaining);

    push_history(
        env,
        parent_id,
        CompositionEvent {
            parent_id,
            child_id,
            timestamp: env.ledger().timestamp(),
            is_decompose: true,
        },
    );

    env.events().publish(
        (symbol_short!("nft_dcp"),),
        (parent_id, child_id, owner.clone()),
    );
}

/// Get an NFT record.
pub fn get_nft(env: &Env, nft_id: u64) -> Option<ComposableNft> {
    env.storage()
        .persistent()
        .get(&DataKey::Nft(NftKey::Record(nft_id)))
}

/// Get all NFT IDs owned by an address.
pub fn get_owner_nfts(env: &Env, owner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::Nft(NftKey::OwnerNfts(owner.clone())))
        .unwrap_or_else(|| Vec::new(env))
}

/// Get child NFT IDs for a parent.
pub fn get_children(env: &Env, parent_id: u64) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::Nft(NftKey::Children(parent_id)))
        .unwrap_or_else(|| Vec::new(env))
}

/// Get composition history for an NFT.
pub fn get_composition_history(env: &Env, nft_id: u64) -> Vec<CompositionEvent> {
    env.storage()
        .persistent()
        .get(&DataKey::Nft(NftKey::History(nft_id)))
        .unwrap_or_else(|| Vec::new(env))
}

fn push_history(env: &Env, nft_id: u64, event: CompositionEvent) {
    let mut hist: Vec<CompositionEvent> = env
        .storage()
        .persistent()
        .get(&DataKey::Nft(NftKey::History(nft_id)))
        .unwrap_or_else(|| Vec::new(env));
    hist.push_back(event);
    env.storage()
        .persistent()
        .set(&DataKey::Nft(NftKey::History(nft_id)), &hist);
}
