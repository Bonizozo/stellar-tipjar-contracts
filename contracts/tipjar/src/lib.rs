#![no_std]
#![deny(unsafe_code)]
#![deny(missing_docs)]

pub mod interfaces;
pub mod integrations;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short,
    token, Address, BytesN, Env, Map, String, Vec,
};

pub mod upgrade;

#[cfg(test)]
extern crate std;

// Advanced Event System
pub mod events;

// Automated Market Maker
pub mod amm;

// Governance System
pub mod governance;

// Staking and Rewards
pub mod staking;

// Yield farming
pub mod farming;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipWithMessage {
    pub sender: Address,
    pub creator: Address,
    pub amount: i128,
    pub message: String,
    pub metadata: Map<String, String>,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Milestone {
    pub id: u64,
    pub creator: Address,
    pub goal_amount: i128,
    pub current_amount: i128,
    pub description: String,
    pub deadline: Option<u64>,
    pub completed: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchTip {
    pub creator: Address,
    pub token: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockedTip {
    pub sender: Address,
    pub creator: Address,
    pub token: Address,
    pub amount: i128,
    pub unlock_timestamp: u64,
}

/// Internal record of a tip for refund tracking.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipRecord {
    pub id: u64,
    pub sender: Address,
    pub creator: Address,
    pub token: Address,
    pub amount: i128,
    pub timestamp: u64,
    pub refunded: bool,
    pub refund_requested: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimePeriod {
    AllTime,
    Monthly,
    Weekly,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaderboardEntry {
    pub address: Address,
    pub total_amount: i128,
    pub tip_count: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParticipantKind {
    Tipper,
    Creator,
}

/// Query parameters for tip history retrieval.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipHistoryQuery {
    pub creator: Option<Address>,
    pub sender: Option<Address>,
    pub min_amount: Option<i128>,
    pub max_amount: Option<i128>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub limit: u32,
    pub offset: u32,
}

/// Role enum for role-based access control.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Role {
    Admin,
    Moderator,
    Creator,
}

/// A sponsor-funded tip matching program.
///
/// `match_ratio` is in basis points: 100 = 1:1, 200 = 2:1.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchingProgram {
    pub id: u64,
    pub sponsor: Address,
    pub creator: Address,
    pub token: Address,
    pub match_ratio: u32,
    pub max_match_amount: i128,
    pub current_matched: i128,
    pub active: bool,
}

/// Storage layout for persistent contract data.
#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    /// Token contract address whitelist state (bool).
    TokenWhitelist(Address),
    /// Creator's currently withdrawable balance held by this contract per token.
    CreatorBalance(Address, Address), // (creator, token)
    /// Historical total tips ever received by creator per token.
    CreatorTotal(Address, Address),   // (creator, token)
    /// Emergency pause state (bool).
    Paused,
    /// Contract administrator (Address).
    Admin,
    /// Messages appended for a creator.
    CreatorMessages(Address),
    /// Current number of milestones for a creator (used for ID).
    MilestoneCounter(Address),
    /// Data for a specific milestone.
    Milestone(Address, u64),
    /// Active milestone IDs for a creator to track.
    ActiveMilestones(Address),
    /// Maps an address to its assigned Role (persistent).
    UserRole(Address),
    /// Maps a Role to the set of addresses holding it (persistent).
    RoleMembers(Role),
    /// Aggregate stats for a tipper in a specific time bucket (bucket_id: 0=AllTime, YYYYMM=Monthly, YYYYWW=Weekly).
    TipperAggregate(Address, u32),
    /// Aggregate stats for a creator in a specific time bucket.
    CreatorAggregate(Address, u32),
    /// Ordered list of all known tipper addresses for a bucket.
    TipperParticipants(u32),
    /// Ordered list of all known creator addresses for a bucket.
    CreatorParticipants(u32),
    /// Locked tip record keyed by (creator, tip_id).
    LockedTip(Address, u64),
    /// Per-creator counter for assigning tip IDs (u64).
    LockedTipCounter(Address),
    /// Global matching program counter.
    MatchingCounter,
    /// Individual matching program by ID.
    MatchingProgram(u64),
    /// Matching program IDs indexed under a creator.
    CreatorMatchingPrograms(Address),
    /// Individual tip record by global tip ID.
    TipRecord(u64),
    /// Global tip counter for assigning tip IDs.
    TipCounter,
    /// Monotonic farming pool counter.
    FarmingPoolCounter,
    /// Farming pool configuration and aggregate state by pool ID.
    FarmingPool(u64),
    /// Farming position by `(pool_id, staker)`.
    FarmingPosition(u64, Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TipJarError {
    AlreadyInitialized = 1,
    TokenNotWhitelisted = 2,
    InvalidAmount = 3,
    NothingToWithdraw = 4,
    MessageTooLong = 5,
    MilestoneNotFound = 6,
    MilestoneAlreadyCompleted = 7,
    InvalidGoalAmount = 8,
    Unauthorized = 9,
    RoleNotFound = 10,
    BatchTooLarge = 11,
    InsufficientBalance = 12,
    InvalidUnlockTime = 13,
    TipStillLocked = 14,
    LockedTipNotFound = 15,
    MatchingProgramNotFound = 16,
    MatchingProgramInactive = 17,
    InvalidMatchRatio = 18,
    DexNotConfigured = 19,
    NftNotConfigured = 20,
    SwapFailed = 21,
    FarmingPoolNotFound = 22,
    FarmingPositionNotFound = 23,
    FarmingLockNotExpired = 24,
}

#[contract]
pub struct TipJarContract;

#[contractimpl]
impl TipJarContract {
    /// One-time setup to choose the administrator for the TipJar.
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, TipJarError::AlreadyInitialized as u32);
        }
        env.storage().instance().put(&DataKey::Admin, &admin);
    }

