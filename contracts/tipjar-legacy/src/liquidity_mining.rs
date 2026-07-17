//! Tip Liquidity Mining Program
//!
//! Incentivises liquidity provision by rewarding LP-token holders with
//! mining rewards that vest linearly over a configurable cliff + duration.
//! Positions can be boosted by locking rewards for longer periods.

use soroban_sdk::{contracttype, panic_with_error, symbol_short, token, Address, Env, Vec};

use crate::{DataKey, TipJarError};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Seconds in a year used for APY calculations.
pub const SECONDS_PER_YEAR: u64 = 31_536_000;

/// Precision multiplier used throughout reward math (1e7).
pub const PRECISION: i128 = 10_000_000;

/// Maximum boost multiplier: 3× (stored as 3 * PRECISION).
pub const MAX_BOOST: i128 = 30_000_000;

/// Minimum boost multiplier: 1× (stored as 1 * PRECISION).
pub const MIN_BOOST: i128 = 10_000_000;

/// Maximum number of active mining programs.
pub const MAX_PROGRAMS: u32 = 50;

/// Maximum vesting duration: 4 years in seconds.
pub const MAX_VESTING_DURATION: u64 = 4 * SECONDS_PER_YEAR;

// ── Data types ────────────────────────────────────────────────────────────────

/// Global configuration for a liquidity mining program.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MiningProgram {
    /// Unique program identifier.
    pub id: u64,
    /// LP token that providers must stake.
    pub lp_token: Address,
    /// Token distributed as mining rewards.
    pub reward_token: Address,
    /// Total rewards allocated to this program.
    pub total_rewards: i128,
    /// Rewards already distributed (claimed + vested).
    pub distributed_rewards: i128,
    /// Reward emission rate in basis points per year (e.g. 2000 = 20 % APY).
    pub reward_rate_bps: u32,
    /// Vesting cliff in seconds before any rewards unlock.
    pub vesting_cliff: u64,
    /// Total vesting duration in seconds (must be >= cliff).
    pub vesting_duration: u64,
    /// Program start timestamp.
    pub start_time: u64,
    /// Program end timestamp (0 = no end).
    pub end_time: u64,
    /// Total LP tokens currently staked in this program.
    pub total_staked: i128,
    /// Whether the program is active.
    pub active: bool,
}

/// A provider's position in a mining program.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MiningPosition {
    /// Provider address.
    pub provider: Address,
    /// Mining program ID.
    pub program_id: u64,
    /// LP tokens staked.
    pub staked_amount: i128,
    /// Timestamp when the position was opened / last updated.
    pub entry_time: u64,
    /// Timestamp of the last reward accrual.
    pub last_update: u64,
    /// Rewards accrued but not yet vested.
    pub pending_rewards: i128,
    /// Rewards that have fully vested and are ready to claim.
    pub claimable_rewards: i128,
    /// Total rewards ever earned by this position (for vesting math).
    pub total_earned: i128,
    /// Boost multiplier applied to this position (PRECISION = 1×).
    pub boost_multiplier: i128,
    /// Timestamp until which the boost lock is active.
    pub boost_lock_until: u64,
}

/// Vesting schedule snapshot for a position.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VestingInfo {
    /// Total rewards earned so far.
    pub total_earned: i128,
    /// Amount already claimed.
    pub claimed: i128,
    /// Amount currently vested and claimable.
    pub vested: i128,
    /// Amount still locked in vesting.
    pub locked: i128,
    /// Seconds until cliff is reached (0 if already past cliff).
    pub cliff_remaining: u64,
    /// Seconds until fully vested (0 if fully vested).
    pub vesting_remaining: u64,
}

// ── Storage helpers ───────────────────────────────────────────────────────────

fn get_program(env: &Env, program_id: u64) -> Option<MiningProgram> {
    env.storage()
        .persistent()
        .get(&DataKey::LmProgram(program_id))
}

fn get_program_or_panic(env: &Env, program_id: u64) -> MiningProgram {
    get_program(env, program_id)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::LmProgramNotFound))
}

fn set_program(env: &Env, program: &MiningProgram) {
    env.storage()
        .persistent()
        .set(&DataKey::LmProgram(program.id), program);
}

fn get_position(env: &Env, program_id: u64, provider: &Address) -> Option<MiningPosition> {
    env.storage()
        .persistent()
        .get(&DataKey::LmPosition(program_id, provider.clone()))
}

