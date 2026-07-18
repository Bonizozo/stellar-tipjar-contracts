//! Conditional escrow system for tips.
//!
//! Tips are held in escrow and released only when all attached conditions
//! are fulfilled. Supports oracle integration and dispute resolution.

use soroban_sdk::{contracttype, symbol_short, token, Address, Env, String, Vec};

use crate::{conditions::types::Condition, DataKey};

/// Status of a conditional escrow.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowStatus {
    /// Funds are held, conditions not yet evaluated.
    Pending,
    /// All conditions passed; funds released to creator.
    Released,
    /// Conditions failed or dispute resolved against creator; funds refunded.
    Refunded,
    /// Under dispute review.
    Disputed,
}

/// A conditional escrow record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionalEscrow {
    pub id: u64,
    pub sender: Address,
    pub creator: Address,
    pub token: Address,
    pub amount: i128,
    pub conditions: Vec<Condition>,
    pub status: EscrowStatus,
    pub created_at: u64,
    /// Optional deadline after which the sender can reclaim funds.
    pub deadline: Option<u64>,
    pub description: String,
}

/// Create a new conditional escrow.
///
/// Transfers `amount` from `sender` into contract escrow.
/// Returns the escrow ID.
pub fn create_escrow(
    env: &Env,
    sender: &Address,
    creator: &Address,
    token_addr: &Address,
    amount: i128,
    conditions: Vec<Condition>,
    deadline: Option<u64>,
    description: String,
) -> u64 {
    let id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::EscrowCounter)
        .unwrap_or(0);
    env.storage().instance().set(&DataKey::EscrowCounter, &(id + 1));

    let escrow = ConditionalEscrow {
        id,
        sender: sender.clone(),
        creator: creator.clone(),
        token: token_addr.clone(),
        amount,
        conditions,
        status: EscrowStatus::Pending,
        created_at: env.ledger().timestamp(),
        deadline,
        description,
    };

    env.storage().persistent().set(&DataKey::Escrow(id), &escrow);

    // Track escrow IDs per creator and sender.
    let mut creator_escrows: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::CreatorEscrows(creator.clone()))
        .unwrap_or_else(|| Vec::new(env));
    creator_escrows.push_back(id);
    env.storage()
        .persistent()
        .set(&DataKey::CreatorEscrows(creator.clone()), &creator_escrows);

    token::Client::new(env, token_addr).transfer(
        sender,
        &env.current_contract_address(),
        &amount,
    );

    env.events().publish(
        (symbol_short!("esc_crt"),),
        (id, sender.clone(), creator.clone(), amount),
    );

    id
}

/// Attempt to release escrow funds to the creator.
///
/// Evaluates all conditions; releases if all pass, otherwise panics.
pub fn release_escrow(env: &Env, caller: &Address, escrow_id: u64) {
    let mut escrow: ConditionalEscrow = env
        .storage()
        .persistent()
        .get(&DataKey::Escrow(escrow_id))
        .expect("Escrow not found");

    assert!(escrow.status == EscrowStatus::Pending, "Escrow not pending");
    // Only creator or sender may trigger release.
    assert!(
        caller == &escrow.creator || caller == &escrow.sender,
        "Unauthorized"
    );

    let all_pass = crate::conditions::evaluator::evaluate_all(env, &escrow.conditions);
    assert!(all_pass, "Conditions not met");

    escrow.status = EscrowStatus::Released;
    env.storage().persistent().set(&DataKey::Escrow(escrow_id), &escrow);

    token::Client::new(env, &escrow.token).transfer(
        &env.current_contract_address(),
        &escrow.creator,
        &escrow.amount,
    );

    env.events().publish(
        (symbol_short!("esc_rel"),),
        (escrow_id, escrow.creator.clone(), escrow.amount),
    );
}

/// Refund escrow to sender.
///
/// Allowed when: deadline has passed, or conditions explicitly fail and
/// caller is the sender.
pub fn refund_escrow(env: &Env, caller: &Address, escrow_id: u64) {
    let mut escrow: ConditionalEscrow = env
        .storage()
        .persistent()
        .get(&DataKey::Escrow(escrow_id))
        .expect("Escrow not found");

    assert!(
        escrow.status == EscrowStatus::Pending || escrow.status == EscrowStatus::Disputed,
        "Escrow not refundable"
    );
    assert!(caller == &escrow.sender, "Only sender can refund");

    // Allow refund if deadline passed or conditions explicitly fail.
    let now = env.ledger().timestamp();
    let deadline_passed = escrow.deadline.map(|d| now >= d).unwrap_or(false);
    let conditions_fail = !crate::conditions::evaluator::evaluate_all(env, &escrow.conditions);

    assert!(deadline_passed || conditions_fail, "Cannot refund yet");

    escrow.status = EscrowStatus::Refunded;
    env.storage().persistent().set(&DataKey::Escrow(escrow_id), &escrow);

    token::Client::new(env, &escrow.token).transfer(
        &env.current_contract_address(),
        &escrow.sender,
        &escrow.amount,
    );

    env.events().publish(
        (symbol_short!("esc_ref"),),
        (escrow_id, escrow.sender.clone(), escrow.amount),
    );
}

/// Mark an escrow as disputed. Only sender or creator may dispute.
pub fn dispute_escrow(env: &Env, caller: &Address, escrow_id: u64) {
    let mut escrow: ConditionalEscrow = env
        .storage()
        .persistent()
        .get(&DataKey::Escrow(escrow_id))
        .expect("Escrow not found");

    assert!(escrow.status == EscrowStatus::Pending, "Escrow not pending");
    assert!(
        caller == &escrow.creator || caller == &escrow.sender,
        "Unauthorized"
    );

    escrow.status = EscrowStatus::Disputed;
    env.storage().persistent().set(&DataKey::Escrow(escrow_id), &escrow);

    env.events().publish(
        (symbol_short!("esc_dis"),),
        (escrow_id, caller.clone()),
    );
}

/// Resolve a disputed escrow. Admin only — releases to creator or refunds sender.
pub fn resolve_escrow_dispute(
    env: &Env,
    escrow_id: u64,
    release_to_creator: bool,
) {
    let mut escrow: ConditionalEscrow = env
        .storage()
        .persistent()
        .get(&DataKey::Escrow(escrow_id))
        .expect("Escrow not found");

    assert!(escrow.status == EscrowStatus::Disputed, "Escrow not disputed");

    let recipient = if release_to_creator {
        escrow.status = EscrowStatus::Released;
        escrow.creator.clone()
    } else {
        escrow.status = EscrowStatus::Refunded;
        escrow.sender.clone()
    };

    env.storage().persistent().set(&DataKey::Escrow(escrow_id), &escrow);

    token::Client::new(env, &escrow.token).transfer(
        &env.current_contract_address(),
        &recipient,
        &escrow.amount,
    );

    env.events().publish(
        (symbol_short!("esc_rsl"),),
        (escrow_id, recipient, release_to_creator),
    );
}

/// Get an escrow record by ID.
pub fn get_escrow(env: &Env, escrow_id: u64) -> Option<ConditionalEscrow> {
    env.storage().persistent().get(&DataKey::Escrow(escrow_id))
}

/// Get all escrow IDs for a creator.
pub fn get_creator_escrows(env: &Env, creator: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::CreatorEscrows(creator.clone()))
        .unwrap_or_else(|| Vec::new(env))
}
