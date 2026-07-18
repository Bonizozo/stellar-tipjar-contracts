//! Streaming vesting module (#247).
//!
//! Tips unlock continuously per second over a vesting period.
//! Supports cancellation (refunds unvested portion to sender) and partial withdrawals.

use soroban_sdk::{contracttype, symbol_short, token, Address, Env, Vec};

use crate::DataKey;

// ── Types ────────────────────────────────────────────────────────────────────

/// Status of a streaming vesting position.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VestingStreamStatus {
    Active,
    Cancelled,
    Completed,
}

/// A streaming vesting record: tokens unlock linearly per second.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VestingStream {
    pub id: u64,
    pub sender: Address,
    pub recipient: Address,
    pub token: Address,
    /// Total tokens deposited into this stream.
    pub total_amount: i128,
    /// Tokens already withdrawn by the recipient.
    pub withdrawn: i128,
    /// Unix timestamp when streaming begins.
    pub start_time: u64,
    /// Unix timestamp when streaming ends (all tokens fully vested).
    pub end_time: u64,
    pub status: VestingStreamStatus,
    pub created_at: u64,
}

// ── DataKey sub-keys ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VestingStreamKey {
    /// Global counter for stream IDs.
    Counter,
    /// Individual stream record keyed by ID.
    Record(u64),
    /// List of stream IDs where `address` is the recipient.
    RecipientStreams(Address),
    /// List of stream IDs where `address` is the sender.
    SenderStreams(Address),
}

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Compute how many tokens have vested up to `now`.
fn vested_at(stream: &VestingStream, now: u64) -> i128 {
    if now <= stream.start_time {
        return 0;
    }
    let duration = stream.end_time.saturating_sub(stream.start_time);
    if duration == 0 {
        return stream.total_amount;
    }
    let elapsed = now.min(stream.end_time).saturating_sub(stream.start_time);
    (stream.total_amount * elapsed as i128) / duration as i128
}

fn load_stream(env: &Env, id: u64) -> VestingStream {
    env.storage()
        .persistent()
        .get(&DataKey::VestingStream(VestingStreamKey::Record(id)))
        .unwrap_or_else(|| panic!("VestingStream not found"))
}

fn save_stream(env: &Env, stream: &VestingStream) {
    env.storage()
        .persistent()
        .set(&DataKey::VestingStream(VestingStreamKey::Record(stream.id)), stream);
}

fn push_to_list(env: &Env, key: DataKey, id: u64) {
    let mut list: Vec<u64> = env.storage().persistent().get(&key).unwrap_or_else(|| Vec::new(env));
    list.push_back(id);
    env.storage().persistent().set(&key, &list);
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Create a new streaming vesting position.
///
/// Transfers `total_amount` of `token` from `sender` into escrow.
/// `duration_seconds` must be > 0.
/// Returns the stream ID.
/// Emits `("vs_create",)` with `(id, sender, recipient, total_amount, end_time)`.
pub fn create(
    env: &Env,
    sender: &Address,
    recipient: &Address,
    token_addr: &Address,
    total_amount: i128,
    duration_seconds: u64,
) -> u64 {
    assert!(total_amount > 0, "amount must be positive");
    assert!(duration_seconds > 0, "duration must be positive");

    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::VestingStream(VestingStreamKey::Counter))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&DataKey::VestingStream(VestingStreamKey::Counter), &(id + 1));

    let now = env.ledger().timestamp();
    let stream = VestingStream {
        id,
        sender: sender.clone(),
        recipient: recipient.clone(),
        token: token_addr.clone(),
        total_amount,
        withdrawn: 0,
        start_time: now,
        end_time: now.saturating_add(duration_seconds),
        status: VestingStreamStatus::Active,
        created_at: now,
    };

    save_stream(env, &stream);
    push_to_list(env, DataKey::VestingStream(VestingStreamKey::RecipientStreams(recipient.clone())), id);
    push_to_list(env, DataKey::VestingStream(VestingStreamKey::SenderStreams(sender.clone())), id);

    token::Client::new(env, token_addr).transfer(sender, &env.current_contract_address(), &total_amount);

    env.events().publish(
        (symbol_short!("vs_crt"),),
        (id, sender.clone(), recipient.clone(), total_amount, stream.end_time),
    );

    id
}

/// Withdraw all currently vested (but not yet withdrawn) tokens to the recipient.
///
/// Returns the amount withdrawn.
/// Emits `("vs_wdr",)` with `(id, recipient, amount)`.
pub fn withdraw(env: &Env, recipient: &Address, stream_id: u64) -> i128 {
    let mut stream = load_stream(env, stream_id);
    assert!(&stream.recipient == recipient, "unauthorized");
    assert!(stream.status == VestingStreamStatus::Active, "stream not active");

    let now = env.ledger().timestamp();
    let vested = vested_at(&stream, now);
    let available = vested.saturating_sub(stream.withdrawn);
    assert!(available > 0, "nothing to withdraw");

    stream.withdrawn = stream.withdrawn.saturating_add(available);
    if now >= stream.end_time {
        stream.status = VestingStreamStatus::Completed;
    }
    save_stream(env, &stream);

    token::Client::new(env, &stream.token).transfer(
        &env.current_contract_address(),
        recipient,
        &available,
    );

    env.events().publish(
        (symbol_short!("vs_wdr"),),
        (stream_id, recipient.clone(), available),
    );

    available
}

/// Cancel a stream. Only the original sender may cancel.
///
/// Transfers vested-but-unwithdrawn tokens to the recipient and refunds
/// the unvested remainder to the sender.
/// Emits `("vs_cancel",)` with `(id, refunded_to_sender, paid_to_recipient)`.
pub fn cancel(env: &Env, sender: &Address, stream_id: u64) {
    let mut stream = load_stream(env, stream_id);
    assert!(&stream.sender == sender, "unauthorized");
    assert!(stream.status == VestingStreamStatus::Active, "stream not active");

    let now = env.ledger().timestamp();
    let vested = vested_at(&stream, now);
    let recipient_amount = vested.saturating_sub(stream.withdrawn);
    let sender_refund = stream.total_amount.saturating_sub(vested);

    stream.status = VestingStreamStatus::Cancelled;
    save_stream(env, &stream);

    let tok = token::Client::new(env, &stream.token);
    if recipient_amount > 0 {
        tok.transfer(&env.current_contract_address(), &stream.recipient, &recipient_amount);
    }
    if sender_refund > 0 {
        tok.transfer(&env.current_contract_address(), sender, &sender_refund);
    }

    env.events().publish(
        (symbol_short!("vs_can"),),
        (stream_id, sender_refund, recipient_amount),
    );
}

/// Returns the amount currently available to withdraw for a stream.
pub fn available_to_withdraw(env: &Env, stream_id: u64) -> i128 {
    let stream = load_stream(env, stream_id);
    if stream.status != VestingStreamStatus::Active {
        return 0;
    }
    let vested = vested_at(&stream, env.ledger().timestamp());
    vested.saturating_sub(stream.withdrawn)
}

/// Returns the stream record.
pub fn get_stream(env: &Env, stream_id: u64) -> VestingStream {
    load_stream(env, stream_id)
}

/// Returns all stream IDs for a recipient.
pub fn get_recipient_streams(env: &Env, recipient: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::VestingStream(VestingStreamKey::RecipientStreams(recipient.clone())))
        .unwrap_or_else(|| Vec::new(env))
}

/// Returns all stream IDs for a sender.
pub fn get_sender_streams(env: &Env, sender: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::VestingStream(VestingStreamKey::SenderStreams(sender.clone())))
        .unwrap_or_else(|| Vec::new(env))
}