fn get_position_or_panic(env: &Env, program_id: u64, provider: &Address) -> MiningPosition {
    get_position(env, program_id, provider)
        .unwrap_or_else(|| panic_with_error!(env, TipJarError::LmPositionNotFound))
}

fn set_position(env: &Env, position: &MiningPosition) {
    env.storage().persistent().set(
        &DataKey::LmPosition(position.program_id, position.provider.clone()),
        position,
    );
}

fn next_program_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::LmProgramCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .persistent()
        .set(&DataKey::LmProgramCounter, &next);
    next
}

fn get_provider_programs(env: &Env, provider: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::LmProviderPrograms(provider.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

fn add_provider_program(env: &Env, provider: &Address, program_id: u64) {
    let mut programs = get_provider_programs(env, provider);
    // Avoid duplicates
    for pid in programs.iter() {
        if pid == program_id {
            return;
        }
    }
    programs.push_back(program_id);
    env.storage()
        .persistent()
        .set(&DataKey::LmProviderPrograms(provider.clone()), &programs);
}

// ── Reward math ───────────────────────────────────────────────────────────────

/// Computes raw rewards for `amount` staked over `elapsed` seconds at `rate_bps`.
fn compute_rewards(amount: i128, rate_bps: u32, elapsed: u64) -> i128 {
    if amount <= 0 || elapsed == 0 || rate_bps == 0 {
        return 0;
    }
    amount * rate_bps as i128 * elapsed as i128 / (10_000 * SECONDS_PER_YEAR as i128)
}

/// Applies the boost multiplier to a raw reward amount.
fn apply_boost_factor(raw: i128, boost: i128) -> i128 {
    raw * boost / PRECISION
}

/// Accrues pending rewards into a position up to `now`.
fn accrue(program: &MiningProgram, position: &mut MiningPosition, now: u64) {
    if now <= position.last_update {
        return;
    }
    // Clamp to program end if set
    let effective_now = if program.end_time > 0 && now > program.end_time {
        program.end_time
    } else {
        now
    };
    if effective_now <= position.last_update {
        return;
    }
    let elapsed = effective_now - position.last_update;
    let raw = compute_rewards(position.staked_amount, program.reward_rate_bps, elapsed);
    let boosted = apply_boost_factor(raw, position.boost_multiplier);
    position.pending_rewards += boosted;
    position.total_earned += boosted;
    position.last_update = effective_now;
}

/// Computes how much of `total_earned` has vested at `now` given the program's
/// vesting cliff and duration, minus what has already been claimed.
fn compute_vested(program: &MiningProgram, position: &MiningPosition, now: u64) -> i128 {
    let elapsed_since_entry = now.saturating_sub(position.entry_time);

    // Before cliff: nothing vested
    if elapsed_since_entry < program.vesting_cliff {
        return 0;
    }

    let vesting_elapsed = elapsed_since_entry - program.vesting_cliff;
    let vesting_duration = program.vesting_duration - program.vesting_cliff;

    let vested_fraction = if vesting_duration == 0 || vesting_elapsed >= vesting_duration {
        PRECISION // 100%
    } else {
        vesting_elapsed as i128 * PRECISION / vesting_duration as i128
    };

    let total_vested = position.total_earned * vested_fraction / PRECISION;
    // Claimable = vested - already claimed (claimable_rewards tracks claimed)
    let already_claimed = position.claimable_rewards; // repurposed as "claimed" tracker
    (total_vested - already_claimed).max(0)
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Creates a new liquidity mining program. Returns the program ID.
///
/// * `reward_rate_bps` — annual reward rate in basis points (e.g. 2000 = 20 %).
/// * `vesting_cliff`   — seconds before any rewards unlock.
/// * `vesting_duration`— total vesting window in seconds (must be >= cliff).
/// * `end_time`        — program end timestamp; pass 0 for no end.
pub fn create_program(
    env: &Env,
    admin: &Address,
    lp_token: &Address,
    reward_token: &Address,
    total_rewards: i128,
    reward_rate_bps: u32,
    vesting_cliff: u64,
    vesting_duration: u64,
    end_time: u64,
) -> u64 {
    admin.require_auth();

    if total_rewards <= 0 {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }
    if reward_rate_bps == 0 {
        panic_with_error!(env, TipJarError::LmInvalidRate);
    }
    if vesting_duration < vesting_cliff {
        panic_with_error!(env, TipJarError::LmInvalidVesting);
    }
    if vesting_duration > MAX_VESTING_DURATION {
        panic_with_error!(env, TipJarError::LmInvalidVesting);
    }
    if end_time > 0 && end_time <= env.ledger().timestamp() {
        panic_with_error!(env, TipJarError::LmInvalidEndTime);
    }

    // Transfer reward tokens from admin into the contract
    token::Client::new(env, reward_token).transfer(
        admin,
        &env.current_contract_address(),
        &total_rewards,
    );

    let program_id = next_program_id(env);
    let now = env.ledger().timestamp();

    let program = MiningProgram {
        id: program_id,
        lp_token: lp_token.clone(),
        reward_token: reward_token.clone(),
        total_rewards,
        distributed_rewards: 0,
        reward_rate_bps,
        vesting_cliff,
        vesting_duration,
        start_time: now,
        end_time,
        total_staked: 0,
        active: true,
    };

    set_program(env, &program);

    env.events().publish(
        (symbol_short!("lm_create"),),
        (
            program_id,
            lp_token.clone(),
            reward_token.clone(),
            total_rewards,
            reward_rate_bps,
        ),
    );

    program_id
}

/// Stakes `amount` LP tokens into a mining program.
pub fn stake(env: &Env, provider: &Address, program_id: u64, amount: i128) {
    provider.require_auth();

    if amount <= 0 {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    let mut program = get_program_or_panic(env, program_id);

    if !program.active {
        panic_with_error!(env, TipJarError::LmProgramInactive);
    }
    if program.end_time > 0 && env.ledger().timestamp() >= program.end_time {
        panic_with_error!(env, TipJarError::LmProgramEnded);
    }

    let now = env.ledger().timestamp();

    let mut position = get_position(env, program_id, provider).unwrap_or(MiningPosition {
        provider: provider.clone(),
        program_id,
        staked_amount: 0,
        entry_time: now,
        last_update: now,
        pending_rewards: 0,
        claimable_rewards: 0,
        total_earned: 0,
        boost_multiplier: MIN_BOOST,
        boost_lock_until: 0,
    });

    // Accrue before changing stake
    accrue(&program, &mut position, now);

    // Transfer LP tokens from provider to contract
    token::Client::new(env, &program.lp_token).transfer(
        provider,
        &env.current_contract_address(),
        &amount,
    );

    position.staked_amount += amount;
    program.total_staked += amount;

    set_position(env, &position);
    set_program(env, &program);
    add_provider_program(env, provider, program_id);

    env.events().publish(
        (symbol_short!("lm_stake"),),
        (provider.clone(), program_id, amount, position.staked_amount),
    );
}

/// Unstakes `amount` LP tokens from a mining program.
/// Accrues rewards before reducing the position.
pub fn unstake(env: &Env, provider: &Address, program_id: u64, amount: i128) {
    provider.require_auth();

    if amount <= 0 {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    let mut program = get_program_or_panic(env, program_id);
    let mut position = get_position_or_panic(env, program_id, provider);

    if amount > position.staked_amount {
        panic_with_error!(env, TipJarError::InsufficientBalance);
    }

    let now = env.ledger().timestamp();
    accrue(&program, &mut position, now);

    position.staked_amount -= amount;
    program.total_staked -= amount;

    // Return LP tokens to provider
    token::Client::new(env, &program.lp_token).transfer(
        &env.current_contract_address(),
        provider,
        &amount,
    );

    set_position(env, &position);
    set_program(env, &program);

    env.events().publish(
        (symbol_short!("lm_unstk"),),
        (provider.clone(), program_id, amount, position.staked_amount),
    );
}

/// Claims all vested rewards for a provider in a program.
/// Returns the amount claimed.
pub fn claim_rewards(env: &Env, provider: &Address, program_id: u64) -> i128 {
    provider.require_auth();

    let program = get_program_or_panic(env, program_id);
    let mut position = get_position_or_panic(env, program_id, provider);

    let now = env.ledger().timestamp();
    accrue(&program, &mut position, now);

    let claimable = compute_vested(&program, &position, now);
    if claimable <= 0 {
        panic_with_error!(env, TipJarError::LmNothingToClaim);
    }

    // Guard against over-distribution
    let remaining = program.total_rewards - program.distributed_rewards;
    let actual_claim = claimable.min(remaining);
    if actual_claim <= 0 {
        panic_with_error!(env, TipJarError::LmRewardsExhausted);
    }

    // Update claimed tracker (claimable_rewards = total claimed)
    position.claimable_rewards += actual_claim;

    // Update program distribution counter
    let mut prog = program.clone();
    prog.distributed_rewards += actual_claim;
    set_program(env, &prog);

    // Transfer reward tokens to provider
    token::Client::new(env, &program.reward_token).transfer(
        &env.current_contract_address(),
        provider,
        &actual_claim,
    );

    set_position(env, &position);

    env.events().publish(
        (symbol_short!("lm_claim"),),
        (provider.clone(), program_id, actual_claim),
    );

    actual_claim
}

/// Applies a boost to a position by locking rewards for `lock_duration` seconds.
///
/// Boost multiplier scales linearly from 1× (no lock) to 3× (max lock = 1 year).
pub fn apply_boost(env: &Env, provider: &Address, program_id: u64, lock_duration: u64) {
    provider.require_auth();

    if lock_duration == 0 {
        panic_with_error!(env, TipJarError::InvalidDuration);
    }

    let program = get_program_or_panic(env, program_id);
    let mut position = get_position_or_panic(env, program_id, provider);

    let now = env.ledger().timestamp();
    accrue(&program, &mut position, now);

    // Boost = 1× + (lock_duration / SECONDS_PER_YEAR).min(1) × 2×
    // Clamped to [MIN_BOOST, MAX_BOOST]
    let max_lock = SECONDS_PER_YEAR;
    let clamped = lock_duration.min(max_lock);
    let extra_boost = (MAX_BOOST - MIN_BOOST) * clamped as i128 / max_lock as i128;
    let new_boost = (MIN_BOOST + extra_boost).min(MAX_BOOST);

    // Only allow increasing the boost
    if new_boost <= position.boost_multiplier {
        panic_with_error!(env, TipJarError::LmBoostTooLow);
    }

    position.boost_multiplier = new_boost;
    position.boost_lock_until = now + lock_duration;

    set_position(env, &position);

    env.events().publish(
        (symbol_short!("lm_boost"),),
        (
            provider.clone(),
            program_id,
            new_boost,
            position.boost_lock_until,
        ),
    );
}

/// Deactivates a mining program. Existing positions can still claim vested rewards.
pub fn deactivate_program(env: &Env, admin: &Address, program_id: u64) {
    admin.require_auth();

    let mut program = get_program_or_panic(env, program_id);
    if !program.active {
        panic_with_error!(env, TipJarError::LmProgramInactive);
    }

    program.active = false;
    set_program(env, &program);

    env.events()
        .publish((symbol_short!("lm_deact"),), (program_id,));
}

/// Returns the current vesting info for a provider's position.
pub fn get_vesting_info(env: &Env, provider: &Address, program_id: u64) -> VestingInfo {
    let program = get_program_or_panic(env, program_id);
    let mut position = get_position_or_panic(env, program_id, provider);

    let now = env.ledger().timestamp();
    accrue(&program, &mut position, now);

    let elapsed_since_entry = now.saturating_sub(position.entry_time);

    let cliff_remaining = program.vesting_cliff.saturating_sub(elapsed_since_entry);

    let vesting_remaining = program.vesting_duration.saturating_sub(elapsed_since_entry);

    let vested = compute_vested(&program, &position, now);
    let claimed = position.claimable_rewards;
    let locked = (position.total_earned - claimed - vested).max(0);

    VestingInfo {
        total_earned: position.total_earned,
        claimed,
        vested,
        locked,
        cliff_remaining,
        vesting_remaining,
    }
}

/// Returns a mining program by ID.
pub fn get_program_info(env: &Env, program_id: u64) -> MiningProgram {
    get_program_or_panic(env, program_id)
}

/// Returns a provider's position in a program.
pub fn get_position_info(env: &Env, provider: &Address, program_id: u64) -> MiningPosition {
    let program = get_program_or_panic(env, program_id);
    let mut position = get_position_or_panic(env, program_id, provider);
    let now = env.ledger().timestamp();
    accrue(&program, &mut position, now);
    position
}

/// Returns all program IDs a provider has participated in.
pub fn get_provider_program_ids(env: &Env, provider: &Address) -> Vec<u64> {
    get_provider_programs(env, provider)
}

/// Returns the pending (not yet vested) rewards for a provider.
pub fn get_pending_rewards(env: &Env, provider: &Address, program_id: u64) -> i128 {
    let program = get_program_or_panic(env, program_id);
    let mut position = match get_position(env, program_id, provider) {
        Some(p) => p,
        None => return 0,
    };
    let now = env.ledger().timestamp();
    accrue(&program, &mut position, now);
    position.pending_rewards
}