    /// Creates a new farming pool for LP staking rewards.
    pub fn create_farming_pool(
        env: Env,
        creator: Address,
        lp_token: Address,
        reward_token: Address,
        reward_rate_bps: u32,
        lock_period: u64,
    ) -> u64 {
        creator.require_auth();
        farming::pool::create_pool(
            &env,
            &lp_token,
            &reward_token,
            reward_rate_bps,
            lock_period,
        )
    }

    /// Stakes LP tokens into a farming pool.
    pub fn farm_stake(env: Env, staker: Address, pool_id: u64, amount: i128) {
        staker.require_auth();
        farming::pool::stake(&env, &staker, pool_id, amount);
    }

    /// Harvests accrued rewards from a farming pool without unstaking principal.
    pub fn farm_harvest(env: Env, staker: Address, pool_id: u64) -> i128 {
        staker.require_auth();
        farming::pool::harvest(&env, &staker, pool_id)
    }

    /// Unstakes LP principal from a farming pool after lock period.
    pub fn farm_unstake(env: Env, staker: Address, pool_id: u64, amount: i128) {
        staker.require_auth();
        farming::pool::unstake(&env, &staker, pool_id, amount);
    }

    /// Returns a farming pool by ID.
    pub fn get_farming_pool(env: Env, pool_id: u64) -> Option<farming::FarmingPool> {
        farming::pool::get_pool(&env, pool_id)
    }

    /// Returns a farming position by pool and staker.
    pub fn get_farming_position(
        env: Env,
        pool_id: u64,
        staker: Address,
    ) -> Option<farming::FarmingPosition> {
        farming::pool::get_position(&env, pool_id, &staker)
    }

    /// Returns configured APY basis points for a farming pool.
    pub fn get_farming_apy_bps(env: Env, pool_id: u64) -> u32 {
        let pool = farming::pool::get_pool_or_panic(&env, pool_id);
        farming::rewards::calculate_apy_bps(&pool)
    }
}