//! Tip Portfolio Rebalancing Module
//!
//! Automatic portfolio rebalancing for diversified tip investments.
//! Supports rule-based rebalancing, optimal allocation calculation,
//! transaction cost accounting, and full rebalancing history.

use soroban_sdk::{contracterror, contracttype, panic_with_error, Address, Env, Vec};

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum PortfolioError {
    /// Portfolio not found.
    PortfolioNotFound = 1,
    /// Asset weights must sum to 10 000 bps.
    InvalidWeights = 2,
    /// Rebalancing is not yet due (frequency constraint).
    RebalanceTooFrequent = 3,
    /// Drift is within tolerance; no rebalance needed.
    DriftWithinTolerance = 4,
    /// Caller is not the portfolio owner.
    Unauthorized = 5,
    /// Portfolio must have at least one asset.
    EmptyPortfolio = 6,
    /// Arithmetic overflow during calculation.
    CalculationOverflow = 7,
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// A single asset target within a portfolio.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetTarget {
    /// Token address for this asset.
    pub token: Address,
    /// Target weight in basis points (sum across all assets must equal 10 000).
    pub target_bps: u32,
    /// Minimum acceptable weight before a rebalance is forced.
    pub min_bps: u32,
    /// Maximum acceptable weight before a rebalance is forced.
    pub max_bps: u32,
}

/// Portfolio configuration owned by a creator/investor.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Portfolio {
    /// Unique portfolio ID.
    pub portfolio_id: u64,
    /// Owner of this portfolio.
    pub owner: Address,
    /// Asset targets (weights must sum to 10 000 bps).
    pub assets: Vec<AssetTarget>,
    /// Drift tolerance in bps before auto-rebalance triggers.
    pub drift_tolerance_bps: u32,
    /// Minimum seconds between rebalances.
    pub rebalance_frequency_seconds: u64,
    /// Transaction cost in bps (deducted from rebalance amounts).
    pub tx_cost_bps: u32,
    /// Timestamp of the last rebalance.
    pub last_rebalance: u64,
    /// Total number of rebalances performed.
    pub rebalance_count: u64,
    /// Creation timestamp.
    pub created_at: u64,
    /// Whether the portfolio is active.
    pub active: bool,
}

/// A single rebalancing history entry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RebalanceHistoryEntry {
    /// Sequential entry index within the portfolio.
    pub entry_index: u64,
    /// Portfolio ID.
    pub portfolio_id: u64,
    /// Timestamp of the rebalance.
    pub timestamp: u64,
    /// Total value rebalanced (sum of absolute adjustments).
    pub total_adjusted: i128,
    /// Transaction costs paid.
    pub tx_costs_paid: i128,
    /// Number of assets adjusted.
    pub assets_adjusted: u32,
}

