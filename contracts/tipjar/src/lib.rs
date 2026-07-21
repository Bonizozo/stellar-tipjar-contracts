#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Env, MuxedAddress,
};

#[cfg(test)]
mod test;
#[cfg(test)]
mod test_exhaustive;
#[cfg(test)]
mod test_invariants;

/// Ledger TTL bump applied to instance and persistent storage on every write.
const LEDGER_THRESHOLD: u32 = 100_000;
const LEDGER_BUMP: u32 = 120_960; // ~7 days at 5s/ledger

const PAYOUT_DELAY_LEDGERS: u32 = 17280; // ~1 day at 5s/ledger

/// Basis-point denominator for fee math (1 bps = 1/10_000).
const BPS_DENOMINATOR: i128 = 10_000;
/// Hard on-chain ceiling for the protocol fee: 1_000 bps = 10%.
const MAX_FEE_BPS: u32 = 1_000;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Address of the SEP-41 token this jar accepts.
    Token,
    /// Withdrawable balance escrowed for a creator.
    CreatorBalance(Address),
    /// Historical total ever tipped to a creator (never decreases). Tracks
    /// the gross amount tipped, before any protocol fee is deducted.
    CreatorTotal(Address),
    /// Payout address designated for a creator.
    PayoutAddress(Address),
    /// Pending change to payout address: (creator) -> (new_payout, effective_ledger)
    PendingPayoutChange(Address),
    /// Operator delegation: (creator, operator) -> (allowance, expiry_ledger)
    Operator(Address, Address),
    /// Contract admin, authorized to configure the protocol fee and propose
    /// admin transfers.
    Admin,
    /// Address proposed as the next admin; not yet in effect until it accepts.
    PendingAdmin,
    /// Protocol fee rate in basis points. Absent or 0 means no fee.
    FeeBps,
    /// Address authorized to withdraw accrued protocol fees.
    FeeCollector,
    /// Withdrawable protocol fee balance, accrued from tips.
    FeeBalance,
}

/// Topics `("tip", creator)`, data `(sender, amount)`.
#[contractevent(data_format = "vec")]
pub struct Tip {
    #[topic]
    creator: Address,
    sender: Address,
    amount: i128,
}

/// Topics `("withdraw", creator)`, data `[amount, to]`.
#[contractevent(data_format = "vec")]
pub struct Withdraw {
    #[topic]
    creator: Address,
    amount: i128,
    to: Address,
}

#[contractevent(data_format = "vec")]
pub struct PayoutChangeProposed {
    #[topic]
    creator: Address,
    new_payout: Address,
    effective_ledger: u32,
}

#[contractevent(data_format = "vec")]
pub struct PayoutChangeApplied {
    #[topic]
    creator: Address,
    new_payout: Address,
}

#[contractevent(data_format = "vec")]
pub struct PayoutChangeCancelled {
    #[topic]
    creator: Address,
}

#[contractevent(data_format = "vec")]
pub struct OperatorAuthorized {
    #[topic]
    creator: Address,
    #[topic]
    operator: Address,
    allowance: i128,
    expiry_ledger: u32,
}

#[contractevent(data_format = "vec")]
pub struct OperatorRevoked {
    #[topic]
    creator: Address,
    #[topic]
    operator: Address,
}

/// Topics `("fee_charged", creator)`, data `[gross, fee, net]`.
/// Emitted alongside `Tip` whenever a nonzero fee rate is configured, so the
/// indexer can reconstruct accounting without re-deriving the fee schedule
/// that was active at the time of the tip.
#[contractevent(data_format = "vec")]
pub struct FeeCharged {
    #[topic]
    creator: Address,
    gross: i128,
    fee: i128,
    net: i128,
}

/// Topics `("fee_configured", admin)`, data `[bps, collector]`.
#[contractevent(data_format = "vec")]
pub struct FeeConfigured {
    #[topic]
    admin: Address,
    bps: u32,
    collector: Address,
}

