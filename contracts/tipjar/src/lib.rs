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

/// Default lifetime of a guardian-initiated pause before it auto-expires,
/// unless the admin confirms it into a persistent pause first. Overridable
/// via `set_guardian_pause_duration`. ~1 day at 5s/ledger.
const DEFAULT_GUARDIAN_PAUSE_DURATION_LEDGERS: u32 = 17280;

/// Bitflag: blocks `tip`.
pub const PAUSE_FLAG_TIPS: u32 = 1 << 0;
/// Bitflag: blocks `withdraw` and the withdrawal-mechanics entrypoints
/// (`set_payout_address`, `cancel_payout_address`, `authorize_operator`,
/// `revoke_operator`).
pub const PAUSE_FLAG_WITHDRAWALS: u32 = 1 << 1;
/// Compound flag: both of the above.
pub const PAUSE_FLAG_ALL: u32 = PAUSE_FLAG_TIPS | PAUSE_FLAG_WITHDRAWALS;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Address of the SEP-41 token this jar accepts.
    Token,
    /// Withdrawable balance escrowed for a creator.
    CreatorBalance(Address),
    /// Historical total ever tipped to a creator (never decreases).
    CreatorTotal(Address),
    /// Payout address designated for a creator.
    PayoutAddress(Address),
    /// Pending change to payout address: (creator) -> (new_payout, effective_ledger)
    PendingPayoutChange(Address),
    /// Operator delegation: (creator, operator) -> (allowance, expiry_ledger)
    Operator(Address, Address),
    /// Sole admin address: can unpause and can pause persistently.
    Admin,
    /// Sole guardian address, settable by admin: can pause instantly but
    /// never unpause. Absent until `set_guardian` is called.
    Guardian,
    /// Circuit-breaker state, see `PauseState`.
    Pause,
    /// Configurable ledger duration for guardian-initiated pauses.
    GuardianPauseDuration,
}

/// Circuit-breaker state, stored as a single instance-storage entry.
///
/// `admin_flags` and `guardian_flags` are independent bitmasks over
/// `PAUSE_FLAG_*` rather than separate booleans, so tips/withdrawals pause
/// independently. They're kept in two buckets (rather than one shared
/// bitmask) so a guardian's temporary pause can never silently overwrite, or
/// be silently promoted into, an admin's deliberate persistent pause:
/// - `admin_flags` bits are set only by the admin and never auto-expire;
///   only an admin `unpause_*` call clears them.
/// - `guardian_flags` bits are set only by the guardian and auto-expire at
///   `guardian_expiry` (a single shared ledger checkpoint) unless the admin
///   confirms them first by calling the matching `pause_*`, which promotes
///   them into `admin_flags` and clears them here.
#[contracttype]
#[derive(Clone)]
pub struct PauseState {
    pub admin_flags: u32,
    pub guardian_flags: u32,
    pub guardian_expiry: u32,
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

/// Topics `("paused", by)`, data `[flags]`.
#[contractevent(data_format = "vec")]
pub struct Paused {
    #[topic]
    by: Address,
    flags: u32,
}

/// Topics `("unpaused", by)`, data `[flags]`.
#[contractevent(data_format = "vec")]
pub struct Unpaused {
    #[topic]
    by: Address,
    flags: u32,
}

#[contractevent(data_format = "vec")]
pub struct GuardianUpdated {
    #[topic]
    admin: Address,
    guardian: Address,
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
    Unauthorized = 11,
    TipsPaused = 12,
    WithdrawalsPaused = 13,
    InvalidDuration = 14,
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
        admin.require_auth();
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
    }