/// Adjustment required for a single asset during rebalancing.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetAdjustment {
    /// Token to adjust.
    pub token: Address,
    /// Positive = buy more; negative = sell.
    pub delta: i128,
    /// Transaction cost for this adjustment.
    pub tx_cost: i128,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortfolioKey {
    /// Next portfolio ID counter.
    Ctr,
    /// Portfolio record keyed by portfolio_id.
    Record(u64),
    /// List of portfolio IDs for an owner.
    OwnerPortfolios(Address),
    /// Rebalance history entry keyed by (portfolio_id, entry_index).
    History(u64, u64),
    /// Total history entries for a portfolio.
    HistoryCount(u64),
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Create a new portfolio with the given asset targets.
///
/// Asset weights must sum to exactly 10 000 bps.
/// Emits `("port_new",)` with data `(portfolio_id, owner, asset_count)`.
pub fn create_portfolio(
    env: &Env,
    owner: Address,
    assets: Vec<AssetTarget>,
    drift_tolerance_bps: u32,
    rebalance_frequency_seconds: u64,
    tx_cost_bps: u32,
) -> Result<u64, PortfolioError> {
    owner.require_auth();

    if assets.is_empty() {
        panic_with_error!(env, PortfolioError::EmptyPortfolio);
    }

    validate_weights(env, &assets)?;

    let portfolio_id = next_portfolio_id(env);
    let now = env.ledger().timestamp();

    let portfolio = Portfolio {
        portfolio_id,
        owner: owner.clone(),
        assets,
        drift_tolerance_bps,
        rebalance_frequency_seconds,
        tx_cost_bps,
        last_rebalance: now,
        rebalance_count: 0,
        created_at: now,
        active: true,
    };

    env.storage()
        .persistent()
        .set(&PortfolioKey::Record(portfolio_id), &portfolio);

    let mut owner_portfolios: Vec<u64> = env
        .storage()
        .persistent()
        .get(&PortfolioKey::OwnerPortfolios(owner.clone()))
        .unwrap_or_else(|| Vec::new(env));
    owner_portfolios.push_back(portfolio_id);
    env.storage()
        .persistent()
        .set(&PortfolioKey::OwnerPortfolios(owner.clone()), &owner_portfolios);

    env.events().publish(
        (soroban_sdk::symbol_short!("port_new"),),
        (portfolio_id, owner, portfolio.assets.len()),
    );

    Ok(portfolio_id)
}

/// Calculate optimal adjustments to rebalance a portfolio given current values.
///
/// `current_values` must have the same length as `portfolio.assets`.
/// Returns a vector of `AssetAdjustment` (one per asset), net of transaction costs.
pub fn calculate_optimal_allocations(
    env: &Env,
    portfolio_id: u64,
    current_values: &Vec<i128>,
) -> Result<Vec<AssetAdjustment>, PortfolioError> {
    let portfolio: Portfolio = env
        .storage()
        .persistent()
        .get(&PortfolioKey::Record(portfolio_id))
        .ok_or(PortfolioError::PortfolioNotFound)?;

    let total_value: i128 = current_values.iter().fold(0i128, |acc, v| acc.saturating_add(v));

    let mut adjustments = Vec::new(env);

    for (i, asset) in portfolio.assets.iter().enumerate() {
        let current = current_values.get(i as u32).unwrap_or(0);
        let target = (total_value as u128)
            .checked_mul(asset.target_bps as u128)
            .ok_or(PortfolioError::CalculationOverflow)?
            .checked_div(10_000)
            .ok_or(PortfolioError::CalculationOverflow)? as i128;

        let delta = target - current;
        let tx_cost = (delta.abs() as u128)
            .checked_mul(portfolio.tx_cost_bps as u128)
            .ok_or(PortfolioError::CalculationOverflow)?
            .checked_div(10_000)
            .ok_or(PortfolioError::CalculationOverflow)? as i128;

        adjustments.push_back(AssetAdjustment {
            token: asset.token.clone(),
            delta,
            tx_cost,
        });
    }

    Ok(adjustments)
}

/// Determine whether a portfolio needs rebalancing given current values.
///
/// Returns `Ok(true)` when drift exceeds tolerance and frequency allows it.
pub fn needs_rebalance(
    env: &Env,
    portfolio_id: u64,
    current_values: &Vec<i128>,
) -> Result<bool, PortfolioError> {
    let portfolio: Portfolio = env
        .storage()
        .persistent()
        .get(&PortfolioKey::Record(portfolio_id))
        .ok_or(PortfolioError::PortfolioNotFound)?;

    let now = env.ledger().timestamp();
    if now < portfolio.last_rebalance.saturating_add(portfolio.rebalance_frequency_seconds) {
        return Err(PortfolioError::RebalanceTooFrequent);
    }

    let total_value: i128 = current_values.iter().fold(0i128, |acc, v| acc.saturating_add(v));
    if total_value == 0 {
        return Ok(false);
    }

    for (i, asset) in portfolio.assets.iter().enumerate() {
        let current = current_values.get(i as u32).unwrap_or(0);
        let current_bps = ((current as u128)
            .saturating_mul(10_000)
            .checked_div(total_value as u128)
            .unwrap_or(0)) as u32;

        let drift = if current_bps > asset.target_bps {
            current_bps - asset.target_bps
        } else {
            asset.target_bps - current_bps
        };

        if drift > portfolio.drift_tolerance_bps {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Execute a rebalance: records the event and updates portfolio state.
///
/// Caller must supply the computed adjustments (from `calculate_optimal_allocations`).
/// Emits `("port_rebal",)` with data `(portfolio_id, owner, total_adjusted, tx_costs_paid)`.
pub fn execute_rebalance(
    env: &Env,
    owner: Address,
    portfolio_id: u64,
    adjustments: &Vec<AssetAdjustment>,
) -> Result<RebalanceHistoryEntry, PortfolioError> {
    owner.require_auth();

    let mut portfolio: Portfolio = env
        .storage()
        .persistent()
        .get(&PortfolioKey::Record(portfolio_id))
        .ok_or(PortfolioError::PortfolioNotFound)?;

    if portfolio.owner != owner {
        panic_with_error!(env, PortfolioError::Unauthorized);
    }

    let now = env.ledger().timestamp();
    let mut total_adjusted: i128 = 0;
    let mut tx_costs_paid: i128 = 0;
    let mut assets_adjusted: u32 = 0;

    for adj in adjustments.iter() {
        if adj.delta != 0 {
            total_adjusted = total_adjusted.saturating_add(adj.delta.abs());
            tx_costs_paid = tx_costs_paid.saturating_add(adj.tx_cost);
            assets_adjusted += 1;
        }
    }

    // Record history
    let entry_index: u64 = env
        .storage()
        .persistent()
        .get(&PortfolioKey::HistoryCount(portfolio_id))
        .unwrap_or(0);

    let entry = RebalanceHistoryEntry {
        entry_index,
        portfolio_id,
        timestamp: now,
        total_adjusted,
        tx_costs_paid,
        assets_adjusted,
    };

    env.storage()
        .persistent()
        .set(&PortfolioKey::History(portfolio_id, entry_index), &entry);
    env.storage()
        .persistent()
        .set(&PortfolioKey::HistoryCount(portfolio_id), &(entry_index + 1));

    // Update portfolio state
    portfolio.last_rebalance = now;
    portfolio.rebalance_count += 1;
    env.storage()
        .persistent()
        .set(&PortfolioKey::Record(portfolio_id), &portfolio);

    env.events().publish(
        (soroban_sdk::symbol_short!("port_rbl"),),
        (portfolio_id, owner, total_adjusted, tx_costs_paid),
    );

    Ok(entry)
}

/// Get a portfolio by ID.
pub fn get_portfolio(env: &Env, portfolio_id: u64) -> Option<Portfolio> {
    env.storage()
        .persistent()
        .get(&PortfolioKey::Record(portfolio_id))
}

/// Get all portfolio IDs for an owner.
pub fn get_owner_portfolios(env: &Env, owner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&PortfolioKey::OwnerPortfolios(owner.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Get a rebalance history entry.
pub fn get_history_entry(
    env: &Env,
    portfolio_id: u64,
    entry_index: u64,
) -> Option<RebalanceHistoryEntry> {
    env.storage()
        .persistent()
        .get(&PortfolioKey::History(portfolio_id, entry_index))
}

/// Get the total number of rebalance history entries for a portfolio.
pub fn get_history_count(env: &Env, portfolio_id: u64) -> u64 {
    env.storage()
        .persistent()
        .get(&PortfolioKey::HistoryCount(portfolio_id))
        .unwrap_or(0)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn next_portfolio_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&PortfolioKey::Ctr)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&PortfolioKey::Ctr, &(id + 1));
    id
}

fn validate_weights(env: &Env, assets: &Vec<AssetTarget>) -> Result<(), PortfolioError> {
    let mut total: u32 = 0;
    for asset in assets.iter() {
        total = total
            .checked_add(asset.target_bps)
            .ok_or(PortfolioError::InvalidWeights)?;
    }
    if total != 10_000 {
        panic_with_error!(env, PortfolioError::InvalidWeights);
    }
    Ok(())
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_portfolio_error_uniqueness() {
        assert_ne!(PortfolioError::PortfolioNotFound as u32, PortfolioError::InvalidWeights as u32);
        assert_ne!(PortfolioError::RebalanceTooFrequent as u32, PortfolioError::DriftWithinTolerance as u32);
        assert_ne!(PortfolioError::Unauthorized as u32, PortfolioError::EmptyPortfolio as u32);
    }
}