/// Topics `("fee_withdraw", collector)`, data `[amount]`.
#[contractevent(data_format = "vec")]
pub struct FeeWithdraw {
    #[topic]
    collector: Address,
    amount: i128,
}

/// Topics `("admin_transfer_proposed", current_admin)`, data `[pending_admin]`.
#[contractevent(data_format = "vec")]
pub struct AdminTransferProposed {
    #[topic]
    current_admin: Address,
    pending_admin: Address,
}

/// Topics `("admin_transfer_accepted", new_admin)`, data `[]`.
#[contractevent(data_format = "vec")]
pub struct AdminTransferAccepted {
    #[topic]
    new_admin: Address,
}

/// Topics `("admin_transfer_cancelled", admin)`, data `[]`.
#[contractevent(data_format = "vec")]
pub struct AdminTransferCancelled {
    #[topic]
    admin: Address,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAmount = 3,
    NothingToWithdraw = 4,
    InvalidOperator = 5,
    OperatorExpired = 6,
    InsufficientAllowance = 7,
    PendingPayoutChangeActive = 8,
    NoPendingPayoutChange = 9,
    InvalidTarget = 10,
    NotAdmin = 11,
    NotPendingAdmin = 12,
    NoPendingAdminProposal = 13,
    InvalidFee = 14,
    FeeOverflow = 15,
    NotFeeCollector = 16,
}

#[contract]
pub struct TipJar;

#[contractimpl]
impl TipJar {
    /// One-time configuration of the token this jar accepts and its admin.
    /// Errors if called twice.
    pub fn init(env: Env, token: Address, admin: Address) {
        if env.storage().instance().has(&DataKey::Token) {
            panic_with_error!(&env, Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
    }

    /// Escrows `amount` of the configured token from `sender` for `creator`,
    /// less the protocol fee (if one is configured). The creator's balance is
    /// credited with `amount - fee`; the fee itself accrues to `FeeBalance`
    /// for later withdrawal by the fee collector. `fee + net == amount` holds
    /// for every input.
    pub fn tip(env: Env, sender: Address, creator: Address, amount: i128) {
        sender.require_auth();

        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }

        let fee_bps: u32 = env.storage().instance().get(&DataKey::FeeBps).unwrap_or(0);
        let fee = Self::fee_for(&env, amount, fee_bps);
        let net_amount = amount - fee;

        let token = Self::token_address(&env);
        let contract_address = env.current_contract_address();

        token::TokenClient::new(&env, &token).transfer(
            &sender,
            MuxedAddress::from(contract_address),
            &amount,
        );

        let balance_key = DataKey::CreatorBalance(creator.clone());
        let total_key = DataKey::CreatorTotal(creator.clone());

        let balance: i128 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        let total: i128 = env.storage().persistent().get(&total_key).unwrap_or(0);

        let new_balance = balance
            .checked_add(net_amount)
            .unwrap_or_else(|| panic_with_error!(&env, Error::InvalidAmount));
        let new_total = total
            .checked_add(amount)
            .unwrap_or_else(|| panic_with_error!(&env, Error::InvalidAmount));

        env.storage().persistent().set(&balance_key, &new_balance);
        env.storage().persistent().set(&total_key, &new_total);
        env.storage()
            .persistent()
            .extend_ttl(&balance_key, LEDGER_THRESHOLD, LEDGER_BUMP);
        env.storage()
            .persistent()
            .extend_ttl(&total_key, LEDGER_THRESHOLD, LEDGER_BUMP);

        // fee_bps == 0 is a true no-op: no fee storage entry, no fee event.
        if fee_bps > 0 {
            let fee_balance_key = DataKey::FeeBalance;
            let fee_balance: i128 = env
                .storage()
                .persistent()
                .get(&fee_balance_key)
                .unwrap_or(0);
            let new_fee_balance = fee_balance
                .checked_add(fee)
                .unwrap_or_else(|| panic_with_error!(&env, Error::FeeOverflow));
            env.storage()
                .persistent()
                .set(&fee_balance_key, &new_fee_balance);
            env.storage()
                .persistent()
                .extend_ttl(&fee_balance_key, LEDGER_THRESHOLD, LEDGER_BUMP);

            FeeCharged {
                creator: creator.clone(),
                gross: amount,
                fee,
                net: net_amount,
            }
            .publish(&env);
        }

        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        Tip {
            creator,
            sender,
            amount,
        }
        .publish(&env);
    }

    /// Historical total ever tipped to `creator`. Zero if the creator has never been tipped.
    pub fn get_total_tips(env: Env, creator: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::CreatorTotal(creator))
            .unwrap_or(0)
    }

