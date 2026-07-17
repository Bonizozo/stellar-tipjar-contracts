//! Quadratic Funding
//!
//! Implements CLR (Constrained Liberal Radicalism) quadratic funding:
//! - Contributions are square-root weighted so many small donors amplify matching more than one large donor.
//! - A matching pool is distributed proportionally to (Σ√contribution_i)² per project.
//! - Sybil resistance: one address = one contributor per project per round.

use soroban_sdk::{contracttype, panic_with_error, symbol_short, token, Address, Env, Vec};

// ── Storage keys ─────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QFKey {
    /// Round record keyed by round_id.
    Round(u64),
    /// Global round counter.
    RoundCtr,
    /// Contribution amount keyed by (round_id, project, contributor).
    Contribution(u64, Address, Address),
    /// Aggregated sqrt-sum for a project in a round: (round_id, project).
    ProjectSqrtSum(u64, Address),
    /// List of unique contributors for a project in a round: (round_id, project).
    ProjectContributors(u64, Address),
    /// List of projects registered in a round.
    RoundProjects(u64),
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// Status of a funding round.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RoundStatus {
    Active,
    Finalized,
    Distributed,
}

/// A quadratic funding round.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundingRound {
    pub round_id: u64,
    pub admin: Address,
    pub token: Address,
    pub matching_pool: i128,
    pub start_time: u64,
    pub end_time: u64,
    pub status: RoundStatus,
    pub total_contributions: i128,
}

/// A single contribution record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Contribution {
    pub contributor: Address,
    pub project: Address,
    pub amount: i128,
    pub round_id: u64,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum QFError {
    RoundNotFound = 200,
    RoundNotActive = 201,
    RoundNotFinalized = 202,
    AlreadyDistributed = 203,
    RoundEnded = 204,
    RoundNotEnded = 205,
    AlreadyContributed = 206,
    InvalidAmount = 207,
    Unauthorized = 208,
    ProjectNotRegistered = 209,
}

// ── Integer square root (no floating point) ───────────────────────────────────

/// Returns floor(√n) using Newton's method with i128 arithmetic.
/// Scaled: input and output are in units of 1e7 (PRECISION).
pub const PRECISION: i128 = 10_000_000; // 1e7

