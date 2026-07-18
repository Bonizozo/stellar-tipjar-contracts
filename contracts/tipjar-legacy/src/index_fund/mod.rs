//! Tip Index Funds
//!
//! Allows users to deposit into diversified baskets of creators.
//! The fund tracks a weighted composition of creators, supports rebalancing,
//! calculates NAV (net asset value), and issues/redeems fund shares.

pub mod composition;
pub mod rebalance;
pub mod shares;

use soroban_sdk::{contracttype, Address, Env, Vec};

/// A single creator entry in the index with its weight.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexComponent {
    /// Creator address included in the index.
    pub creator: Address,
    /// Weight in basis points (sum of all components must equal 10_000).
    pub weight_bps: u32,
}

/// Index fund configuration and aggregate state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexFund {
    /// Unique fund identifier.
    pub id: u64,
    /// Human-readable name.
    pub name: soroban_sdk::String,
    /// Address of the manager who can rebalance.
    pub manager: Address,
    /// Token used for deposits/withdrawals.
    pub token: Address,
    /// Ordered list of creator components.
    pub components: Vec<IndexComponent>,
    /// Total fund shares outstanding.
    pub total_shares: i128,
    /// Total token value held by the fund (sum of all creator allocations).
    pub total_value: i128,
    /// Creation timestamp.
    pub created_at: u64,
    /// Last rebalance timestamp.
    pub last_rebalanced: u64,
    /// Whether the fund is active.
    pub active: bool,
}

/// A user's share position in a fund.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundShare {
    /// Fund this position belongs to.
    pub fund_id: u64,
    /// Holder address.
    pub holder: Address,
    /// Number of shares held.
    pub shares: i128,
    /// Cumulative amount deposited (for reference).
    pub deposited: i128,
}

/// Storage keys for index funds.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Fund record by ID.
    Fund(u64),
    /// Global fund counter.
    FundCounter,
    /// User share position keyed by (fund_id, holder).
    FundShare(u64, Address),
    /// Creator allocation within a fund keyed by (fund_id, creator).
    CreatorAlloc(u64, Address),
}

/// Minimum deposit amount (1 token unit).
pub const MIN_DEPOSIT: i128 = 1;
/// Initial share price denominator (1 share = 1 token on first deposit).
pub const INITIAL_SHARE_PRICE: i128 = 1_000_000; // 6 decimal precision

// ── storage helpers ──────────────────────────────────────────────────────────

/// Get a fund by ID.
pub fn get_fund(env: &Env, fund_id: u64) -> Option<IndexFund> {
    env.storage().persistent().get(&DataKey::Fund(fund_id))
}

/// Persist a fund record.
pub fn save_fund(env: &Env, fund: &IndexFund) {
    env.storage()
        .persistent()
        .set(&DataKey::Fund(fund.id), fund);
}

/// Get the next fund ID and increment the counter.
pub fn next_fund_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::FundCounter)
        .unwrap_or(0)
        + 1;
    env.storage().persistent().set(&DataKey::FundCounter, &id);
    id
}

/// Get a user's share position.
pub fn get_share(env: &Env, fund_id: u64, holder: &Address) -> FundShare {
    env.storage()
        .persistent()
        .get(&DataKey::FundShare(fund_id, holder.clone()))
        .unwrap_or(FundShare {
            fund_id,
            holder: holder.clone(),
            shares: 0,
            deposited: 0,
        })
}

/// Persist a user's share position.
pub fn save_share(env: &Env, share: &FundShare) {
    env.storage().persistent().set(
        &DataKey::FundShare(share.fund_id, share.holder.clone()),
        share,
    );
}

/// Get creator allocation within a fund (token amount allocated to that creator).
pub fn get_creator_alloc(env: &Env, fund_id: u64, creator: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::CreatorAlloc(fund_id, creator.clone()))
        .unwrap_or(0)
}

/// Set creator allocation within a fund.
pub fn set_creator_alloc(env: &Env, fund_id: u64, creator: &Address, amount: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::CreatorAlloc(fund_id, creator.clone()), &amount);
}

/// Calculate the current NAV per share (scaled by INITIAL_SHARE_PRICE).
/// Returns 0 if no shares are outstanding.
pub fn nav_per_share(fund: &IndexFund) -> i128 {
    if fund.total_shares == 0 {
        return INITIAL_SHARE_PRICE;
    }
    fund.total_value * INITIAL_SHARE_PRICE / fund.total_shares
}