    /// Pays out a creator's full or partial withdrawable balance.
    /// If caller is an operator, checks their allowance and expiry.
    /// Applies any matured payout address change before transferring.
    pub fn withdraw(
        env: Env,
        caller: Address,
        creator: Address,
        to: Address,
        amount: Option<i128>,
    ) {
        caller.require_auth();

        let balance_key = DataKey::CreatorBalance(creator.clone());
        let balance: i128 = env.storage().persistent().get(&balance_key).unwrap_or(0);

        if balance == 0 {
            panic_with_error!(&env, Error::NothingToWithdraw);
        }

        let amount_to_withdraw = amount.unwrap_or(balance);
        if amount_to_withdraw <= 0 || amount_to_withdraw > balance {
            panic_with_error!(&env, Error::InvalidAmount);
        }

        if caller != creator {
            let operator_key = DataKey::Operator(creator.clone(), caller.clone());
            let operator_data: Option<(i128, u32)> = env.storage().persistent().get(&operator_key);
            match operator_data {
                Some((allowance, expiry_ledger)) => {
                    if env.ledger().sequence() > expiry_ledger {
                        panic_with_error!(&env, Error::OperatorExpired);
                    }
                    if allowance < amount_to_withdraw {
                        panic_with_error!(&env, Error::InsufficientAllowance);
                    }

                    let new_allowance = allowance - amount_to_withdraw;
                    if new_allowance == 0 {
                        env.storage().persistent().remove(&operator_key);
                    } else {
                        env.storage()
                            .persistent()
                            .set(&operator_key, &(new_allowance, expiry_ledger));
                        env.storage().persistent().extend_ttl(
                            &operator_key,
                            LEDGER_THRESHOLD,
                            LEDGER_BUMP,
                        );
                    }
                }
                None => {
                    panic_with_error!(&env, Error::InvalidOperator);
                }
            }
        }

        // Apply pending payout address change if matured
        let pending_key = DataKey::PendingPayoutChange(creator.clone());
        let payout_key = DataKey::PayoutAddress(creator.clone());
        if let Some((new_payout, effective_ledger)) = env
            .storage()
            .persistent()
            .get::<_, (Address, u32)>(&pending_key)
        {
            if env.ledger().sequence() >= effective_ledger {
                env.storage().persistent().set(&payout_key, &new_payout);
                env.storage()
                    .persistent()
                    .extend_ttl(&payout_key, LEDGER_THRESHOLD, LEDGER_BUMP);
                env.storage().persistent().remove(&pending_key);
                PayoutChangeApplied {
                    creator: creator.clone(),
                    new_payout,
                }
                .publish(&env);
            }
        }

        let payout_address: Address = env
            .storage()
            .persistent()
            .get(&payout_key)
            .unwrap_or(creator.clone());
        if env.storage().persistent().has(&payout_key) {
            env.storage()
                .persistent()
                .extend_ttl(&payout_key, LEDGER_THRESHOLD, LEDGER_BUMP);
        }

        if to != payout_address {
            panic_with_error!(&env, Error::InvalidTarget);
        }

        let token = Self::token_address(&env);
        let contract_address = env.current_contract_address();

        token::TokenClient::new(&env, &token).transfer(
            &contract_address,
            MuxedAddress::from(payout_address.clone()),
            &amount_to_withdraw,
        );

        let new_balance = balance - amount_to_withdraw;
        env.storage().persistent().set(&balance_key, &new_balance);
        env.storage()
            .persistent()
            .extend_ttl(&balance_key, LEDGER_THRESHOLD, LEDGER_BUMP);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        Withdraw {
            creator,
            amount: amount_to_withdraw,
            to: payout_address,
        }
        .publish(&env);
    }

