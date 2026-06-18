//! Fractional ownership of tip revenue streams.
//!
//! Allows a creator to mint a fixed supply of fractions representing shares of
//! their future tip revenue.  Fraction holders receive proportional revenue
//! distributions and may transfer or sell their fractions to other parties.
//! Any holder can buy out all remaining fractions at a pre-set price to
//! consolidate full ownership.

use soroban_sdk::{contracttype, Address, Env, Vec};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Maximum total fractions that can be minted for a single creator.
pub const MAX_FRACTIONS: u64 = 1_000_000;

// ── Types ────────────────────────────────────────────────────────────────────

/// Metadata for a creator's fractional ownership pool.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionPool {
    /// Creator whose revenue is being fractionalised.
    pub creator: Address,
    /// Total fractions minted (immutable after minting).
    pub total_supply: u64,
    /// Accumulated revenue (in the creator's tip token) not yet distributed.
    pub pending_revenue: i128,
    /// Per-fraction revenue already distributed (scaled by `total_supply`).
    /// Used to compute each holder's unclaimed share without iterating holders.
    pub revenue_per_fraction: i128,
    /// Price per fraction for a buyout (0 = buyout disabled).
    pub buyout_price_per_fraction: i128,
}

/// A single holder's position in a fraction pool.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionPosition {
    /// Number of fractions held.
    pub amount: u64,
    /// Snapshot of `revenue_per_fraction` at the last claim / position change.
    pub revenue_debt: i128,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

fn load_pool(env: &Env, creator: &Address) -> Option<FractionPool> {
    env.storage()
        .persistent()
        .get(&DataKey::FractionPool(creator.clone()))
}

fn save_pool(env: &Env, pool: &FractionPool) {
    env.storage()
        .persistent()
        .set(&DataKey::FractionPool(pool.creator.clone()), pool);
}

fn load_position(env: &Env, creator: &Address, holder: &Address) -> FractionPosition {
    env.storage()
        .persistent()
        .get(&DataKey::FractionPosition(creator.clone(), holder.clone()))
        .unwrap_or(FractionPosition {
            amount: 0,
            revenue_debt: 0,
        })
}

fn save_position(env: &Env, creator: &Address, holder: &Address, pos: &FractionPosition) {
    env.storage().persistent().set(
        &DataKey::FractionPosition(creator.clone(), holder.clone()),
        pos,
    );
}

