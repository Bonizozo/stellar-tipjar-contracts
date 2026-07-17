//! Tip state channels — off-chain tip transactions with on-chain settlement.
//!
//! Lifecycle:
//!   open_tip_channel → record_tip (off-chain, many times) → settle_channel
//!                                                          → dispute_channel (unilateral)

use soroban_sdk::{contracttype, symbol_short, token, Address, Env, Vec};

use crate::DataKey;

// ── Types ────────────────────────────────────────────────────────────────────

/// Status of a tip state channel.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TipChannelStatus {
    /// Channel is open; off-chain tips are being accumulated.
    Open,
    /// A unilateral close has been initiated; dispute window is active.
    Disputed,
    /// Channel is settled; funds have been distributed.
    Settled,
}

/// A single off-chain tip record committed to the channel state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChannelTip {
    /// Sequential index of this tip within the channel.
    pub index: u64,
    /// Amount tipped.
    pub amount: i128,
    /// Ledger timestamp when this tip was recorded on-chain.
    pub recorded_at: u64,
}

/// On-chain record for a tip state channel between a tipper and a creator.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipStateChannel {
    /// Tipper (funder of the channel).
    pub tipper: Address,
    /// Creator (recipient of tips).
    pub creator: Address,
    /// Token used in this channel.
    pub token: Address,
    /// Total collateral deposited by the tipper.
    pub deposit: i128,
    /// Cumulative amount tipped to the creator so far (off-chain total).
    pub tipped_amount: i128,
    /// Monotonically increasing state nonce; prevents replay of stale states.
    pub nonce: u64,
    /// Number of individual tips recorded in this channel.
    pub tip_count: u64,
    /// Current channel status.
    pub status: TipChannelStatus,
    /// Ledger timestamp when the channel was opened.
    pub opened_at: u64,
    /// Ledger timestamp when a dispute was initiated (0 if none).
    pub dispute_started_at: u64,
    /// Seconds the counterparty has to submit a newer state during a dispute.
    pub dispute_window: u64,
    /// Address that initiated the current dispute (if any).
    pub disputer: Option<Address>,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

fn channel_key(tipper: &Address, creator: &Address, token: &Address) -> DataKey {
    DataKey::TipChannel(tipper.clone(), creator.clone(), token.clone())
}

fn tip_key(tipper: &Address, creator: &Address, token: &Address, index: u64) -> DataKey {
    DataKey::TipChannelEntry(tipper.clone(), creator.clone(), token.clone(), index)
}

/// Load a channel or panic.
pub fn load(env: &Env, tipper: &Address, creator: &Address, token: &Address) -> TipStateChannel {
    env.storage()
        .persistent()
        .get(&channel_key(tipper, creator, token))
        .expect("TipChannel not found")
}

/// Persist a channel.
pub fn save(env: &Env, channel: &TipStateChannel) {
    env.storage().persistent().set(
        &channel_key(&channel.tipper, &channel.creator, &channel.token),
        channel,
    );
}

// ── Core operations ──────────────────────────────────────────────────────────

/// Opens a new tip state channel, transferring `deposit` from `tipper` into escrow.
/// Returns the created channel.
pub fn open(
    env: &Env,
    tipper: &Address,
    creator: &Address,
    token: &Address,
    deposit: i128,
    dispute_window: u64,
) -> TipStateChannel {
    let channel = TipStateChannel {
        tipper: tipper.clone(),
        creator: creator.clone(),
        token: token.clone(),
        deposit,
        tipped_amount: 0,
        nonce: 0,
        tip_count: 0,
        status: TipChannelStatus::Open,
        opened_at: env.ledger().timestamp(),
        dispute_started_at: 0,
        dispute_window,
        disputer: None,
    };

    save(env, &channel);

    token::Client::new(env, token).transfer(tipper, &env.current_contract_address(), &deposit);

    env.events().publish(
        (symbol_short!("tch_open"),),
        (tipper.clone(), creator.clone(), token.clone(), deposit),
    );

    channel
}

/// Records an off-chain tip update. Both parties must authorise.
///
/// `new_tipped_amount` is the new cumulative total tipped; must be ≥ current and ≤ deposit.
/// `nonce` must be strictly greater than the current nonce.
pub fn record_tip(
    env: &Env,
    tipper: &Address,
    creator: &Address,
    token: &Address,
    new_tipped_amount: i128,
    nonce: u64,
) {
    let mut channel = load(env, tipper, creator, token);

    assert!(channel.status == TipChannelStatus::Open, "Channel not open");
    assert!(nonce > channel.nonce, "Stale nonce");
    assert!(
        new_tipped_amount >= channel.tipped_amount,
        "Tipped amount cannot decrease"
    );
    assert!(new_tipped_amount <= channel.deposit, "Exceeds deposit");

    let tip_index = channel.tip_count;
    let tip_amount = new_tipped_amount - channel.tipped_amount;

    channel.tipped_amount = new_tipped_amount;
    channel.nonce = nonce;
    channel.tip_count += 1;

    // Store individual tip entry for history/audit.
    let entry = ChannelTip {
        index: tip_index,
        amount: tip_amount,
        recorded_at: env.ledger().timestamp(),
    };
    env.storage()
        .persistent()
        .set(&tip_key(tipper, creator, token, tip_index), &entry);

    save(env, &channel);

    env.events().publish(
        (symbol_short!("tch_tip"),),
        (
            tipper.clone(),
            creator.clone(),
            token.clone(),
            tip_amount,
            nonce,
        ),
    );
}