    pub fn set_payout_address(env: Env, creator: Address, payout: Address) {
        creator.require_auth();
        let effective_ledger = env.ledger().sequence() + PAYOUT_DELAY_LEDGERS;
        let key = DataKey::PendingPayoutChange(creator.clone());
        env.storage()
            .persistent()
            .set(&key, &(payout.clone(), effective_ledger));
        env.storage()
            .persistent()
            .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
        PayoutChangeProposed {
            creator,
            new_payout: payout,
            effective_ledger,
        }
        .publish(&env);
    }

    pub fn cancel_payout_address(env: Env, creator: Address) {
        creator.require_auth();
        let key = DataKey::PendingPayoutChange(creator.clone());
        if !env.storage().persistent().has(&key) {
            panic_with_error!(&env, Error::NoPendingPayoutChange);
        }
        env.storage().persistent().remove(&key);
        PayoutChangeCancelled { creator }.publish(&env);
    }

    pub fn authorize_operator(
        env: Env,
        creator: Address,
        operator: Address,
        allowance: i128,
        expiry_ledger: u32,
    ) {
        creator.require_auth();
        let key = DataKey::Operator(creator.clone(), operator.clone());
        env.storage()
            .persistent()
            .set(&key, &(allowance, expiry_ledger));
        env.storage()
            .persistent()
            .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
        OperatorAuthorized {
            creator,
            operator,
            allowance,
            expiry_ledger,
        }
        .publish(&env);
    }

    pub fn revoke_operator(env: Env, creator: Address, operator: Address) {
        creator.require_auth();
        let key = DataKey::Operator(creator.clone(), operator.clone());
        env.storage().persistent().remove(&key);
        OperatorRevoked { creator, operator }.publish(&env);
    }

    /// Sets the protocol fee rate and its collector. Admin-only; `bps` is
    /// hard-capped at `MAX_FEE_BPS`. Setting `bps` to 0 disables fees.
    pub fn set_fee(env: Env, caller: Address, bps: u32, collector: Address) {
        caller.require_auth();
        if caller != Self::admin_address(&env) {
            panic_with_error!(&env, Error::NotAdmin);
        }
        if bps > MAX_FEE_BPS {
            panic_with_error!(&env, Error::InvalidFee);
        }

        env.storage().instance().set(&DataKey::FeeBps, &bps);
        env.storage()
            .instance()
            .set(&DataKey::FeeCollector, &collector);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        FeeConfigured {
            admin: caller,
            bps,
            collector,
        }
        .publish(&env);
    }

