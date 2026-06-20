//! Programmable royalty system with dynamic splits and conditional distributions.
//!
//! Extends the base royalty system with:
//! - Rule definitions (condition-based splits)
//! - Dynamic splits (context-aware percentage adjustments)
//! - Conditional distributions (rule evaluation)
//! - Nested royalty chains (recursive application)
//! - Payment tracking (audit trail)

use soroban_sdk::{contracttype, symbol_short, Address, Env, Vec};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

pub const MAX_RULES: u32 = 10;
pub const MAX_CONDITIONS: u32 = 5;
pub const MAX_RECIPIENTS: u32 = 20;
pub const BPS_DENOM: i128 = 10_000;
pub const MAX_NEST_DEPTH: u32 = 5;

// ── Condition Types ──────────────────────────────────────────────────────────

/// A condition that can gate a distribution rule.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Condition {
    /// Always applies (default).
    Always,
    /// Apply if tip amount >= threshold.
    MinAmount(i128),
    /// Apply if tip amount <= threshold.
    MaxAmount(i128),
    /// Apply only to specified sender.
    FromSender(Address),
    /// Apply only from a list of senders.
    FromList(Vec<Address>),
    /// Apply if current timestamp >= start (unix seconds).
    TimeAfter(u64),
    /// Apply if current timestamp <= end (unix seconds).
    TimeBefore(u64),
    /// Apply if tipper count >= threshold.
    MinTipperCount(u32),
    /// Apply if total tips received >= threshold.
    MinTotalTips(i128),
}

/// A recipient with a dynamic (context-aware) share percentage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicRecipient {
    pub recipient: Address,
    /// Base share in basis points.
    pub base_bps: u32,
    /// Optional bonus in bps (applied on top of base when conditions met).
    pub bonus_bps: u32,
}

/// A royalty distribution rule tied to conditions.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyRule {
    /// Unique identifier for this rule.
    pub rule_id: u64,
    /// Conditions that must all be true for this rule to apply.
    pub conditions: Vec<Condition>,
    /// Recipients and their dynamic shares.
    pub recipients: Vec<DynamicRecipient>,
    /// Owner can modify or remove this rule.
    pub owner: Address,
    /// Enabled/disabled flag.
    pub enabled: bool,
    /// Priority (higher = evaluated first).
    pub priority: u32,
}

/// A royalty payment record for tracking.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyPayment {
    /// Who received the payment.
    pub recipient: Address,
    /// How much.
    pub amount: i128,
    /// When (unix seconds).
    pub timestamp: u64,
    /// Which rule was applied.
    pub rule_id: u64,
    /// Source tip amount.
    pub source_amount: i128,
}

// ── Storage Helpers ──────────────────────────────────────────────────────────

/// Get all rules for a creator, sorted by priority descending.
fn load_rules(env: &Env, creator: &Address) -> Vec<RoyaltyRule> {
    // This would ideally be stored as a single vector or map.
    // For now, we load each rule individually (up to MAX_RULES).
    let mut rules = Vec::new(env);
    for i in 0..MAX_RULES {
        if let Some(rule_id) = get_creator_rule_id(env, creator, i) {
            if let Some(rule) = load_rule(env, rule_id) {
                rules.push_back(rule);
            }
        }
    }
    // Sort by priority descending (higher priority first).
    let n = rules.len();
    for i in 0..n {
        for j in i + 1..n {
            if rules.get(i).unwrap().priority < rules.get(j).unwrap().priority {
                let tmp = rules.get(i).unwrap();
                rules.set(i, rules.get(j).unwrap());
                rules.set(j, tmp);
            }
        }
    }
    rules
}

fn load_rule(env: &Env, rule_id: u64) -> Option<RoyaltyRule> {
    env.storage()
        .persistent()
        .get(&DataKey::ProgrammableRoyaltyRule(rule_id))
}