    /// Escrows `amount` of the configured token from `sender` for `creator`.
    pub fn tip(env: Env, sender: Address, creator: Address, amount: i128) {
        sender.require_auth();
        Self::check_not_paused(&env, PAUSE_FLAG_TIPS);

        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }

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
            .checked_add(amount)
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
        Self::check_not_paused(&env, PAUSE_FLAG_WITHDRAWALS);

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
        Self::check_not_paused(&env, PAUSE_FLAG_WITHDRAWALS);
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
        Self::check_not_paused(&env, PAUSE_FLAG_WITHDRAWALS);
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
        Self::check_not_paused(&env, PAUSE_FLAG_WITHDRAWALS);
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
        Self::check_not_paused(&env, PAUSE_FLAG_WITHDRAWALS);
        let key = DataKey::Operator(creator.clone(), operator.clone());
        env.storage().persistent().remove(&key);
        OperatorRevoked { creator, operator }.publish(&env);
    }

    // ── circuit breaker ─────────────────────────────────────────────────

    /// Appoints (or replaces) the guardian. Admin only.
    pub fn set_guardian(env: Env, admin: Address, guardian: Address) {
        admin.require_auth();
        Self::require_admin(&env, &admin);
        env.storage().instance().set(&DataKey::Guardian, &guardian);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
        GuardianUpdated { admin, guardian }.publish(&env);
    }

    /// Configures how many ledgers a guardian-initiated pause lasts before
    /// auto-expiring. Admin only.
    pub fn set_guardian_pause_duration(env: Env, admin: Address, ledgers: u32) {
        admin.require_auth();
        Self::require_admin(&env, &admin);
        if ledgers == 0 {
            panic_with_error!(&env, Error::InvalidDuration);
        }
        env.storage()
            .instance()
            .set(&DataKey::GuardianPauseDuration, &ledgers);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
    }

    /// Pauses `tip`. Callable by admin (persists until explicitly unpaused)
    /// or guardian (auto-expires; call again as admin to confirm/persist it).
    pub fn pause_tips(env: Env, caller: Address) {
        Self::pause_internal(env, caller, PAUSE_FLAG_TIPS);
    }

    /// Pauses `withdraw` and the withdrawal-mechanics entrypoints. See `pause_tips`.
    pub fn pause_withdrawals(env: Env, caller: Address) {
        Self::pause_internal(env, caller, PAUSE_FLAG_WITHDRAWALS);
    }

    /// Pauses both tips and withdrawals in one call. See `pause_tips`.
    pub fn pause_all(env: Env, caller: Address) {
        Self::pause_internal(env, caller, PAUSE_FLAG_ALL);
    }

    /// Unpauses `tip`. Admin only — guardians can pause but never unpause.
    pub fn unpause_tips(env: Env, caller: Address) {
        Self::unpause_internal(env, caller, PAUSE_FLAG_TIPS);
    }

    /// Unpauses `withdraw` and the withdrawal-mechanics entrypoints. Admin only.
    pub fn unpause_withdrawals(env: Env, caller: Address) {
        Self::unpause_internal(env, caller, PAUSE_FLAG_WITHDRAWALS);
    }

    /// Unpauses both tips and withdrawals in one call. Admin only.
    pub fn unpause_all(env: Env, caller: Address) {
        Self::unpause_internal(env, caller, PAUSE_FLAG_ALL);
    }

    /// True if every bit in `flag` is currently paused (accounting for
    /// guardian auto-expiry).
    pub fn is_feature_paused(env: Env, flag: u32) -> bool {
        let state = Self::pause_state(&env);
        let effective = Self::effective_pause_flags(&env, &state);
        flag != 0 && (effective & flag) == flag
    }

    /// The currently effective pause bitmask (accounting for guardian auto-expiry).
    pub fn pause_flags(env: Env) -> u32 {
        let state = Self::pause_state(&env);
        Self::effective_pause_flags(&env, &state)
    }

    /// Ledger sequence at which the current guardian-originated pause bits
    /// expire. 0 if no guardian pause is active.
    pub fn guardian_pause_expiry_ledger(env: Env) -> u32 {
        Self::pause_state(&env).guardian_expiry
    }

    pub fn get_admin(env: Env) -> Address {
        Self::admin_address(&env)
    }

    pub fn get_guardian(env: Env) -> Option<Address> {
        Self::guardian_address(&env)
    }

    fn pause_internal(env: Env, caller: Address, flag: u32) {
        caller.require_auth();
        let admin = Self::admin_address(&env);
        let guardian = Self::guardian_address(&env);
        let mut state = Self::pause_state(&env);

        if caller == admin {
            // Persistent: never auto-expires. Also confirms/promotes any
            // matching guardian-originated bits, so they stop being subject
            // to expiry.
            state.admin_flags |= flag;
            state.guardian_flags &= !flag;
        } else if guardian == Some(caller.clone()) {
            state.guardian_flags |= flag;
            state.guardian_expiry = env.ledger().sequence() + Self::guardian_pause_duration(&env);
        } else {
            panic_with_error!(&env, Error::Unauthorized);
        }

        env.storage().instance().set(&DataKey::Pause, &state);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
        Paused {
            by: caller,
            flags: flag,
        }
        .publish(&env);
    }

    fn unpause_internal(env: Env, caller: Address, flag: u32) {
        caller.require_auth();
        let admin = Self::admin_address(&env);
        if caller != admin {
            panic_with_error!(&env, Error::Unauthorized);
        }

        let mut state = Self::pause_state(&env);
        state.admin_flags &= !flag;
        state.guardian_flags &= !flag;
        env.storage().instance().set(&DataKey::Pause, &state);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
        Unpaused {
            by: caller,
            flags: flag,
        }
        .publish(&env);
    }

    /// Panics with the flag-specific typed error if `flag` is currently paused.
    /// Must be called before any token transfer or storage write in a gated entrypoint.
    fn check_not_paused(env: &Env, flag: u32) {
        let state = Self::pause_state(env);
        if Self::effective_pause_flags(env, &state) & flag != 0 {
            if flag == PAUSE_FLAG_TIPS {
                panic_with_error!(env, Error::TipsPaused);
            } else {
                panic_with_error!(env, Error::WithdrawalsPaused);
            }
        }
    }

    fn effective_pause_flags(env: &Env, state: &PauseState) -> u32 {
        let mut effective = state.admin_flags;
        if state.guardian_flags != 0 && env.ledger().sequence() < state.guardian_expiry {
            effective |= state.guardian_flags;
        }
        effective
    }

    fn pause_state(env: &Env) -> PauseState {
        env.storage()
            .instance()
            .get(&DataKey::Pause)
            .unwrap_or(PauseState {
                admin_flags: 0,
                guardian_flags: 0,
                guardian_expiry: 0,
            })
    }

    fn guardian_pause_duration(env: &Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::GuardianPauseDuration)
            .unwrap_or(DEFAULT_GUARDIAN_PAUSE_DURATION_LEDGERS)
    }

    fn admin_address(env: &Env) -> Address {
        match env.storage().instance().get(&DataKey::Admin) {
            Some(admin) => admin,
            None => panic_with_error!(env, Error::NotInitialized),
        }
    }

    fn guardian_address(env: &Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Guardian)
    }

    fn require_admin(env: &Env, caller: &Address) {
        if *caller != Self::admin_address(env) {
            panic_with_error!(env, Error::Unauthorized);
        }
    }

    fn token_address(env: &Env) -> Address {
        match env.storage().instance().get(&DataKey::Token) {
            Some(token) => token,
            None => panic_with_error!(env, Error::NotInitialized),
        }
    }
}
