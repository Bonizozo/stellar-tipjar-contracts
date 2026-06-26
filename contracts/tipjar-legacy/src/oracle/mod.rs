//! Decentralized Oracle Network
//!
//! Provides external data feeds and price information via a network of oracle
//! nodes with aggregation, dispute resolution, provider rewards, and reputation
//! tracking.

use soroban_sdk::{contracttype, symbol_short, Address, Env, Vec};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A registered oracle node provider.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleNode {
    pub provider: Address,
    /// Reputation score (0–10_000 basis points).
    pub reputation: u32,
    /// Total submissions made.
    pub submissions: u64,
    /// Successful (non-disputed) submissions.
    pub accepted: u64,
    /// Staked collateral for dispute slashing.
    pub stake: i128,
    pub active: bool,
}

/// A single price submission from an oracle node.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceSubmission {
    pub provider: Address,
    /// Price scaled by 1_000_000.
    pub price: i128,
    pub timestamp: u64,
}

/// Aggregated price feed for an asset pair.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceFeed {
    /// e.g. b"XLM/USD"
    pub pair: soroban_sdk::Bytes,
    /// Median of accepted submissions, scaled by 1_000_000.
    pub price: i128,
    pub last_updated: u64,
    pub submission_count: u32,
}

/// A dispute raised against a price submission.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleDispute {
    pub dispute_id: u64,
    pub challenger: Address,
    pub provider: Address,
    pub pair: soroban_sdk::Bytes,
    pub disputed_price: i128,
    pub reference_price: i128,
    pub resolved: bool,
    pub challenger_won: bool,
    pub created_at: u64,
}

// ---------------------------------------------------------------------------
// Storage helpers
// ---------------------------------------------------------------------------

fn node_key(provider: &Address) -> (soroban_sdk::Symbol, Address) {
    (symbol_short!("onode"), provider.clone())
}

fn feed_key(pair: &soroban_sdk::Bytes) -> (soroban_sdk::Symbol, soroban_sdk::Bytes) {
    (symbol_short!("feed"), pair.clone())
}

fn dispute_count_key() -> soroban_sdk::Symbol {
    symbol_short!("d_count")
}

fn dispute_key(id: u64) -> (soroban_sdk::Symbol, u64) {
    (symbol_short!("dispute"), id)
}

fn pending_key(pair: &soroban_sdk::Bytes) -> (soroban_sdk::Symbol, soroban_sdk::Bytes) {
    (symbol_short!("pending"), pair.clone())
}

// ---------------------------------------------------------------------------
// Node registration
// ---------------------------------------------------------------------------

/// Register a new oracle node with an initial stake.
pub fn register_node(env: &Env, provider: &Address, stake: i128) -> OracleNode {
    provider.require_auth();
    assert!(stake > 0, "stake must be positive");

    let node = OracleNode {
        provider: provider.clone(),
        reputation: 5_000, // start at 50%
        submissions: 0,
        accepted: 0,
        stake,
        active: true,
    };

    env.storage()
        .persistent()
        .set(&node_key(provider), &node);

    env.events().publish(
        (symbol_short!("oracle"), symbol_short!("reg")),
        provider.clone(),
    );

    node
}

/// Get a registered oracle node.
pub fn get_node(env: &Env, provider: &Address) -> Option<OracleNode> {
    env.storage().persistent().get(&node_key(provider))
}

// ---------------------------------------------------------------------------
// Price submission & aggregation
// ---------------------------------------------------------------------------

/// Submit a price for an asset pair.
///
/// Prices are buffered as pending submissions; call `aggregate_feed` to
/// compute the median and publish the canonical feed.
pub fn submit_price(
    env: &Env,
    provider: &Address,
    pair: soroban_sdk::Bytes,
    price: i128,
) {
    provider.require_auth();
    assert!(price > 0, "price must be positive");

    let mut node: OracleNode = env
        .storage()
        .persistent()
        .get(&node_key(provider))
        .expect("provider not registered");
    assert!(node.active, "node is inactive");

    node.submissions += 1;
    env.storage().persistent().set(&node_key(provider), &node);

    let sub = PriceSubmission {
        provider: provider.clone(),
        price,
        timestamp: env.ledger().timestamp(),
    };

    // Append to pending list for this pair.
    let pk = pending_key(&pair);
    let mut pending: Vec<PriceSubmission> = env
        .storage()
        .temporary()
        .get(&pk)
        .unwrap_or(Vec::new(env));
    pending.push_back(sub);
    env.storage().temporary().set(&pk, &pending);

    env.events().publish(
        (symbol_short!("oracle"), symbol_short!("sub")),
        (provider.clone(), price),
    );
}