fn save_rule(env: &Env, rule: &RoyaltyRule) {
    env.storage()
        .persistent()
        .set(&DataKey::ProgrammableRoyaltyRule(rule.rule_id), rule);
}

fn get_creator_rule_id(env: &Env, creator: &Address, index: u32) -> Option<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::ProgrammableRoyaltyCreatorRule(
            creator.clone(),
            index as u64,
        ))
}

fn register_creator_rule(env: &Env, creator: &Address, index: u32, rule_id: u64) {
    env.storage().persistent().set(
        &DataKey::ProgrammableRoyaltyCreatorRule(creator.clone(), index as u64),
        &rule_id,
    );
}

fn get_next_rule_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::ProgrammableRoyaltyCounter)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&DataKey::ProgrammableRoyaltyCounter, &(id + 1));
    id + 1
}

fn record_payment(env: &Env, creator: &Address, payment: &RoyaltyPayment) {
    let count: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::ProgrammableRoyaltyPaymentCount(creator.clone()))
        .unwrap_or(0);
    env.storage().persistent().set(
        &DataKey::ProgrammableRoyaltyPayment(creator.clone(), count),
        payment,
    );
    env.storage().persistent().set(
        &DataKey::ProgrammableRoyaltyPaymentCount(creator.clone()),
        &(count + 1),
    );
}

// ── Condition Evaluation ─────────────────────────────────────────────────────

fn evaluate_condition(
    env: &Env,
    cond: &Condition,
    tip_amount: i128,
    sender: &Address,
    creator: &Address,
) -> bool {
    match cond {
        Condition::Always => true,
        Condition::MinAmount(threshold) => tip_amount >= *threshold,
        Condition::MaxAmount(threshold) => tip_amount <= *threshold,
        Condition::FromSender(s) => s == sender,
        Condition::FromList(senders) => {
            for i in 0..senders.len() {
                if senders.get(i).unwrap() == sender.clone() {
                    return true;
                }
            }
            false
        }
        Condition::TimeAfter(start_ts) => env.ledger().timestamp() >= *start_ts,
        Condition::TimeBefore(end_ts) => env.ledger().timestamp() <= *end_ts,
        Condition::MinTipperCount(threshold) => {
            let count: u32 = env
                .storage()
                .persistent()
                .get(&DataKey::ProgrammableRoyaltyTipperCount(creator.clone()))
                .unwrap_or(0);
            count >= *threshold
        }
        Condition::MinTotalTips(threshold) => {
            let total: i128 = env
                .storage()
                .persistent()
                .get(&DataKey::ProgrammableRoyaltyTotalTips(creator.clone()))
                .unwrap_or(0);
            total >= *threshold
        }
    }
}

fn all_conditions_met(
    env: &Env,
    conditions: &Vec<Condition>,
    tip_amount: i128,
    sender: &Address,
    creator: &Address,
) -> bool {
    for i in 0..conditions.len() {
        if !evaluate_condition(
            env,
            &conditions.get(i).unwrap(),
            tip_amount,
            sender,
            creator,
        ) {
            return false;
        }
    }
    true
}

// ── Dynamic Split Calculation ────────────────────────────────────────────────

fn calculate_recipient_share(recipient: &DynamicRecipient, bonus: bool) -> u32 {
    if bonus {
        recipient.base_bps + recipient.bonus_bps
    } else {
        recipient.base_bps
    }
}

