//! Tip Inheritance Mechanism
//!
//! Allows creators to designate beneficiaries for unclaimed tips.
//! Supports multiple beneficiaries with share-based allocation,
//! time-based triggers, and on-chain claim processing.

use soroban_sdk::{contracttype, symbol_short, Address, Env, Vec};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single beneficiary with a share of the inheritance.
///
/// `share_bps` is in basis points (10_000 = 100%).
/// All beneficiaries for a creator must sum to 10_000.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Beneficiary {
    pub address: Address,
    /// Share in basis points (0–10_000).
    pub share_bps: u32,
}

/// Inheritance rules set by a creator.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InheritanceConfig {
    pub creator: Address,
    pub beneficiaries: Vec<Beneficiary>,
    /// Seconds of inactivity before inheritance becomes claimable.
    pub inactivity_trigger: u64,
    /// Ledger timestamp of the creator's last withdrawal or activity.
    pub last_activity: u64,
    pub active: bool,
}

/// A processed inheritance claim.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InheritanceClaim {
    pub claim_id: u64,
    pub creator: Address,
    pub claimant: Address,
    pub amount: i128,
    pub claimed_at: u64,
}

// ---------------------------------------------------------------------------
// Storage helpers
// ---------------------------------------------------------------------------

fn config_key(creator: &Address) -> (soroban_sdk::Symbol, Address) {
    (symbol_short!("inh_cfg"), creator.clone())
}

fn claim_count_key() -> soroban_sdk::Symbol {
    symbol_short!("inh_cnt")
}

fn claim_key(id: u64) -> (soroban_sdk::Symbol, u64) {
    (symbol_short!("inh_clm"), id)
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Set or update the inheritance configuration for a creator.
///
/// `beneficiaries` shares must sum to exactly 10_000 bps.
/// `inactivity_trigger` is the number of seconds of inactivity required
/// before beneficiaries may claim.
pub fn set_inheritance(
    env: &Env,
    creator: &Address,
    beneficiaries: Vec<Beneficiary>,
    inactivity_trigger: u64,
) -> InheritanceConfig {
    creator.require_auth();

    assert!(!beneficiaries.is_empty(), "need at least one beneficiary");
    assert!(inactivity_trigger > 0, "trigger must be positive");

    // Validate shares sum to 10_000.
    let total: u32 = beneficiaries.iter().map(|b| b.share_bps).sum();
    assert!(total == 10_000, "shares must sum to 10000 bps");

    let cfg = InheritanceConfig {
        creator: creator.clone(),
        beneficiaries,
        inactivity_trigger,
        last_activity: env.ledger().timestamp(),
        active: true,
    };

    env.storage().persistent().set(&config_key(creator), &cfg);

    env.events().publish(
        (symbol_short!("inh"), symbol_short!("set")),
        creator.clone(),
    );

    cfg
}

/// Get the inheritance configuration for a creator.
pub fn get_inheritance(env: &Env, creator: &Address) -> Option<InheritanceConfig> {
    env.storage().persistent().get(&config_key(creator))
}

/// Update the creator's last-activity timestamp (call on withdrawal/tip).
pub fn touch_activity(env: &Env, creator: &Address) {
    if let Some(mut cfg) = get_inheritance(env, creator) {
        cfg.last_activity = env.ledger().timestamp();
        env.storage().persistent().set(&config_key(creator), &cfg);
    }
}

// ---------------------------------------------------------------------------
// Claiming
// ---------------------------------------------------------------------------

/// Check whether the inactivity trigger has elapsed for a creator.
pub fn is_claimable(env: &Env, creator: &Address) -> bool {
    match get_inheritance(env, creator) {
        Some(cfg) if cfg.active => {
            let elapsed = env.ledger().timestamp() - cfg.last_activity;
            elapsed >= cfg.inactivity_trigger
        }
        _ => false,
    }
}

/// Claim the inheritance share for a beneficiary.
///
/// `unclaimed_balance` is the creator's current escrowed balance.
/// Returns the amount allocated to `claimant`.
pub fn claim_inheritance(
    env: &Env,
    claimant: &Address,
    creator: &Address,
    unclaimed_balance: i128,
) -> i128 {
    claimant.require_auth();

    assert!(
        is_claimable(env, creator),
        "inheritance not yet claimable"
    );
    assert!(unclaimed_balance > 0, "no balance to inherit");

    let cfg: InheritanceConfig = get_inheritance(env, creator).expect("no inheritance config");

    // Find claimant's share.
    let share_bps = cfg
        .beneficiaries
        .iter()
        .find(|b| b.address == *claimant)
        .map(|b| b.share_bps)
        .expect("claimant is not a beneficiary");

    let amount = unclaimed_balance * share_bps as i128 / 10_000;
    assert!(amount > 0, "share rounds to zero");

    // Record the claim.
    let id: u64 = env
        .storage()
        .persistent()
        .get(&claim_count_key())
        .unwrap_or(0u64)
        + 1;
    env.storage().persistent().set(&claim_count_key(), &id);

    let record = InheritanceClaim {
        claim_id: id,
        creator: creator.clone(),
        claimant: claimant.clone(),
        amount,
        claimed_at: env.ledger().timestamp(),
    };
    env.storage().persistent().set(&claim_key(id), &record);

    env.events().publish(
        (symbol_short!("inh"), symbol_short!("claim")),
        (claimant.clone(), creator.clone(), amount),
    );

    amount
}

/// Get a recorded inheritance claim by ID.
pub fn get_claim(env: &Env, claim_id: u64) -> Option<InheritanceClaim> {
    env.storage().persistent().get(&claim_key(claim_id))
}

/// Deactivate inheritance (creator revokes it).
pub fn revoke_inheritance(env: &Env, creator: &Address) {
    creator.require_auth();
    if let Some(mut cfg) = get_inheritance(env, creator) {
        cfg.active = false;
        env.storage().persistent().set(&config_key(creator), &cfg);
        env.events().publish(
            (symbol_short!("inh"), symbol_short!("revoke")),
            creator.clone(),
        );
    }
}
