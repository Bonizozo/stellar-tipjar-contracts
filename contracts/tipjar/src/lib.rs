#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Env, MuxedAddress,
};

#[cfg(test)]
mod test;
#[cfg(test)]
mod test_exhaustive;

/// Ledger TTL bump applied to instance and persistent storage on every write.
const LEDGER_THRESHOLD: u32 = 100_000;
const LEDGER_BUMP: u32 = 120_960; // ~7 days at 5s/ledger

const PAYOUT_DELAY_LEDGERS: u32 = 17280; // ~1 day at 5s/ledger

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
}

#[contract]
pub struct TipJar;

#[contractimpl]
impl TipJar {
    /// One-time configuration of the token this jar accepts. Errors if called twice.
    pub fn init(env: Env, token: Address) {
        if env.storage().instance().has(&DataKey::Token) {
            panic_with_error!(&env, Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Token, &token);
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
    pub fn withdraw(env: Env, caller: Address, creator: Address, to: Address, amount: Option<i128>) {
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
                        env.storage().persistent().set(&operator_key, &(new_allowance, expiry_ledger));
                        env.storage().persistent().extend_ttl(&operator_key, LEDGER_THRESHOLD, LEDGER_BUMP);
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
        if let Some((new_payout, effective_ledger)) = env.storage().persistent().get::<_, (Address, u32)>(&pending_key) {
            if env.ledger().sequence() >= effective_ledger {
                env.storage().persistent().set(&payout_key, &new_payout);
                env.storage().persistent().extend_ttl(&payout_key, LEDGER_THRESHOLD, LEDGER_BUMP);
                env.storage().persistent().remove(&pending_key);
                PayoutChangeApplied {
                    creator: creator.clone(),
                    new_payout,
                }.publish(&env);
            }
        }

        let payout_address: Address = env.storage().persistent().get(&payout_key).unwrap_or(creator.clone());
        if env.storage().persistent().has(&payout_key) {
            env.storage().persistent().extend_ttl(&payout_key, LEDGER_THRESHOLD, LEDGER_BUMP);
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
        env.storage().persistent().set(&key, &(payout.clone(), effective_ledger));
        env.storage().persistent().extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP);
        env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
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
        env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
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

    fn token_address(env: &Env) -> Address {
        match env.storage().instance().get(&DataKey::Token) {
            Some(token) => token,
            None => panic_with_error!(env, Error::NotInitialized),
        }
    }
}
