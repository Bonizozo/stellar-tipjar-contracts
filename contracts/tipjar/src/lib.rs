#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, BytesN, Env, MuxedAddress, Vec,
};

#[cfg(test)]
mod test;
#[cfg(test)]
mod test_coverage_gaps;
#[cfg(test)]
mod test_exhaustive;
#[cfg(test)]
mod test_invariants;
#[cfg(test)]
mod test_upgrade;

/// Ledger TTL bump applied to instance and persistent storage on every write.
const LEDGER_THRESHOLD: u32 = 100_000;
const LEDGER_BUMP: u32 = 120_960; // ~7 days at 5s/ledger

const PAYOUT_DELAY_LEDGERS: u32 = 17280; // ~1 day at 5s/ledger

/// Basis-point denominator for fee math (1 bps = 1/10_000).
const BPS_DENOMINATOR: i128 = 10_000;
/// Hard on-chain ceiling for the protocol fee: 1_000 bps = 10%.
const MAX_FEE_BPS: u32 = 1_000;

/// Storage schema version this build of the contract expects. `migrate()`
/// advances `DataKey::DataVersion` towards this value; a build whose
/// `DATA_VERSION` is already met treats `migrate()` as a no-op.
///
/// Unrelated to multi-token migration (see `ensure_token_allowed`/
/// `maybe_migrate_creator_data`), which is keyed off the presence of
/// `DataKey::AllowedTokens` / `DataKey::Token`, not this version number.
const DATA_VERSION: u32 = 1;

/// Maximum number of tokens that can be in the multi-token allowlist.
const MAX_ALLOWED_TOKENS: u32 = 50;

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
    /// Legacy: Address of the SEP-41 token this jar accepts (v1 only).
    Token,
    /// Legacy: Withdrawable balance escrowed for a creator (v1 only).
    CreatorBalance(Address),
    /// Historical total ever tipped to a creator (never decreases). Tracks
    /// the gross amount tipped, before any protocol fee is deducted.
    CreatorTotal(Address),
    /// Token allowlist for multi-token support.
    AllowedTokens,
    /// Withdrawable balance escrowed for a (creator, token) pair.
    Balance(Address, Address),
    /// Historical total ever tipped to a (creator, token) pair.
    Total(Address, Address),
    /// Payout address designated for a creator.
    PayoutAddress(Address),
    /// Pending change to payout address: (creator) -> (new_payout, effective_ledger)
    PendingPayoutChange(Address),
    /// Operator delegation: (creator, operator) -> (allowance, expiry_ledger)
    Operator(Address, Address),
    /// Address authorized to propose/cancel upgrades and admin transfers, to
    /// manage the circuit breaker (pause/guardian), and to configure the
    /// protocol fee.
    Admin,
    /// Two-step admin transfer: address that may call `accept_admin`.
    PendingAdmin,
    /// Ledger delay enforced between `propose_upgrade` and `execute_upgrade`, set at `init`.
    UpgradeTimelockLedgers,
    /// Pending upgrade proposal: (new_wasm_hash, unlock_ledger)
    PendingUpgrade,
    /// Storage schema version, advanced by `migrate()` after an upgrade.
    DataVersion,
    /// Sole guardian address, settable by admin: can pause instantly but
    /// never unpause. Absent until `set_guardian` is called.
    Guardian,
    /// Circuit-breaker state, see `PauseState`.
    Pause,
    /// Configurable ledger duration for guardian-initiated pauses.
    GuardianPauseDuration,
    /// Protocol fee rate in basis points. Absent or 0 means no fee.
    FeeBps,
    /// Address authorized to withdraw accrued protocol fees.
    FeeCollector,
    /// Legacy unparameterized protocol fee counter. Kept so existing
    /// persistent entries remain readable after upgrade; migrated into
    /// `FeeBalance(token)` for the primary token on first fee access.
    FeeBalance,
    /// Withdrawable protocol fee balance accrued from tips in `token`.
    /// Named to match `Balance`/`Total` (keyed by token) without colliding
    /// with the legacy unit `FeeBalance` variant required for migration.
    FeeBalanceToken(Address),
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