/// Aggregate pending submissions into a canonical price feed using the median.
///
/// Requires at least 3 submissions. Clears the pending buffer afterwards.
pub fn aggregate_feed(env: &Env, pair: soroban_sdk::Bytes) -> PriceFeed {
    let pk = pending_key(&pair);
    let pending: Vec<PriceSubmission> = env
        .storage()
        .temporary()
        .get(&pk)
        .unwrap_or(Vec::new(env));

    assert!(pending.len() >= 3, "need at least 3 submissions");

    // Collect prices and compute median.
    let mut prices: Vec<i128> = Vec::new(env);
    for sub in pending.iter() {
        prices.push_back(sub.price);
    }
    // Simple insertion sort for small N.
    let n = prices.len() as usize;
    for i in 1..n {
        let key_val = prices.get(i as u32).unwrap();
        let mut j = i;
        while j > 0 && prices.get((j - 1) as u32).unwrap() > key_val {
            let prev = prices.get((j - 1) as u32).unwrap();
            prices.set(j as u32, prev);
            j -= 1;
        }
        prices.set(j as u32, key_val);
    }
    let median = prices.get((n / 2) as u32).unwrap();

    let feed = PriceFeed {
        pair: pair.clone(),
        price: median,
        last_updated: env.ledger().timestamp(),
        submission_count: pending.len(),
    };

    env.storage().persistent().set(&feed_key(&pair), &feed);
    env.storage().temporary().remove(&pk);

    env.events().publish(
        (symbol_short!("oracle"), symbol_short!("agg")),
        median,
    );

    feed
}

/// Get the latest aggregated price feed for a pair.
pub fn get_price(env: &Env, pair: soroban_sdk::Bytes) -> Option<PriceFeed> {
    env.storage().persistent().get(&feed_key(&pair))
}

// ---------------------------------------------------------------------------
// Disputes
// ---------------------------------------------------------------------------

/// Raise a dispute against a provider's submission.
///
/// `reference_price` is the challenger's claimed correct price.
/// Dispute is auto-resolved: if |disputed - reference| > 5% of reference,
/// the challenger wins and the provider's reputation is slashed.
pub fn raise_dispute(
    env: &Env,
    challenger: &Address,
    provider: &Address,
    pair: soroban_sdk::Bytes,
    disputed_price: i128,
    reference_price: i128,
) -> OracleDispute {
    challenger.require_auth();
    assert!(reference_price > 0, "invalid reference price");

    let id: u64 = env
        .storage()
        .persistent()
        .get(&dispute_count_key())
        .unwrap_or(0u64)
        + 1;
    env.storage().persistent().set(&dispute_count_key(), &id);

    // Auto-resolve: deviation > 5% → challenger wins.
    let deviation = (disputed_price - reference_price).abs();
    let threshold = reference_price / 20; // 5%
    let challenger_won = deviation > threshold;

    if challenger_won {
        if let Some(mut node) = get_node(env, provider) {
            node.reputation = node.reputation.saturating_sub(500); // -5%
            env.storage().persistent().set(&node_key(provider), &node);
        }
    } else {
        // Reward provider reputation for surviving dispute.
        if let Some(mut node) = get_node(env, provider) {
            node.accepted += 1;
            node.reputation = (node.reputation + 100).min(10_000);
            env.storage().persistent().set(&node_key(provider), &node);
        }
    }

    let dispute = OracleDispute {
        dispute_id: id,
        challenger: challenger.clone(),
        provider: provider.clone(),
        pair,
        disputed_price,
        reference_price,
        resolved: true,
        challenger_won,
        created_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&dispute_key(id), &dispute);

    env.events().publish(
        (symbol_short!("oracle"), symbol_short!("disp")),
        (id, challenger_won),
    );

    dispute
}

/// Get a dispute by ID.
pub fn get_dispute(env: &Env, dispute_id: u64) -> Option<OracleDispute> {
    env.storage().persistent().get(&dispute_key(dispute_id))
}

// ---------------------------------------------------------------------------
// Rewards
// ---------------------------------------------------------------------------

/// Reward an oracle provider for a successful submission round.
///
/// Increases reputation and records the accepted submission.
pub fn reward_provider(env: &Env, provider: &Address) {
    let mut node: OracleNode = env
        .storage()
        .persistent()
        .get(&node_key(provider))
        .expect("provider not registered");

    node.accepted += 1;
    node.reputation = (node.reputation + 50).min(10_000);
    env.storage().persistent().set(&node_key(provider), &node);

    env.events().publish(
        (symbol_short!("oracle"), symbol_short!("rwrd")),
        provider.clone(),
    );
}
