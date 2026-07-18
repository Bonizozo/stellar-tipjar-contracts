//! Tip sharding for parallel tip processing across multiple shards.
//!
//! Tips are assigned to shards based on a hash of the sender address.
//! Each shard maintains independent state. Cross-shard communication
//! is handled via shard transfer records. Rebalancing redistributes
//! load when shards become uneven.

use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{contracttype, symbol_short, Address, BytesN, Env};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Default number of shards.
pub const DEFAULT_SHARD_COUNT: u32 = 8;

/// Maximum number of shards.
pub const MAX_SHARD_COUNT: u32 = 64;

/// Rebalance threshold: trigger when a shard's tip count exceeds this
/// multiple of the average.
pub const REBALANCE_THRESHOLD_BPS: u32 = 15000; // 150%

// ── Types ────────────────────────────────────────────────────────────────────

/// State of a shard.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShardStatus {
    /// Shard is active and accepting tips.
    Active,
    /// Shard is being drained for rebalancing.
    Draining,
    /// Shard is inactive.
    Inactive,
}

/// Per-shard state record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShardState {
    /// Shard index (0-based).
    pub shard_id: u32,
    /// Current status.
    pub status: ShardStatus,
    /// Total number of tips processed by this shard.
    pub tip_count: u64,
    /// Total tip volume (sum of amounts) processed.
    pub total_volume: i128,
    /// Ledger timestamp of last activity.
    pub last_active: u64,
}

/// A cross-shard transfer record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShardTransfer {
    /// Unique transfer ID.
    pub id: u64,
    /// Source shard.
    pub from_shard: u32,
    /// Destination shard.
    pub to_shard: u32,
    /// Tip ID being transferred.
    pub tip_id: u64,
    /// Amount being transferred.
    pub amount: i128,
    /// Token address.
    pub token: Address,
    /// Ledger timestamp.
    pub created_at: u64,
    /// Whether the transfer has been finalized.
    pub finalized: bool,
}

/// Global sharding configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShardConfig {
    /// Number of active shards.
    pub shard_count: u32,
    /// Whether automatic rebalancing is enabled.
    pub auto_rebalance: bool,
    /// Rebalance threshold in basis points (relative to average load).
    pub rebalance_threshold_bps: u32,
}

// ── Storage sub-keys ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShardKey {
    /// Global shard configuration.
    Config,
    /// ShardState keyed by shard_id.
    State(u32),
    /// Shard assignment for a given address (cached).
    Assignment(Address),
    /// Cross-shard transfer counter.
    TransferCounter,
    /// Cross-shard transfer record keyed by transfer ID.
    Transfer(u64),
}

// ── Storage helpers ──────────────────────────────────────────────────────────

fn get_config(env: &Env) -> ShardConfig {
    env.storage()
        .persistent()
        .get(&DataKey::Shard(ShardKey::Config))
        .unwrap_or(ShardConfig {
            shard_count: DEFAULT_SHARD_COUNT,
            auto_rebalance: true,
            rebalance_threshold_bps: REBALANCE_THRESHOLD_BPS,
        })
}

fn save_config(env: &Env, config: &ShardConfig) {
    env.storage()
        .persistent()
        .set(&DataKey::Shard(ShardKey::Config), config);
}

fn get_shard_state(env: &Env, shard_id: u32) -> ShardState {
    env.storage()
        .persistent()
        .get(&DataKey::Shard(ShardKey::State(shard_id)))
        .unwrap_or(ShardState {
            shard_id,
            status: ShardStatus::Active,
            tip_count: 0,
            total_volume: 0,
            last_active: 0,
        })
}

fn save_shard_state(env: &Env, state: &ShardState) {
    env.storage()
        .persistent()
        .set(&DataKey::Shard(ShardKey::State(state.shard_id)), state);
}

fn next_transfer_id(env: &Env) -> u64 {
    let key = DataKey::Shard(ShardKey::TransferCounter);
    let id: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage().persistent().set(&key, &(id + 1));
    id
}

// ── Shard assignment ─────────────────────────────────────────────────────────

/// Compute the shard assignment for an address deterministically.
///
/// Uses SHA-256 of the XDR-encoded address, then takes the first 4 bytes
/// as a u32 and mods by `shard_count`.
pub fn assign_shard(env: &Env, addr: &Address) -> u32 {
    let config = get_config(env);
    let addr_bytes = addr.to_xdr(env);
    let hash: BytesN<32> = env.crypto().sha256(&addr_bytes).to_bytes();
    // Read first 4 bytes as big-endian u32.
    let b0 = hash.get(0).unwrap_or(0) as u32;
    let b1 = hash.get(1).unwrap_or(0) as u32;
    let b2 = hash.get(2).unwrap_or(0) as u32;
    let b3 = hash.get(3).unwrap_or(0) as u32;
    let val = (b0 << 24) | (b1 << 16) | (b2 << 8) | b3;
    val % config.shard_count
}

