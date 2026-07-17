//! Atomic hash-time-locked swap (HTLC) functionality for trustless tip token exchanges.

use soroban_sdk::{contracttype, symbol_short, token, Address, Bytes, BytesN, Env};

use crate::{DataKey, TipJarError};

/// Status of an atomic swap.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SwapStatus {
    /// Awaiting execution or refund.
    Pending,
    /// Recipient claimed the funds with the correct preimage.
    Completed,
    /// Initiator reclaimed the funds after the time lock expired.
    Refunded,
}

/// An atomic hash-time-locked swap between two parties.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtomicSwap {
    /// Unique swap identifier.
    pub id: u64,
    /// Address that created and funded the swap.
    pub initiator: Address,
    /// Address that may claim the funds by revealing the preimage.
    pub recipient: Address,
    /// Token contract address.
    pub token: Address,
    /// Amount of tokens escrowed.
    pub amount: i128,
    /// SHA-256 hash of the secret preimage.
    pub hash_lock: BytesN<32>,
    /// Ledger timestamp after which the initiator may refund.
    pub time_lock: u64,
    /// Current lifecycle status.
    pub status: SwapStatus,
}

// ── storage helpers ──────────────────────────────────────────────────────────

fn next_swap_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::SwapCounter)
        .unwrap_or(0u64);
    let next = id + 1;
    env.storage()
        .instance()
        .set(&DataKey::SwapCounter, &next);
    next
}

fn load_swap(env: &Env, id: u64) -> AtomicSwap {
    env.storage()
        .persistent()
        .get(&DataKey::Swap(id))
        .unwrap_or_else(|| soroban_sdk::panic_with_error!(env, TipJarError::SwapNotFound))
}

fn save_swap(env: &Env, swap: &AtomicSwap) {
    env.storage()
        .persistent()
        .set(&DataKey::Swap(swap.id), swap);
}

// ── public interface ─────────────────────────────────────────────────────────

/// Creates a new hash-time-locked swap and escrows `amount` tokens from `initiator`.
///
/// * `hash_lock` – SHA-256 hash of the secret the recipient must reveal.
/// * `time_lock` – Unix timestamp (seconds) after which the initiator may refund.
pub fn create_swap(
    env: &Env,
    initiator: Address,
    recipient: Address,
    token: Address,
    amount: i128,
    hash_lock: BytesN<32>,
    time_lock: u64,
) -> u64 {
    initiator.require_auth();

    if amount <= 0 {
        soroban_sdk::panic_with_error!(env, TipJarError::InvalidAmount);
    }
    if time_lock <= env.ledger().timestamp() {
        soroban_sdk::panic_with_error!(env, TipJarError::InvalidTimeLock);
    }

    // Escrow tokens into this contract.
    token::Client::new(env, &token).transfer(
        &initiator,
        &env.current_contract_address(),
        &amount,
    );

    let id = next_swap_id(env);
    let swap = AtomicSwap {
        id,
        initiator: initiator.clone(),
        recipient: recipient.clone(),
        token,
        amount,
        hash_lock,
        time_lock,
        status: SwapStatus::Pending,
    };
    save_swap(env, &swap);

    env.events()
        .publish((symbol_short!("swap_new"),), (id, initiator, recipient, amount));

    id
}

/// Executes a pending swap by revealing the preimage.
///
/// The SHA-256 hash of `preimage` must match the stored `hash_lock`.
pub fn execute_swap(env: &Env, id: u64, preimage: BytesN<32>) {
    let mut swap = load_swap(env, id);

    if swap.status != SwapStatus::Pending {
        soroban_sdk::panic_with_error!(env, TipJarError::SwapNotPending);
    }

    // Verify hash lock: sha256(preimage) == hash_lock.
    let digest: BytesN<32> = env.crypto().sha256(&Bytes::from(&preimage)).into();
    if digest != swap.hash_lock {
        soroban_sdk::panic_with_error!(env, TipJarError::InvalidPreimage);
    }

    swap.status = SwapStatus::Completed;
    save_swap(env, &swap);

    token::Client::new(env, &swap.token).transfer(
        &env.current_contract_address(),
        &swap.recipient,
        &swap.amount,
    );

    env.events()
        .publish((symbol_short!("swap_done"),), (id,));
}

/// Refunds a pending swap to the initiator after the time lock has expired.
pub fn refund_swap(env: &Env, id: u64) {
    let mut swap = load_swap(env, id);

    if swap.status != SwapStatus::Pending {
        soroban_sdk::panic_with_error!(env, TipJarError::SwapNotPending);
    }
    if env.ledger().timestamp() < swap.time_lock {
        soroban_sdk::panic_with_error!(env, TipJarError::TimeLockNotExpired);
    }

    swap.initiator.require_auth();
    swap.status = SwapStatus::Refunded;
    save_swap(env, &swap);

    token::Client::new(env, &swap.token).transfer(
        &env.current_contract_address(),
        &swap.initiator,
        &swap.amount,
    );

    env.events()
        .publish((symbol_short!("swap_rfnd"),), (id,));
}

/// Returns the swap record for the given ID.
pub fn get_swap(env: &Env, id: u64) -> AtomicSwap {
    load_swap(env, id)
}