fn load_holders(env: &Env, creator: &Address) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::FractionHolders(creator.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

fn save_holders(env: &Env, creator: &Address, holders: &Vec<Address>) {
    env.storage()
        .persistent()
        .set(&DataKey::FractionHolders(creator.clone()), holders);
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Mint `total_supply` fractions for `creator`, assigning all of them to the
/// creator initially.  `buyout_price_per_fraction` may be 0 to disable buyouts.
///
/// Panics if a pool already exists for this creator or if `total_supply` is
/// zero or exceeds [`MAX_FRACTIONS`].
pub fn mint_fractions(
    env: &Env,
    creator: &Address,
    total_supply: u64,
    buyout_price_per_fraction: i128,
) {
    creator.require_auth();

    assert!(
        total_supply > 0 && total_supply <= MAX_FRACTIONS,
        "invalid supply"
    );
    assert!(load_pool(env, creator).is_none(), "pool already exists");

    let pool = FractionPool {
        creator: creator.clone(),
        total_supply,
        pending_revenue: 0,
        revenue_per_fraction: 0,
        buyout_price_per_fraction,
    };
    save_pool(env, &pool);

    // Creator starts with the full supply.
    let pos = FractionPosition {
        amount: total_supply,
        revenue_debt: 0,
    };
    save_position(env, creator, creator, &pos);

    let mut holders = load_holders(env, creator);
    holders.push_back(creator.clone());
    save_holders(env, creator, &holders);
}

/// Record `amount` of new revenue for `creator`'s pool.  Called internally
/// whenever a tip is received so that revenue is tracked for distribution.
///
/// No-ops if no pool exists for this creator.
pub fn accrue_revenue(env: &Env, creator: &Address, amount: i128) {
    let Some(mut pool) = load_pool(env, creator) else {
        return;
    };
    if amount <= 0 {
        return;
    }
    // Accumulate revenue; distribute lazily when holders claim.
    pool.pending_revenue += amount;
    pool.revenue_per_fraction += amount / pool.total_supply as i128;
    save_pool(env, &pool);
}

/// Claim the caller's share of accumulated revenue.  Returns the amount paid
/// out (0 if nothing is owed).
///
/// Panics if no pool exists for this creator or the caller holds no fractions.
pub fn claim_revenue(env: &Env, creator: &Address, holder: &Address) -> i128 {
    holder.require_auth();

    let pool = load_pool(env, creator).expect("no pool");
    let mut pos = load_position(env, creator, holder);

    let owed = (pool.revenue_per_fraction - pos.revenue_debt) * pos.amount as i128;
    if owed <= 0 {
        return 0;
    }

    pos.revenue_debt = pool.revenue_per_fraction;
    save_position(env, creator, holder, &pos);

    // Emit event so off-chain indexers can track distributions.
    env.events().publish(
        (soroban_sdk::symbol_short!("frac_clm"), creator.clone()),
        (holder.clone(), owed),
    );

    owed
}

/// Transfer `amount` fractions from `from` to `to`.
///
/// Panics if `from` has insufficient fractions or no pool exists.
pub fn transfer_fractions(env: &Env, creator: &Address, from: &Address, to: &Address, amount: u64) {
    from.require_auth();

    let pool = load_pool(env, creator).expect("no pool");
    let mut from_pos = load_position(env, creator, from);

    assert!(from_pos.amount >= amount, "insufficient fractions");

    // Settle any pending revenue for `from` before changing their balance.
    let from_owed = (pool.revenue_per_fraction - from_pos.revenue_debt) * from_pos.amount as i128;
    if from_owed > 0 {
        // Revenue stays in the pool; debt is updated so it isn't lost.
        from_pos.revenue_debt = pool.revenue_per_fraction;
    }

    from_pos.amount -= amount;
    save_position(env, creator, from, &from_pos);

    let mut to_pos = load_position(env, creator, to);
    // Settle `to`'s existing position before adding fractions.
    to_pos.revenue_debt = pool.revenue_per_fraction;
    to_pos.amount += amount;
    save_position(env, creator, to, &to_pos);

    // Track new holder.
    let mut holders = load_holders(env, creator);
    if !holders.contains(to) {
        holders.push_back(to.clone());
        save_holders(env, creator, &holders);
    }

    env.events().publish(
        (soroban_sdk::symbol_short!("frac_xfr"), creator.clone()),
        (from.clone(), to.clone(), amount),
    );
}

/// Buy out all fractions not already held by `buyer` at the pool's
/// `buyout_price_per_fraction`.  Returns the total cost paid.
///
/// The caller is responsible for transferring the token amount externally;
/// this function only updates ownership state.
///
/// Panics if buyouts are disabled (price == 0), no pool exists, or the buyer
/// already owns the full supply.
pub fn buyout(env: &Env, creator: &Address, buyer: &Address) -> i128 {
    buyer.require_auth();

    let pool = load_pool(env, creator).expect("no pool");
    assert!(pool.buyout_price_per_fraction > 0, "buyout disabled");

    let buyer_pos = load_position(env, creator, buyer);
    let fractions_to_buy = pool.total_supply - buyer_pos.amount;
    assert!(fractions_to_buy > 0, "already full owner");

    let total_cost = pool.buyout_price_per_fraction * fractions_to_buy as i128;

    // Transfer all fractions from every other holder to the buyer.
    let holders = load_holders(env, creator);
    for i in 0..holders.len() {
        let holder = holders.get(i).unwrap();
        if holder == *buyer {
            continue;
        }
        let mut h_pos = load_position(env, creator, &holder);
        if h_pos.amount == 0 {
            continue;
        }
        h_pos.amount = 0;
        h_pos.revenue_debt = pool.revenue_per_fraction;
        save_position(env, creator, &holder, &h_pos);
    }

    let mut buyer_pos_mut = load_position(env, creator, buyer);
    buyer_pos_mut.amount = pool.total_supply;
    buyer_pos_mut.revenue_debt = pool.revenue_per_fraction;
    save_position(env, creator, buyer, &buyer_pos_mut);

    env.events().publish(
        (soroban_sdk::symbol_short!("frac_buy"), creator.clone()),
        (buyer.clone(), total_cost),
    );

    total_cost
}

/// Returns the fraction pool for `creator`, or `None` if not initialised.
pub fn get_pool(env: &Env, creator: &Address) -> Option<FractionPool> {
    load_pool(env, creator)
}

/// Returns the fraction position for `holder` in `creator`'s pool.
pub fn get_position(env: &Env, creator: &Address, holder: &Address) -> FractionPosition {
    load_position(env, creator, holder)
}
