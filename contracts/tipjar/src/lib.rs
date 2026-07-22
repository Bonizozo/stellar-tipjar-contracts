#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, BytesN, Env, MuxedAddress,
};

#[cfg(test)]
mod test;
#[cfg(test)]
mod test_exhaustive;
#[cfg(test)]
mod test_upgrade;

/// Ledger TTL bump applied to instance and persistent storage on every write.
const LEDGER_THRESHOLD: u32 = 100_000;
const LEDGER_BUMP: u32 = 120_960; // ~7 days at 5s/ledger

const PAYOUT_DELAY_LEDGERS: u32 = 17280; // ~1 day at 5s/ledger

/// Storage schema version this build of the contract expects. `migrate()`
/// advances `DataKey::DataVersion` towards this value; a build whose
/// `DATA_VERSION` is already met treats `migrate()` as a no-op.
const DATA_VERSION: u32 = 1;

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
    /// Address authorized to propose/cancel upgrades and admin transfers.
    Admin,
    /// Two-step admin transfer: address that may call `accept_admin`.
    PendingAdmin,
    /// Ledger delay enforced between `propose_upgrade` and `execute_upgrade`, set at `init`.
    UpgradeTimelockLedgers,
    /// Pending upgrade proposal: (new_wasm_hash, unlock_ledger)
    PendingUpgrade,
    /// Storage schema version, advanced by `migrate()` after an upgrade.
    DataVersion,
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
}

#[contract]
pub struct TipJar;

#[contractimpl]
impl TipJar {
    /// One-time configuration of the token this jar accepts, the upgrade
    /// admin, and the ledger delay `execute_upgrade` must wait out after a
    /// `propose_upgrade`. Errors if called twice.
    pub fn init(env: Env, token: Address, admin: Address, upgrade_timelock_ledgers: u32) {
        if env.storage().instance().has(&DataKey::Token) {
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
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
    }

    /// Escrows `amount` of the configured token from `sender` for `creator`.
    pub fn tip(env: Env, sender: Address, creator: Address, amount: i128) {
        sender.require_auth();

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

    /// Current admin address.
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotInitialized))
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

    fn require_admin(env: &Env, caller: &Address) {
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, Error::NotInitialized));
        if *caller != stored {
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