/// Cooperatively settles the channel. Both parties must authorise.
///
/// Distributes `tipped_amount` to creator and remainder back to tipper.
pub fn settle(env: &Env, tipper: &Address, creator: &Address, token: &Address) {
    let mut channel = load(env, tipper, creator, token);

    assert!(channel.status == TipChannelStatus::Open, "Channel not open");

    let to_creator = channel.tipped_amount;
    let to_tipper = channel.deposit - to_creator;

    channel.status = TipChannelStatus::Settled;
    save(env, &channel);

    let tok = token::Client::new(env, token);
    if to_creator > 0 {
        tok.transfer(&env.current_contract_address(), creator, &to_creator);
    }
    if to_tipper > 0 {
        tok.transfer(&env.current_contract_address(), tipper, &to_tipper);
    }

    env.events().publish(
        (symbol_short!("tch_setl"),),
        (
            tipper.clone(),
            creator.clone(),
            token.clone(),
            to_creator,
            to_tipper,
        ),
    );
}

/// Initiates or finalises a unilateral dispute close.
///
/// First call: sets status to `Disputed`, records claimed state.
/// Second call (counterparty, within window): may submit a newer state.
/// After window: anyone may finalise with the last submitted state.
pub fn dispute(
    env: &Env,
    caller: &Address,
    tipper: &Address,
    creator: &Address,
    token: &Address,
    claimed_tipped_amount: i128,
    nonce: u64,
) {
    let mut channel = load(env, tipper, creator, token);

    assert!(
        caller == &channel.tipper || caller == &channel.creator,
        "Not a channel party"
    );

    let now = env.ledger().timestamp();

    match channel.status {
        TipChannelStatus::Open => {
            assert!(claimed_tipped_amount >= 0, "Invalid tipped amount");
            assert!(claimed_tipped_amount <= channel.deposit, "Exceeds deposit");
            assert!(nonce >= channel.nonce, "Stale nonce");

            channel.status = TipChannelStatus::Disputed;
            channel.dispute_started_at = now;
            channel.disputer = Some(caller.clone());
            channel.tipped_amount = claimed_tipped_amount;
            channel.nonce = nonce;
            save(env, &channel);

            env.events().publish(
                (symbol_short!("tch_disp"),),
                (
                    caller.clone(),
                    tipper.clone(),
                    creator.clone(),
                    token.clone(),
                    claimed_tipped_amount,
                    nonce,
                ),
            );
        }
        TipChannelStatus::Disputed => {
            let window_end = channel.dispute_started_at + channel.dispute_window;

            if now < window_end {
                // Counterparty submits a newer state.
                assert!(nonce > channel.nonce, "Stale nonce");
                assert!(claimed_tipped_amount >= 0, "Invalid tipped amount");
                assert!(claimed_tipped_amount <= channel.deposit, "Exceeds deposit");

                channel.tipped_amount = claimed_tipped_amount;
                channel.nonce = nonce;
                save(env, &channel);

                env.events().publish(
                    (symbol_short!("tch_disp"),),
                    (
                        caller.clone(),
                        tipper.clone(),
                        creator.clone(),
                        token.clone(),
                        claimed_tipped_amount,
                        nonce,
                    ),
                );
            } else {
                // Window elapsed — finalise with last submitted state.
                let to_creator = channel.tipped_amount;
                let to_tipper = channel.deposit - to_creator;

                channel.status = TipChannelStatus::Settled;
                save(env, &channel);

                let tok = token::Client::new(env, token);
                if to_creator > 0 {
                    tok.transfer(&env.current_contract_address(), creator, &to_creator);
                }
                if to_tipper > 0 {
                    tok.transfer(&env.current_contract_address(), tipper, &to_tipper);
                }

                env.events().publish(
                    (symbol_short!("tch_fin"),),
                    (
                        tipper.clone(),
                        creator.clone(),
                        token.clone(),
                        to_creator,
                        to_tipper,
                    ),
                );
            }
        }
        TipChannelStatus::Settled => {
            panic!("Channel already settled");
        }
    }
}

/// Returns the remaining balance available for future tips.
pub fn remaining_balance(channel: &TipStateChannel) -> i128 {
    channel.deposit - channel.tipped_amount
}

/// Returns all tip entries for a channel.
pub fn get_channel_tips(
    env: &Env,
    tipper: &Address,
    creator: &Address,
    token: &Address,
) -> Vec<ChannelTip> {
    let channel = load(env, tipper, creator, token);
    let mut tips = Vec::new(env);
    for i in 0..channel.tip_count {
        if let Some(entry) = env
            .storage()
            .persistent()
            .get(&tip_key(tipper, creator, token, i))
        {
            tips.push_back(entry);
        }
    }
    tips
}