fn normalize_shares(env: &Env, recipients: &Vec<DynamicRecipient>, bonus: bool) -> Vec<u32> {
    let mut shares = Vec::new(env);
    let mut total: u64 = 0;

    for i in 0..recipients.len() {
        let share = calculate_recipient_share(&recipients.get(i).unwrap(), bonus);
        shares.push_back(share);
        total += share as u64;
    }

    // If shares don't sum to exactly 10_000, proportionally scale.
    if total == 0 {
        return shares;
    }

    for i in 0..shares.len() {
        let scaled = (shares.get(i).unwrap() as u64 * 10_000 / total) as u32;
        shares.set(i, scaled);
    }

    shares
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Create a new royalty rule for a creator.
pub fn create_rule(
    env: &Env,
    creator: &Address,
    owner: &Address,
    conditions: Vec<Condition>,
    recipients: Vec<DynamicRecipient>,
    priority: u32,
) -> u64 {
    owner.require_auth();
    assert!(!conditions.is_empty(), "at least one condition required");
    assert!(!recipients.is_empty(), "at least one recipient required");
    assert!(recipients.len() <= MAX_RECIPIENTS, "too many recipients");

    let rule_id = get_next_rule_id(env);
    let rule = RoyaltyRule {
        rule_id,
        conditions,
        recipients,
        owner: owner.clone(),
        enabled: true,
        priority,
    };

    save_rule(env, &rule);

    // Find next available slot for this creator.
    for i in 0..MAX_RULES {
        if get_creator_rule_id(env, creator, i).is_none() {
            register_creator_rule(env, creator, i, rule_id);
            break;
        }
    }

    env.events()
        .publish((symbol_short!("prog_rul"), creator.clone()), rule_id);
    rule_id
}

/// Update an existing rule (owner only).
pub fn update_rule(
    env: &Env,
    creator: &Address,
    rule_id: u64,
    owner: &Address,
    conditions: Vec<Condition>,
    recipients: Vec<DynamicRecipient>,
    priority: u32,
) {
    owner.require_auth();
    let mut rule = load_rule(env, rule_id).expect("rule not found");
    assert_eq!(rule.owner, *owner, "not rule owner");

    rule.conditions = conditions;
    rule.recipients = recipients;
    rule.priority = priority;

    save_rule(env, &rule);
    env.events()
        .publish((symbol_short!("prog_upd"), creator.clone()), rule_id);
}

/// Enable or disable a rule.
pub fn toggle_rule(env: &Env, rule_id: u64, owner: &Address, enabled: bool) {
    owner.require_auth();
    let mut rule = load_rule(env, rule_id).expect("rule not found");
    assert_eq!(rule.owner, *owner, "not rule owner");

    rule.enabled = enabled;
    save_rule(env, &rule);
}

/// Evaluate and apply all applicable rules to distribute royalties.
///
/// Returns the remaining amount after all distributions.
/// Nested rules (where a recipient is also a creator with rules) are resolved recursively.
pub fn distribute_programmable_royalties(
    env: &Env,
    creator: &Address,
    tip_amount: i128,
    sender: &Address,
) -> i128 {
    distribute_programmable_inner(env, creator, tip_amount, sender, 0)
}

fn distribute_programmable_inner(
    env: &Env,
    creator: &Address,
    tip_amount: i128,
    sender: &Address,
    depth: u32,
) -> i128 {
    if tip_amount <= 0 || depth >= MAX_NEST_DEPTH {
        return tip_amount;
    }

    // Update creator stats.
    let is_new_tipper: bool = {
        let prev: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::ProgrammableRoyaltyTotalTips(creator.clone()))
            .unwrap_or(0);
        prev == 0
    };

    if is_new_tipper {
        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::ProgrammableRoyaltyTipperCount(creator.clone()))
            .unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::ProgrammableRoyaltyTipperCount(creator.clone()),
            &(count + 1),
        );
    }

    let total: i128 = env
        .storage()
        .persistent()
        .get(&DataKey::ProgrammableRoyaltyTotalTips(creator.clone()))
        .unwrap_or(0);
    env.storage().persistent().set(
        &DataKey::ProgrammableRoyaltyTotalTips(creator.clone()),
        &(total + tip_amount),
    );

    // Load and sort rules by priority.
    let rules = load_rules(env, creator);
    let mut distributed: i128 = 0;

    for rule_idx in 0..rules.len() {
        let rule = rules.get(rule_idx).unwrap();
        if !rule.enabled {
            continue;
        }

        // Check if all conditions are met.
        if !all_conditions_met(env, &rule.conditions, tip_amount, sender, creator) {
            continue;
        }

        // Calculate shares (with bonus if applicable).
        let shares = normalize_shares(env, &rule.recipients, true);
        let remaining_for_rule = tip_amount - distributed;

        for recip_idx in 0..rule.recipients.len() {
            let recipient = rule.recipients.get(recip_idx).unwrap();
            let share_bps = shares.get(recip_idx).unwrap();
            let amount_to_distribute = (remaining_for_rule * (share_bps as i128)) / BPS_DENOM;

            if amount_to_distribute <= 0 {
                continue;
            }

            // Record the payment.
            record_payment(
                env,
                creator,
                &RoyaltyPayment {
                    recipient: recipient.recipient.clone(),
                    amount: amount_to_distribute,
                    timestamp: env.ledger().timestamp(),
                    rule_id: rule.rule_id,
                    source_amount: tip_amount,
                },
            );

            // Nested: if recipient is also a creator with rules, recurse.
            if depth < MAX_NEST_DEPTH {
                let nested_remaining = distribute_programmable_inner(
                    env,
                    &recipient.recipient,
                    amount_to_distribute,
                    sender,
                    depth + 1,
                );
                // Only the distributed portion is counted.
                distributed += amount_to_distribute - nested_remaining;
            } else {
                // Leaf: credit the balance.
                let key = DataKey::ProgrammableRoyaltyBalance(recipient.recipient.clone());
                let bal: i128 = env.storage().persistent().get(&key).unwrap_or(0);
                env.storage()
                    .persistent()
                    .set(&key, &(bal + amount_to_distribute));
                distributed += amount_to_distribute;
            }
        }

        // Only apply the first matching rule (or multiple if chaining is desired).
        break;
    }

    if depth == 0 {
        env.events()
            .publish((symbol_short!("prog_dst"), creator.clone()), distributed);
    }

    tip_amount - distributed
}