/// Topics `("tip", creator)`, data `(token, sender, amount)`.
#[contractevent(data_format = "vec")]
pub struct Tip {
    #[topic]
    creator: Address,
    token: Address,
    sender: Address,
    amount: i128,
}

/// Topics `("withdraw", creator)`, data `[token, amount, to]`.
#[contractevent(data_format = "vec")]
pub struct Withdraw {
    #[topic]
    creator: Address,
    token: Address,
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

/// Topics `("admin_transfer_proposed",)`, data `(current_admin, new_admin)`.
#[contractevent(data_format = "vec")]
pub struct AdminTransferProposed {
    current_admin: Address,
    new_admin: Address,
}

/// Topics `("admin_transfer_accepted",)`, data `(new_admin,)`.
#[contractevent(data_format = "vec")]
pub struct AdminTransferAccepted {
    new_admin: Address,
}

/// Topics `("admin_transfer_cancelled", admin)`, data `[]`.
#[contractevent(data_format = "vec")]
pub struct AdminTransferCancelled {
    #[topic]
    admin: Address,
}

/// Topics `("upgrade_proposed", hash)`, data `(unlock_ledger,)`.
#[contractevent(data_format = "vec")]
pub struct UpgradeProposed {
    #[topic]
    hash: BytesN<32>,
    unlock_ledger: u32,
}

/// Topics `("upgrade_executed", hash)`, data `()`.
#[contractevent(data_format = "vec")]
pub struct UpgradeExecuted {
    #[topic]
    hash: BytesN<32>,
}

/// Topics `("upgrade_cancelled", hash)`, data `()`.
#[contractevent(data_format = "vec")]
pub struct UpgradeCancelled {
    #[topic]
    hash: BytesN<32>,
}

/// Topics `("migrated",)`, data `(from_version, to_version)`.
#[contractevent(data_format = "vec")]
pub struct Migrated {
    from_version: u32,
    to_version: u32,
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
    NoPendingAdmin = 12,
    InvalidTimelock = 13,
    UpgradeAlreadyPending = 14,
    NoPendingUpgrade = 15,
    TimelockNotElapsed = 16,
    TipsPaused = 17,
    WithdrawalsPaused = 18,
    InvalidDuration = 19,
    InvalidFee = 20,
    FeeOverflow = 21,
    NotFeeCollector = 22,
    TokenNotAllowed = 23,
    TokenAlreadyExists = 24,
    MaxTokensReached = 25,
}

#[contract]
pub struct TipJar;

#[contractimpl]
impl TipJar {
    /// One-time configuration: the token this jar accepts (seeded as the
    /// first entry of the multi-token allowlist), the admin, and the ledger
    /// delay `execute_upgrade` must wait out after a `propose_upgrade`.
    /// Errors if called twice.
    pub fn init(env: Env, token: Address, admin: Address, upgrade_timelock_ledgers: u32) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, Error::AlreadyInitialized);
        }
        if upgrade_timelock_ledgers == 0 {
            panic_with_error!(&env, Error::InvalidTimelock);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::UpgradeTimelockLedgers, &upgrade_timelock_ledgers);
        env.storage()
            .instance()
            .set(&DataKey::DataVersion, &DATA_VERSION);

        let mut allowed_tokens = Vec::new(&env);
        allowed_tokens.push_back(token);
        env.storage()
            .instance()
            .set(&DataKey::AllowedTokens, &allowed_tokens);

        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
    }

    /// Escrows `amount` of the configured token from `sender` for `creator`,
    /// less the protocol fee (if one is configured). The creator's balance is
    /// credited with `amount - fee`; the fee itself accrues to
    /// `FeeBalance(token)` for later withdrawal by the fee collector.
    /// `fee + net == amount` holds for every input.
    pub fn tip(env: Env, sender: Address, creator: Address, token: Address, amount: i128) {
        sender.require_auth();
        Self::check_not_paused(&env, PAUSE_FLAG_TIPS);

        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }

        Self::ensure_initialized(&env);
        Self::ensure_token_allowed(&env, &token);
        Self::maybe_migrate_creator_data(&env, &creator, &token);

        let fee_bps: u32 = env.storage().instance().get(&DataKey::FeeBps).unwrap_or(0);
        let fee = Self::fee_for(&env, amount, fee_bps);
        let net_amount = amount - fee;

        let contract_address = env.current_contract_address();

        token::TokenClient::new(&env, &token).transfer(
            &sender,
            MuxedAddress::from(contract_address),
            &amount,
        );

        let balance_key = DataKey::Balance(creator.clone(), token.clone());
        let total_key = DataKey::Total(creator.clone(), token.clone());

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
            Self::maybe_migrate_fee_balance(&env, &token);
            let fee_balance_key = DataKey::FeeBalanceToken(token.clone());
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
            token,
            sender,
            amount,
        }
        .publish(&env);
    }

    /// Historical total ever tipped to `creator` for a specific `token`. Zero if never tipped.
    pub fn get_total_tips(env: Env, creator: Address, token: Address) -> i128 {
        Self::maybe_migrate_creator_data(&env, &creator, &token);
        env.storage()
            .persistent()
            .get(&DataKey::Total(creator, token))
            .unwrap_or(0)
    }

    /// Gets the withdrawable balance for a creator and specific token.
    pub fn get_balance(env: Env, creator: Address, token: Address) -> i128 {
        Self::maybe_migrate_creator_data(&env, &creator, &token);
        env.storage()
            .persistent()
            .get(&DataKey::Balance(creator, token))
            .unwrap_or(0)
    }

    /// Returns the list of allowed tokens.
    pub fn get_tokens(env: Env) -> Vec<Address> {
        Self::ensure_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::AllowedTokens)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Pays out a creator's withdrawable balance for a specific token.
    pub fn withdraw(
        env: Env,
        caller: Address,
        creator: Address,
        token: Address,
        to: Address,
        amount: Option<i128>,
    ) {
        caller.require_auth();
        Self::check_not_paused(&env, PAUSE_FLAG_WITHDRAWALS);

        Self::ensure_initialized(&env);
        Self::maybe_migrate_creator_data(&env, &creator, &token);

        let balance_key = DataKey::Balance(creator.clone(), token.clone());
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
            token,
            amount: amount_to_withdraw,
            to: payout_address,
        }
        .publish(&env);
    }

    /// Adds a token to the allowlist. Admin-only operation.
    pub fn add_token(env: Env, admin: Address, token: Address) {
        admin.require_auth();
        Self::require_admin(&env, &admin);

        let mut allowed_tokens: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AllowedTokens)
            .unwrap_or_else(|| Vec::new(&env));

        if allowed_tokens.len() >= MAX_ALLOWED_TOKENS {
            panic_with_error!(&env, Error::MaxTokensReached);
        }

        // Check if token already exists
        for existing_token in allowed_tokens.iter() {
            if existing_token == token {
                panic_with_error!(&env, Error::TokenAlreadyExists);
            }
        }

        allowed_tokens.push_back(token);
        env.storage()
            .instance()
            .set(&DataKey::AllowedTokens, &allowed_tokens);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
    }

    /// Removes a token from the allowlist. Admin-only operation.
    /// Existing balances remain withdrawable.
    pub fn remove_token(env: Env, admin: Address, token: Address) {
        admin.require_auth();
        Self::require_admin(&env, &admin);

        let allowed_tokens: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AllowedTokens)
            .unwrap_or_else(|| Vec::new(&env));

        let mut found = false;
        let mut new_tokens = Vec::new(&env);

        for existing_token in allowed_tokens.iter() {
            if existing_token != token {
                new_tokens.push_back(existing_token);
            } else {
                found = true;
            }
        }

        if !found {
            panic_with_error!(&env, Error::TokenNotAllowed);
        }

        env.storage()
            .instance()
            .set(&DataKey::AllowedTokens, &new_tokens);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
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

    /// Proposes `new_admin` as the next admin. Takes effect only once
    /// `new_admin` calls `accept_admin` — a single-step transfer to a typo'd
    /// or unreachable address can never permanently lock out administration.
    pub fn propose_admin(env: Env, admin: Address, new_admin: Address) {
        admin.require_auth();
        Self::require_admin(&env, &admin);

        env.storage()
            .instance()
            .set(&DataKey::PendingAdmin, &new_admin);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        AdminTransferProposed {
            current_admin: admin,
            new_admin,
        }
        .publish(&env);
    }

    /// Completes a two-step admin transfer. Must be called by the address
    /// named in the pending proposal.
    pub fn accept_admin(env: Env, new_admin: Address) {
        new_admin.require_auth();

        let pending: Address = env
            .storage()
            .instance()
            .get(&DataKey::PendingAdmin)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NoPendingAdmin));
        if pending != new_admin {
            panic_with_error!(&env, Error::NoPendingAdmin);
        }

        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.storage().instance().remove(&DataKey::PendingAdmin);

        AdminTransferAccepted { new_admin }.publish(&env);
    }

    /// Abandons a pending admin transfer, leaving the current admin in
    /// place. Admin-only.
    pub fn cancel_admin_transfer(env: Env, admin: Address) {
        admin.require_auth();
        Self::require_admin(&env, &admin);
        if !env.storage().instance().has(&DataKey::PendingAdmin) {
            panic_with_error!(&env, Error::NoPendingAdmin);
        }

        env.storage().instance().remove(&DataKey::PendingAdmin);
        AdminTransferCancelled { admin }.publish(&env);
    }

    /// Current admin address.
    pub fn get_admin(env: Env) -> Address {
        Self::admin_address(&env)
    }

    /// Address currently proposed as the next admin, if any.
    pub fn get_pending_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::PendingAdmin)
    }

    /// Admin-only. Records `new_wasm_hash` as a pending upgrade, unlocked
    /// after the ledger delay configured at `init`. Only one proposal may be
    /// pending at a time — cancel the existing one first to replace it.
    pub fn propose_upgrade(env: Env, admin: Address, new_wasm_hash: BytesN<32>) {
        admin.require_auth();
        Self::require_admin(&env, &admin);

        if env.storage().instance().has(&DataKey::PendingUpgrade) {
            panic_with_error!(&env, Error::UpgradeAlreadyPending);
        }

        let timelock: u32 = env
            .storage()
            .instance()
            .get(&DataKey::UpgradeTimelockLedgers)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotInitialized));
        let unlock_ledger = env.ledger().sequence() + timelock;

        env.storage().instance().set(
            &DataKey::PendingUpgrade,
            &(new_wasm_hash.clone(), unlock_ledger),
        );
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        UpgradeProposed {
            hash: new_wasm_hash,
            unlock_ledger,
        }
        .publish(&env);
    }

    /// Admin-only. Aborts a pending upgrade proposal without waiting out the
    /// timelock.
    pub fn cancel_upgrade(env: Env, admin: Address) {
        admin.require_auth();
        Self::require_admin(&env, &admin);

        let (hash, _): (BytesN<32>, u32) = env
            .storage()
            .instance()
            .get(&DataKey::PendingUpgrade)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NoPendingUpgrade));
        env.storage().instance().remove(&DataKey::PendingUpgrade);

        UpgradeCancelled { hash }.publish(&env);
    }

    /// Swaps this contract's WASM to the proposed hash once its timelock has
    /// elapsed. Permissionless by design — the admin already authorized the
    /// upgrade at `propose_upgrade`, and its unlock ledger is public
    /// on-chain state, so no caller identity check adds meaningful security
    /// here. Storage is preserved by the host across the swap; call the new
    /// WASM's `migrate()` afterwards to apply any storage-layout changes.
    pub fn execute_upgrade(env: Env) {
        let (hash, unlock_ledger): (BytesN<32>, u32) = env
            .storage()
            .instance()
            .get(&DataKey::PendingUpgrade)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NoPendingUpgrade));

        if env.ledger().sequence() < unlock_ledger {
            panic_with_error!(&env, Error::TimelockNotElapsed);
        }

        // Clear the proposal before the swap so a re-invocation of this same
        // function (impossible post-swap unless the new WASM redefines it,
        // but defensively) always fails closed with NoPendingUpgrade.
        env.storage().instance().remove(&DataKey::PendingUpgrade);

        env.deployer().update_current_contract_wasm(hash.clone());

        UpgradeExecuted { hash }.publish(&env);
    }

    /// Admin-only, idempotent. Advances `DataKey::DataVersion` towards this
    /// build's `DATA_VERSION`, applying any storage transformation the new
    /// WASM requires. A no-op (no panic, no event) if the stored version
    /// already meets or exceeds `DATA_VERSION` — safe to call more than once,
    /// including before the first upgrade or after a repeated invocation.
    pub fn migrate(env: Env, admin: Address) {
        admin.require_auth();
        Self::require_admin(&env, &admin);

        let current: u32 = env
            .storage()
            .instance()
            .get(&DataKey::DataVersion)
            .unwrap_or(1);
        if current >= DATA_VERSION {
            return;
        }

        // Storage-layout transformations for this version step would run
        // here, ahead of recording the new version below.
        env.storage()
            .instance()
            .set(&DataKey::DataVersion, &DATA_VERSION);

        Migrated {
            from_version: current,
            to_version: DATA_VERSION,
        }
        .publish(&env);
    }

    /// Current storage schema version.
    pub fn get_data_version(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::DataVersion)
            .unwrap_or(1)
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
        Self::require_admin(&env, &caller);

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

    // ── protocol fee ─────────────────────────────────────────────────────

    /// Sets the protocol fee rate and its collector. Admin-only; `bps` is
    /// hard-capped at `MAX_FEE_BPS`. Setting `bps` to 0 disables fees.
    pub fn set_fee(env: Env, admin: Address, bps: u32, collector: Address) {
        admin.require_auth();
        Self::require_admin(&env, &admin);
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
            admin,
            bps,
            collector,
        }
        .publish(&env);
    }

    /// Pays out the fee collector's full or partial share of
    /// `FeeBalance(token)`. Only moves units of `token`. Mirrors `withdraw`'s
    /// pattern, including TTL extension.
    pub fn withdraw_fees(env: Env, caller: Address, token: Address, amount: Option<i128>) {
        caller.require_auth();

        let collector: Address = env
            .storage()
            .instance()
            .get(&DataKey::FeeCollector)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NothingToWithdraw));
        if caller != collector {
            panic_with_error!(&env, Error::NotFeeCollector);
        }

        Self::maybe_migrate_fee_balance(&env, &token);

        let balance_key = DataKey::FeeBalanceToken(token.clone());
        let balance: i128 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        if balance == 0 {
            panic_with_error!(&env, Error::NothingToWithdraw);
        }

        let amount_to_withdraw = amount.unwrap_or(balance);
        if amount_to_withdraw <= 0 || amount_to_withdraw > balance {
            panic_with_error!(&env, Error::InvalidAmount);
        }

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

    pub fn get_fee_bps(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::FeeBps).unwrap_or(0)
    }

    pub fn get_fee_collector(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::FeeCollector)
    }

    pub fn get_fee_balance(env: Env, token: Address) -> i128 {
        Self::maybe_migrate_fee_balance(&env, &token);
        env.storage()
            .persistent()
            .get(&DataKey::FeeBalanceToken(token))
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
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, Error::NotInitialized))
    }

    fn guardian_address(env: &Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Guardian)
    }

    fn require_admin(env: &Env, caller: &Address) {
        if *caller != Self::admin_address(env) {
            panic_with_error!(env, Error::Unauthorized);
        }
    }

    // Legacy methods for backward compatibility with v1 API
    // These use the first token in the allowlist as the default token
    pub fn tip_legacy(env: Env, sender: Address, creator: Address, amount: i128) {
        let tokens = Self::get_tokens(env.clone());
        if tokens.is_empty() {
            panic_with_error!(&env, Error::NotInitialized);
        }
        let default_token = tokens.first().unwrap();

        Self::tip(env, sender, creator, default_token, amount);
    }

    pub fn get_total_tips_legacy(env: Env, creator: Address) -> i128 {
        let tokens = Self::get_tokens(env.clone());
        if tokens.is_empty() {
            return 0;
        }
        let default_token = tokens.first().unwrap();

        Self::get_total_tips(env, creator, default_token)
    }

    pub fn withdraw_legacy(
        env: Env,
        caller: Address,
        creator: Address,
        to: Address,
        amount: Option<i128>,
    ) {
        let tokens = Self::get_tokens(env.clone());
        if tokens.is_empty() {
            panic_with_error!(&env, Error::NotInitialized);
        }
        let default_token = tokens.first().unwrap();

        Self::withdraw(env, caller, creator, default_token, to, amount);
    }

    // Internal helper methods
    fn ensure_initialized(env: &Env) {
        if !env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(env, Error::NotInitialized);
        }
    }

    fn ensure_token_allowed(env: &Env, token: &Address) {
        let allowed_tokens: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AllowedTokens)
            .unwrap_or_else(|| Vec::new(env));

        for allowed_token in allowed_tokens.iter() {
            if &allowed_token == token {
                return;
            }
        }

        panic_with_error!(env, Error::TokenNotAllowed);
    }

    /// Lazy migration: moves a creator's legacy (pre-multi-token) balance and
    /// total into the (creator, token) format on first access under `token`,
    /// if legacy data for them exists and hasn't been migrated yet.
    fn maybe_migrate_creator_data(env: &Env, creator: &Address, token: &Address) {
        let legacy_token: Option<Address> = env.storage().instance().get(&DataKey::Token);
        let Some(legacy_token) = legacy_token else {
            return;
        };
        if &legacy_token != token {
            return;
        }

        let v1_balance_key = DataKey::CreatorBalance(creator.clone());
        let v1_total_key = DataKey::CreatorTotal(creator.clone());

        if let Some(v1_balance) = env.storage().persistent().get::<_, i128>(&v1_balance_key) {
            let v2_balance_key = DataKey::Balance(creator.clone(), token.clone());
            env.storage().persistent().set(&v2_balance_key, &v1_balance);
            env.storage()
                .persistent()
                .extend_ttl(&v2_balance_key, LEDGER_THRESHOLD, LEDGER_BUMP);
            env.storage().persistent().remove(&v1_balance_key);
        }

        if let Some(v1_total) = env.storage().persistent().get::<_, i128>(&v1_total_key) {
            let v2_total_key = DataKey::Total(creator.clone(), token.clone());
            env.storage().persistent().set(&v2_total_key, &v1_total);
            env.storage()
                .persistent()
                .extend_ttl(&v2_total_key, LEDGER_THRESHOLD, LEDGER_BUMP);
            env.storage().persistent().remove(&v1_total_key);
        }
    }

    /// Moves the unparameterized `FeeBalance` counter into
    /// `FeeBalanceToken(primary_token)` the first time that token is touched,
    /// so in-flight fees are not stranded after upgrade.
    fn maybe_migrate_fee_balance(env: &Env, token: &Address) {
        let legacy: Option<i128> = env.storage().persistent().get(&DataKey::FeeBalance);
        let Some(legacy) = legacy else {
            return;
        };
        let Some(primary): Option<Address> = env.storage().instance().get(&DataKey::Token) else {
            return;
        };
        if token != &primary {
            return;
        }

        let keyed = DataKey::FeeBalanceToken(token.clone());
        let current: i128 = env.storage().persistent().get(&keyed).unwrap_or(0);
        let combined = current
            .checked_add(legacy)
            .unwrap_or_else(|| panic_with_error!(env, Error::FeeOverflow));
        env.storage().persistent().set(&keyed, &combined);
        env.storage()
            .persistent()
            .extend_ttl(&keyed, LEDGER_THRESHOLD, LEDGER_BUMP);
        env.storage().persistent().remove(&DataKey::FeeBalance);
    }
}