/// Get (or compute and cache) the shard assignment for an address.
pub fn get_assignment(env: &Env, addr: &Address) -> u32 {
    let key = DataKey::Shard(ShardKey::Assignment(addr.clone()));
    if let Some(cached) = env.storage().persistent().get::<_, u32>(&key) {
        return cached;
    }
    let shard_id = assign_shard(env, addr);
    env.storage().persistent().set(&key, &shard_id);
    shard_id
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Initialize sharding configuration.
pub fn init_sharding(env: &Env, admin: &Address, shard_count: u32, auto_rebalance: bool) {
    admin.require_auth();
    assert!(
        shard_count > 0 && shard_count <= MAX_SHARD_COUNT,
        "invalid shard count"
    );
    let config = ShardConfig {
        shard_count,
        auto_rebalance,
        rebalance_threshold_bps: REBALANCE_THRESHOLD_BPS,
    };
    save_config(env, &config);

    // Initialize all shard states.
    for i in 0..shard_count {
        let state = ShardState {
            shard_id: i,
            status: ShardStatus::Active,
            tip_count: 0,
            total_volume: 0,
            last_active: 0,
        };
        save_shard_state(env, &state);
    }

    env.events()
        .publish((symbol_short!("sh_init"),), (shard_count, auto_rebalance));
}

/// Record a tip being processed by the appropriate shard.
///
/// Returns the shard ID that handled the tip.
pub fn record_tip_in_shard(env: &Env, sender: &Address, amount: i128, tip_id: u64) -> u32 {
    let shard_id = get_assignment(env, sender);
    let mut state = get_shard_state(env, shard_id);

    assert!(
        matches!(state.status, ShardStatus::Active),
        "shard not active"
    );

    state.tip_count = state.tip_count.saturating_add(1);
    state.total_volume = state.total_volume.saturating_add(amount);
    state.last_active = env.ledger().timestamp();
    save_shard_state(env, &state);

    env.events()
        .publish((symbol_short!("sh_tip"),), (shard_id, tip_id, amount));

    // Trigger rebalance check if auto-rebalance is enabled.
    let config = get_config(env);
    if config.auto_rebalance {
        maybe_rebalance(env, &config);
    }

    shard_id
}

/// Initiate a cross-shard transfer for a tip.
///
/// Returns the transfer ID.
pub fn cross_shard_transfer(
    env: &Env,
    from_shard: u32,
    to_shard: u32,
    tip_id: u64,
    amount: i128,
    token: &Address,
) -> u64 {
    let config = get_config(env);
    assert!(from_shard < config.shard_count, "invalid from_shard");
    assert!(to_shard < config.shard_count, "invalid to_shard");
    assert!(from_shard != to_shard, "same shard transfer");

    let transfer_id = next_transfer_id(env);
    let transfer = ShardTransfer {
        id: transfer_id,
        from_shard,
        to_shard,
        tip_id,
        amount,
        token: token.clone(),
        created_at: env.ledger().timestamp(),
        finalized: false,
    };

    env.storage()
        .persistent()
        .set(&DataKey::Shard(ShardKey::Transfer(transfer_id)), &transfer);

    env.events().publish(
        (symbol_short!("sh_xfer"),),
        (transfer_id, from_shard, to_shard, amount),
    );

    transfer_id
}

/// Finalize a cross-shard transfer, updating destination shard state.
pub fn finalize_transfer(env: &Env, transfer_id: u64) {
    let key = DataKey::Shard(ShardKey::Transfer(transfer_id));
    let mut transfer: ShardTransfer = env
        .storage()
        .persistent()
        .get(&key)
        .expect("transfer not found");

    assert!(!transfer.finalized, "already finalized");

    let mut dest = get_shard_state(env, transfer.to_shard);
    dest.tip_count = dest.tip_count.saturating_add(1);
    dest.total_volume = dest.total_volume.saturating_add(transfer.amount);
    dest.last_active = env.ledger().timestamp();
    save_shard_state(env, &dest);

    transfer.finalized = true;
    env.storage().persistent().set(&key, &transfer);

    env.events()
        .publish((symbol_short!("sh_fin"),), (transfer_id, transfer.to_shard));
}

/// Rebalance shards by marking overloaded shards as Draining.
///
/// Called automatically when `auto_rebalance` is enabled.
pub fn rebalance(env: &Env, admin: &Address) {
    admin.require_auth();
    let config = get_config(env);
    maybe_rebalance(env, &config);
}

/// Get the state of a specific shard.
pub fn get_shard(env: &Env, shard_id: u32) -> ShardState {
    get_shard_state(env, shard_id)
}

/// Get the sharding configuration.
pub fn get_config_pub(env: &Env) -> ShardConfig {
    get_config(env)
}

/// Get a cross-shard transfer record.
pub fn get_transfer(env: &Env, transfer_id: u64) -> Option<ShardTransfer> {
    env.storage()
        .persistent()
        .get(&DataKey::Shard(ShardKey::Transfer(transfer_id)))
}

// ── Internal ─────────────────────────────────────────────────────────────────

fn maybe_rebalance(env: &Env, config: &ShardConfig) {
    let n = config.shard_count;
    let mut total_tips: u64 = 0;

    for i in 0..n {
        let s = get_shard_state(env, i);
        total_tips = total_tips.saturating_add(s.tip_count);
    }

    if total_tips == 0 {
        return;
    }

    let avg = total_tips / n as u64;
    let threshold = avg.saturating_mul(config.rebalance_threshold_bps as u64) / 10000;

    for i in 0..n {
        let mut s = get_shard_state(env, i);
        if s.tip_count > threshold && matches!(s.status, ShardStatus::Active) {
            s.status = ShardStatus::Draining;
            save_shard_state(env, &s);
            env.events()
                .publish((symbol_short!("sh_drain"),), (i, s.tip_count, avg));
        } else if s.tip_count <= avg && matches!(s.status, ShardStatus::Draining) {
            s.status = ShardStatus::Active;
            save_shard_state(env, &s);
        }
    }
}
