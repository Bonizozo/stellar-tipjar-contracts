#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Env, MuxedAddress, Vec,
};

#[cfg(test)]
mod test;
#[cfg(test)]
mod test_exhaustive;
#[cfg(test)]
mod test_multitoken;

/// Ledger TTL bump applied to instance and persistent storage on every write.
const LEDGER_THRESHOLD: u32 = 100_000;
const LEDGER_BUMP: u32 = 120_960; // ~7 days at 5s/ledger

const PAYOUT_DELAY_LEDGERS: u32 = 17280; // ~1 day at 5s/ledger

/// Current data version for migration tracking.
const CURRENT_DATA_VERSION: u32 = 2;
const V1_DATA_VERSION: u32 = 1;

/// Maximum number of tokens that can be in the allowlist.
const MAX_ALLOWED_TOKENS: u32 = 50;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Legacy: Address of the SEP-41 token this jar accepts (v1 only).
    Token,
    /// Legacy: Withdrawable balance escrowed for a creator (v1 only).
    CreatorBalance(Address),
    /// Legacy: Historical total ever tipped to a creator (v1 only).
    CreatorTotal(Address),
    /// Current data version for migration tracking.
    DataVersion,
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
    TokenNotAllowed = 11,
    TokenAlreadyExists = 12,
    MaxTokensReached = 13,
    Unauthorized = 14,
}

#[contract]
pub struct TipJar;

#[contractimpl]
impl TipJar {
    /// One-time configuration of the token this jar accepts. Errors if called twice.
    /// For v2: initializes with a single token in the allowlist.
    pub fn init(env: Env, token: Address) {
        if env.storage().instance().has(&DataKey::DataVersion) {
            panic_with_error!(&env, Error::AlreadyInitialized);
        }
        
        // Initialize v2 directly
        env.storage()
            .instance()
            .set(&DataKey::DataVersion, &CURRENT_DATA_VERSION);
        
        let mut allowed_tokens = Vec::new(&env);
        allowed_tokens.push_back(token);
        env.storage()
            .instance()
            .set(&DataKey::AllowedTokens, &allowed_tokens);
            
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
    }

    /// Escrows `amount` of the specified `token` from `sender` for `creator`.
    pub fn tip(env: Env, sender: Address, creator: Address, token: Address, amount: i128) {
        sender.require_auth();

        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }

        Self::ensure_initialized(&env);
        Self::ensure_token_allowed(&env, &token);

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

        Self::ensure_initialized(&env);

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

        Self::ensure_initialized(&env);

        let mut allowed_tokens: Vec<Address> = env
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

    // Legacy method for backward compatibility (v1 contracts)
    pub fn token_address(env: Env) -> Address {
        match env.storage().instance().get(&DataKey::Token) {
            Some(token) => token,
            None => panic_with_error!(&env, Error::NotInitialized),
        }
    }

    // Legacy methods for backward compatibility with v1 API
    // These use the first token in the allowlist as the default token
    pub fn tip_legacy(env: Env, sender: Address, creator: Address, amount: i128) {
        sender.require_auth();
        
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
        let version: u32 = env
            .storage()
            .instance()
            .get(&DataKey::DataVersion)
            .unwrap_or(V1_DATA_VERSION);
        
        if version < V1_DATA_VERSION {
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

    /// Lazy migration: Migrates v1 creator data to v2 format on first access.
    fn maybe_migrate_creator_data(env: &Env, creator: &Address, token: &Address) {
        let version: u32 = env
            .storage()
            .instance()
            .get(&DataKey::DataVersion)
            .unwrap_or(V1_DATA_VERSION);

        if version == V1_DATA_VERSION {
            // Check if we have v1 data for this creator
            let v1_balance_key = DataKey::CreatorBalance(creator.clone());
            let v1_total_key = DataKey::CreatorTotal(creator.clone());
            let legacy_token_key = DataKey::Token;

            if let Some(legacy_token) = env.storage().instance().get::<_, Address>(&legacy_token_key) {
                if &legacy_token == token {
                    // Migrate v1 data to v2 format
                    if let Some(v1_balance) = env.storage().persistent().get::<_, i128>(&v1_balance_key) {
                        let v2_balance_key = DataKey::Balance(creator.clone(), token.clone());
                        env.storage().persistent().set(&v2_balance_key, &v1_balance);
                        env.storage().persistent().extend_ttl(
                            &v2_balance_key,
                            LEDGER_THRESHOLD,
                            LEDGER_BUMP,
                        );
                        env.storage().persistent().remove(&v1_balance_key);
                    }

                    if let Some(v1_total) = env.storage().persistent().get::<_, i128>(&v1_total_key) {
                        let v2_total_key = DataKey::Total(creator.clone(), token.clone());
                        env.storage().persistent().set(&v2_total_key, &v1_total);
                        env.storage().persistent().extend_ttl(
                            &v2_total_key,
                            LEDGER_THRESHOLD,
                            LEDGER_BUMP,
                        );
                        env.storage().persistent().remove(&v1_total_key);
                    }
                }
            }

            // Upgrade to v2 and initialize allowlist with legacy token
            if let Some(legacy_token) = env.storage().instance().get::<_, Address>(&legacy_token_key) {
                let mut allowed_tokens = Vec::new(env);
                allowed_tokens.push_back(legacy_token);
                env.storage()
                    .instance()
                    .set(&DataKey::AllowedTokens, &allowed_tokens);
            }

            env.storage()
                .instance()
                .set(&DataKey::DataVersion, &CURRENT_DATA_VERSION);
            env.storage()
                .instance()
                .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
        }
    }
}