/// Computes floor(√(amount * PRECISION²)) so the result is in PRECISION units.
/// This lets us accumulate sqrt-sums as fixed-point integers.
pub fn isqrt_scaled(amount: i128) -> i128 {
    if amount <= 0 {
        return 0;
    }
    // We want floor(sqrt(amount) * PRECISION).
    // Equivalent to floor(sqrt(amount * PRECISION^2)).
    let n = amount.saturating_mul(PRECISION).saturating_mul(PRECISION);
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

// ── Core functions ────────────────────────────────────────────────────────────

/// Creates a new funding round. Admin deposits the matching pool immediately.
pub fn create_round(
    env: &Env,
    admin: &Address,
    token: &Address,
    matching_pool: i128,
    duration_seconds: u64,
) -> u64 {
    if matching_pool <= 0 || duration_seconds == 0 {
        panic_with_error!(env, QFError::InvalidAmount);
    }

    let round_id: u64 = env.storage().instance().get(&QFKey::RoundCtr).unwrap_or(0);
    env.storage()
        .instance()
        .set(&QFKey::RoundCtr, &(round_id + 1));

    let now = env.ledger().timestamp();
    let round = FundingRound {
        round_id,
        admin: admin.clone(),
        token: token.clone(),
        matching_pool,
        start_time: now,
        end_time: now.saturating_add(duration_seconds),
        status: RoundStatus::Active,
        total_contributions: 0,
    };

    // Transfer matching pool from admin into contract escrow.
    token::Client::new(env, token).transfer(admin, &env.current_contract_address(), &matching_pool);

    env.storage()
        .persistent()
        .set(&QFKey::Round(round_id), &round);

    env.events().publish(
        (symbol_short!("qf_new"),),
        (round_id, admin.clone(), token.clone(), matching_pool),
    );

    round_id
}

/// Records a contribution from `contributor` to `project` in `round_id`.
/// Each address may contribute only once per project per round (Sybil resistance).
pub fn contribute(
    env: &Env,
    contributor: &Address,
    round_id: u64,
    project: &Address,
    amount: i128,
) {
    if amount <= 0 {
        panic_with_error!(env, QFError::InvalidAmount);
    }

    let mut round: FundingRound = env
        .storage()
        .persistent()
        .get(&QFKey::Round(round_id))
        .unwrap_or_else(|| panic_with_error!(env, QFError::RoundNotFound));

    if round.status != RoundStatus::Active {
        panic_with_error!(env, QFError::RoundNotActive);
    }
    if env.ledger().timestamp() > round.end_time {
        panic_with_error!(env, QFError::RoundEnded);
    }

    // Sybil check: one contribution per (contributor, project, round).
    let contrib_key = QFKey::Contribution(round_id, project.clone(), contributor.clone());
    if env.storage().persistent().has(&contrib_key) {
        panic_with_error!(env, QFError::AlreadyContributed);
    }

    // Register project in round if new.
    let projects_key = QFKey::RoundProjects(round_id);
    let mut projects: Vec<Address> = env
        .storage()
        .persistent()
        .get(&projects_key)
        .unwrap_or_else(|| Vec::new(env));
    if !projects.contains(project) {
        projects.push_back(project.clone());
        env.storage().persistent().set(&projects_key, &projects);
    }

    // Update contributor list for this project.
    let contribs_key = QFKey::ProjectContributors(round_id, project.clone());
    let mut contributors: Vec<Address> = env
        .storage()
        .persistent()
        .get(&contribs_key)
        .unwrap_or_else(|| Vec::new(env));
    contributors.push_back(contributor.clone());
    env.storage().persistent().set(&contribs_key, &contributors);

    // Accumulate sqrt-sum for the project.
    let sqrt_key = QFKey::ProjectSqrtSum(round_id, project.clone());
    let current_sqrt: i128 = env.storage().persistent().get(&sqrt_key).unwrap_or(0);
    let new_sqrt = current_sqrt.saturating_add(isqrt_scaled(amount));
    env.storage().persistent().set(&sqrt_key, &new_sqrt);

    // Store individual contribution.
    env.storage().persistent().set(
        &contrib_key,
        &Contribution {
            contributor: contributor.clone(),
            project: project.clone(),
            amount,
            round_id,
        },
    );

    // Update round totals.
    round.total_contributions = round.total_contributions.saturating_add(amount);
    env.storage()
        .persistent()
        .set(&QFKey::Round(round_id), &round);

    // Transfer contribution into contract escrow.
    token::Client::new(env, &round.token).transfer(
        contributor,
        &env.current_contract_address(),
        &amount,
    );

    env.events().publish(
        (symbol_short!("qf_con"),),
        (round_id, contributor.clone(), project.clone(), amount),
    );
}

/// Finalizes a round after its end_time. Admin only.
pub fn finalize_round(env: &Env, admin: &Address, round_id: u64) {
    let mut round: FundingRound = env
        .storage()
        .persistent()
        .get(&QFKey::Round(round_id))
        .unwrap_or_else(|| panic_with_error!(env, QFError::RoundNotFound));

    if round.admin != *admin {
        panic_with_error!(env, QFError::Unauthorized);
    }
    if round.status != RoundStatus::Active {
        panic_with_error!(env, QFError::RoundNotActive);
    }
    if env.ledger().timestamp() < round.end_time {
        panic_with_error!(env, QFError::RoundNotEnded);
    }

    round.status = RoundStatus::Finalized;
    env.storage()
        .persistent()
        .set(&QFKey::Round(round_id), &round);

    env.events()
        .publish((symbol_short!("qf_fin"),), (round_id,));
}

/// Distributes matching funds to projects using the quadratic formula.
/// Contributions are returned to contributors; matching goes to projects.
/// Admin only; round must be Finalized.
pub fn distribute_matching(env: &Env, admin: &Address, round_id: u64) {
    let mut round: FundingRound = env
        .storage()
        .persistent()
        .get(&QFKey::Round(round_id))
        .unwrap_or_else(|| panic_with_error!(env, QFError::RoundNotFound));

    if round.admin != *admin {
        panic_with_error!(env, QFError::Unauthorized);
    }
    if round.status != RoundStatus::Finalized {
        panic_with_error!(env, QFError::RoundNotFinalized);
    }

    let projects: Vec<Address> = env
        .storage()
        .persistent()
        .get(&QFKey::RoundProjects(round_id))
        .unwrap_or_else(|| Vec::new(env));

    let token_client = token::Client::new(env, &round.token);
    let contract = env.current_contract_address();

    // Compute total_weight = Σ (sqrt_sum_i)²
    let mut total_weight: i128 = 0;
    for project in projects.iter() {
        let sqrt_sum: i128 = env
            .storage()
            .persistent()
            .get(&QFKey::ProjectSqrtSum(round_id, project.clone()))
            .unwrap_or(0);
        // (sqrt_sum)² / PRECISION² to keep units manageable
        let weight = (sqrt_sum / PRECISION).saturating_mul(sqrt_sum / PRECISION);
        total_weight = total_weight.saturating_add(weight);
    }

    // Distribute matching pool proportionally; return contributions to contributors.
    let matching_pool = round.matching_pool;
    let mut distributed: i128 = 0;
    let project_count = projects.len();

    for (i, project) in projects.iter().enumerate() {
        let sqrt_sum: i128 = env
            .storage()
            .persistent()
            .get(&QFKey::ProjectSqrtSum(round_id, project.clone()))
            .unwrap_or(0);
        let weight = (sqrt_sum / PRECISION).saturating_mul(sqrt_sum / PRECISION);

        // Last project absorbs rounding remainder.
        let match_amount = if i == (project_count - 1) as usize {
            matching_pool - distributed
        } else if total_weight > 0 {
            (matching_pool * weight) / total_weight
        } else {
            0
        };

        if match_amount > 0 {
            token_client.transfer(&contract, &project, &match_amount);
            distributed += match_amount;
        }

        // Return contributions to each contributor.
        let contributors: Vec<Address> = env
            .storage()
            .persistent()
            .get(&QFKey::ProjectContributors(round_id, project.clone()))
            .unwrap_or_else(|| Vec::new(env));

        for contributor in contributors.iter() {
            let contrib_key = QFKey::Contribution(round_id, project.clone(), contributor.clone());
            if let Some(c) = env
                .storage()
                .persistent()
                .get::<QFKey, Contribution>(&contrib_key)
            {
                token_client.transfer(&contract, &contributor, &c.amount);
            }
        }

        env.events().publish(
            (symbol_short!("qf_dist"),),
            (round_id, project.clone(), match_amount),
        );
    }

    round.status = RoundStatus::Distributed;
    env.storage()
        .persistent()
        .set(&QFKey::Round(round_id), &round);

    env.events()
        .publish((symbol_short!("qf_done"),), (round_id, distributed));
}

// ── Queries ───────────────────────────────────────────────────────────────────

pub fn get_round(env: &Env, round_id: u64) -> Option<FundingRound> {
    env.storage().persistent().get(&QFKey::Round(round_id))
}

pub fn get_contribution(
    env: &Env,
    round_id: u64,
    project: &Address,
    contributor: &Address,
) -> Option<Contribution> {
    env.storage().persistent().get(&QFKey::Contribution(
        round_id,
        project.clone(),
        contributor.clone(),
    ))
}

/// Returns the quadratic match estimate for a project in an active round.
/// Result is in token units.
pub fn get_match_estimate(env: &Env, round_id: u64, project: &Address) -> i128 {
    let round: FundingRound = match env.storage().persistent().get(&QFKey::Round(round_id)) {
        Some(r) => r,
        None => return 0,
    };

    let projects: Vec<Address> = env
        .storage()
        .persistent()
        .get(&QFKey::RoundProjects(round_id))
        .unwrap_or_else(|| Vec::new(env));

    let mut total_weight: i128 = 0;
    let mut project_weight: i128 = 0;

    for p in projects.iter() {
        let sqrt_sum: i128 = env
            .storage()
            .persistent()
            .get(&QFKey::ProjectSqrtSum(round_id, p.clone()))
            .unwrap_or(0);
        let w = (sqrt_sum / PRECISION).saturating_mul(sqrt_sum / PRECISION);
        total_weight = total_weight.saturating_add(w);
        if p == *project {
            project_weight = w;
        }
    }

    if total_weight == 0 {
        return 0;
    }
    (round.matching_pool * project_weight) / total_weight
}
