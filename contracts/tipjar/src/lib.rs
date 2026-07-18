#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Env, MuxedAddress,
};

#[cfg(test)]
mod test;

/// Ledger TTL bump applied to instance and persistent storage on every write.
const LEDGER_THRESHOLD: u32 = 100_000;
const LEDGER_BUMP: u32 = 120_960; // ~7 days at 5s/ledger

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Address of the SEP-41 token this jar accepts.
    Token,
    /// Withdrawable balance escrowed for a creator.
    CreatorBalance(Address),
    /// Historical total ever tipped to a creator (never decreases).
    CreatorTotal(Address),
}

/// Topics `("tip", creator)`, data `(sender, amount)`.
#[contractevent(data_format = "vec")]
pub struct Tip {
    #[topic]
    creator: Address,
    sender: Address,
    amount: i128,
}

/// Topics `("withdraw", creator)`, data `[amount]`.
#[contractevent(data_format = "vec")]
pub struct Withdraw {
    #[topic]
    creator: Address,
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

    /// Pays out a creator's full withdrawable balance and resets it to zero.
    /// Historical totals are left untouched.
    pub fn withdraw(env: Env, creator: Address) {
        creator.require_auth();

        let balance_key = DataKey::CreatorBalance(creator.clone());
        let balance: i128 = env.storage().persistent().get(&balance_key).unwrap_or(0);

        if balance == 0 {
            panic_with_error!(&env, Error::NothingToWithdraw);
        }

        let token = Self::token_address(&env);
        let contract_address = env.current_contract_address();

        token::TokenClient::new(&env, &token).transfer(
            &contract_address,
            MuxedAddress::from(creator.clone()),
            &balance,
        );

        env.storage().persistent().set(&balance_key, &0i128);
        env.storage()
            .persistent()
            .extend_ttl(&balance_key, LEDGER_THRESHOLD, LEDGER_BUMP);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        Withdraw {
            creator,
            amount: balance,
        }
        .publish(&env);
    }

    fn token_address(env: &Env) -> Address {
        match env.storage().instance().get(&DataKey::Token) {
            Some(token) => token,
            None => panic_with_error!(env, Error::NotInitialized),
        }
    }
}