/// Withdraw accumulated programmable royalties.
pub fn withdraw_programmable_royalties(
    env: &Env,
    recipient: &Address,
    _token_addr: &Address,
) -> i128 {
    recipient.require_auth();

    let key = DataKey::ProgrammableRoyaltyBalance(recipient.clone());
    let amount: i128 = env.storage().persistent().get(&key).unwrap_or(0);

    if amount > 0 {
        env.storage().persistent().remove(&key);
        env.events()
            .publish((symbol_short!("prog_wdw"), recipient.clone()), amount);
    }

    amount
}

/// Get accumulated balance for a recipient.
pub fn get_programmable_balance(env: &Env, recipient: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::ProgrammableRoyaltyBalance(recipient.clone()))
        .unwrap_or(0)
}

/// Get a rule by ID.
pub fn get_rule(env: &Env, rule_id: u64) -> Option<RoyaltyRule> {
    load_rule(env, rule_id)
}

/// Get all rules for a creator.
pub fn get_creator_rules(env: &Env, creator: &Address) -> Vec<RoyaltyRule> {
    load_rules(env, creator)
}

/// Get a payment record by creator and index.
pub fn get_payment(env: &Env, creator: &Address, index: u64) -> Option<RoyaltyPayment> {
    env.storage()
        .persistent()
        .get(&DataKey::ProgrammableRoyaltyPayment(creator.clone(), index))
}

/// Get total payment count for a creator.
pub fn get_payment_count(env: &Env, creator: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::ProgrammableRoyaltyPaymentCount(creator.clone()))
        .unwrap_or(0)
}

/// Get total tips received by a creator (for condition evaluation).
pub fn get_total_tips_received(env: &Env, creator: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::ProgrammableRoyaltyTotalTips(creator.clone()))
        .unwrap_or(0)
}

/// Get tipper count for a creator.
pub fn get_tipper_count(env: &Env, creator: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::ProgrammableRoyaltyTipperCount(creator.clone()))
        .unwrap_or(0)
}