    /// Pays out the fee collector's full or partial share of `FeeBalance`.
    /// Mirrors `withdraw`'s pattern, including TTL extension.
    pub fn withdraw_fees(env: Env, caller: Address, amount: Option<i128>) {
        caller.require_auth();

        let collector: Address = env
            .storage()
            .instance()
            .get(&DataKey::FeeCollector)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NothingToWithdraw));
        if caller != collector {
            panic_with_error!(&env, Error::NotFeeCollector);
        }

        let balance_key = DataKey::FeeBalance;
        let balance: i128 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        if balance == 0 {
            panic_with_error!(&env, Error::NothingToWithdraw);
        }

        let amount_to_withdraw = amount.unwrap_or(balance);
        if amount_to_withdraw <= 0 || amount_to_withdraw > balance {
            panic_with_error!(&env, Error::InvalidAmount);
        }

        let token = Self::token_address(&env);
        let contract_address = env.current_contract_address();

        token::TokenClient::new(&env, &token).transfer(
            &contract_address,
            MuxedAddress::from(collector.clone()),
            &amount_to_withdraw,
        );

        let new_balance = balance - amount_to_withdraw;
        env.storage().persistent().set(&balance_key, &new_balance);
        env.storage()
            .persistent()
            .extend_ttl(&balance_key, LEDGER_THRESHOLD, LEDGER_BUMP);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        FeeWithdraw {
            collector,
            amount: amount_to_withdraw,
        }
        .publish(&env);
    }

    /// Proposes `new_admin` as the next admin. Takes effect only once
    /// `new_admin` calls `accept_admin`, so a typoed address can't brick
    /// governance. Admin-only.
    pub fn propose_admin(env: Env, caller: Address, new_admin: Address) {
        caller.require_auth();
        if caller != Self::admin_address(&env) {
            panic_with_error!(&env, Error::NotAdmin);
        }

        env.storage()
            .instance()
            .set(&DataKey::PendingAdmin, &new_admin);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        AdminTransferProposed {
            current_admin: caller,
            pending_admin: new_admin,
        }
        .publish(&env);
    }

    /// Completes a two-step admin transfer. Callable only by the address
    /// currently proposed as the pending admin.
    pub fn accept_admin(env: Env, caller: Address) {
        caller.require_auth();

        let pending: Address = env
            .storage()
            .instance()
            .get(&DataKey::PendingAdmin)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NoPendingAdminProposal));
        if caller != pending {
            panic_with_error!(&env, Error::NotPendingAdmin);
        }

        env.storage().instance().set(&DataKey::Admin, &caller);
        env.storage().instance().remove(&DataKey::PendingAdmin);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        AdminTransferAccepted { new_admin: caller }.publish(&env);
    }

    /// Abandons a pending admin transfer, leaving the current admin in
    /// place. Admin-only.
    pub fn cancel_admin_transfer(env: Env, caller: Address) {
        caller.require_auth();
        if caller != Self::admin_address(&env) {
            panic_with_error!(&env, Error::NotAdmin);
        }
        if !env.storage().instance().has(&DataKey::PendingAdmin) {
            panic_with_error!(&env, Error::NoPendingAdminProposal);
        }

        env.storage().instance().remove(&DataKey::PendingAdmin);
        AdminTransferCancelled { admin: caller }.publish(&env);
    }

    pub fn get_admin(env: Env) -> Address {
        Self::admin_address(&env)
    }

    pub fn get_pending_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::PendingAdmin)
    }

    pub fn get_fee_bps(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::FeeBps).unwrap_or(0)
    }

    pub fn get_fee_collector(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::FeeCollector)
    }

    pub fn get_fee_balance(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::FeeBalance)
            .unwrap_or(0)
    }

    /// Computes `(fee, net)` for `amount` at `bps` without touching storage.
    /// Exposed read-only so off-chain callers (SDKs, indexers, tests) can
    /// preview the exact split the contract will apply.
    pub fn preview_fee(env: Env, amount: i128, bps: u32) -> (i128, i128) {
        let fee = Self::fee_for(&env, amount, bps);
        (fee, amount - fee)
    }

    /// `floor(amount * bps / BPS_DENOMINATOR)`, checked against i128 overflow.
    /// Since callers only ever pass a stored `bps <= MAX_FEE_BPS`, the result
    /// is always `<= amount`, so `amount - fee` never underflows.
    fn fee_for(env: &Env, amount: i128, bps: u32) -> i128 {
        if bps == 0 {
            return 0;
        }
        amount
            .checked_mul(bps as i128)
            .unwrap_or_else(|| panic_with_error!(env, Error::FeeOverflow))
            .checked_div(BPS_DENOMINATOR)
            .unwrap_or_else(|| panic_with_error!(env, Error::FeeOverflow))
    }

    fn admin_address(env: &Env) -> Address {
        match env.storage().instance().get(&DataKey::Admin) {
            Some(admin) => admin,
            None => panic_with_error!(env, Error::NotInitialized),
        }
    }

    fn token_address(env: &Env) -> Address {
        match env.storage().instance().get(&DataKey::Token) {
            Some(token) => token,
            None => panic_with_error!(env, Error::NotInitialized),
        }
    }
}
