//! An adversarial token contract for testing security properties of the TipJar.
//!
//! This token contract can switch between multiple failure modes to verify that
//! TipJar correctly handles misbehaving token implementations:
//!
//! 1. **Panic on Transfer**: Reverts the entire transaction
//! 2. **Silent No-Op**: Returns success but doesn't actually move funds
//! 3. **Reentry Attack**: Attempts to re-enter tip/withdraw during transfer
//! 4. **Amount Burn**: Transfers amount - 1 (lossy transfer)

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, token, Address, Env,
    Symbol,
};

#[contracttype]
#[derive(Clone, Debug)]
pub struct AdversarialConfig {
    /// 0 = normal, 1 = panic, 2 = silent noop, 3 = reenter, 4 = burn
    pub failure_mode: u32,
    /// Reference to the tipjar contract for reentry attempts
    pub tipjar_contract: Address,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AdversarialError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    PanicOnTransfer = 3,
    ReentryAttempted = 4,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Balance {
    pub amount: i128,
}

#[derive(Clone)]
pub enum DataKey {
    Config,
    Balance(Address),
    Approved(Address, Address),
}

#[contract]
pub struct AdversarialTokenContract;

#[contractimpl]
impl AdversarialTokenContract {
    /// Initialize the adversarial token with a failure mode and tipjar reference.
    pub fn init(env: Env, admin: Address, failure_mode: u32, tipjar: Address) {
        let config_key = soroban_sdk::Symbol::short("config");
        if env.storage().instance().has(&config_key) {
            panic_with_error!(&env, AdversarialError::AlreadyInitialized);
        }

        let config = AdversarialConfig {
            failure_mode,
            tipjar_contract: tipjar,
        };
        env.storage().instance().set(&config_key, &config);

        // Admin gets initial balance
        let admin_key = soroban_sdk::Symbol::new(&env, &format!("bal:{}", admin));
        env.storage()
            .persistent()
            .set(&admin_key, &Balance { amount: i128::MAX / 2 });
    }

    /// Get current failure mode.
    pub fn get_failure_mode(env: Env) -> u32 {
        let config_key = soroban_sdk::Symbol::short("config");
        let config: AdversarialConfig = env
            .storage()
            .instance()
            .get(&config_key)
            .unwrap_or_else(|| panic_with_error!(&env, AdversarialError::NotInitialized));
        config.failure_mode
    }

    /// Set failure mode (admin only in real scenario, but unrestricted here for testing).
    pub fn set_failure_mode(env: Env, mode: u32) {
        let config_key = soroban_sdk::Symbol::short("config");
        let mut config: AdversarialConfig = env
            .storage()
            .instance()
            .get(&config_key)
            .unwrap_or_else(|| panic_with_error!(&env, AdversarialError::NotInitialized));
        config.failure_mode = mode;
        env.storage().instance().set(&config_key, &config);
    }

    /// Get balance of an address.
    pub fn balance(env: Env, address: Address) -> i128 {
        let key = format!("bal:{}", address);
        let key = soroban_sdk::Symbol::new(&env, &key);
        env.storage()
            .persistent()
            .get::<_, Balance>(&key)
            .map(|b| b.amount)
            .unwrap_or(0)
    }

    /// Mint tokens (simplified — no authorization for testing).
    pub fn mint(env: Env, to: Address, amount: i128) {
        let key = format!("bal:{}", to);
        let key = soroban_sdk::Symbol::new(&env, &key);
        let current = env
            .storage()
            .persistent()
            .get::<_, Balance>(&key)
            .map(|b| b.amount)
            .unwrap_or(0);
        let new_amount = current.checked_add(amount).unwrap_or(i128::MAX);
        env.storage()
            .persistent()
            .set(&key, &Balance { amount: new_amount });
    }

    /// Transfer with switchable failure modes.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        let config_key = soroban_sdk::Symbol::short("config");
        let config: AdversarialConfig = env
            .storage()
            .instance()
            .get(&config_key)
            .unwrap_or_else(|| panic_with_error!(&env, AdversarialError::NotInitialized));

        match config.failure_mode {
            0 => {
                // Normal transfer (mode 0)
                Self::transfer_normal(&env, &from, &to, amount);
            }
            1 => {
                // Panic on transfer (mode 1)
                panic_with_error!(&env, AdversarialError::PanicOnTransfer);
            }
            2 => {
                // Silent no-op: return success but don't move funds (mode 2)
                // This exploits SEP-41 trust model: the caller assumes the transfer happened
                // but the token contract is lying about its balance updates
            }
            3 => {
                // Reentry attempt (mode 3)
                // Try to call back into tipjar.tip during this transfer
                // This documents current Soroban host behavior (same-contract reentrancy prohibited)
                Self::transfer_with_reentry(&env, &from, &to, amount, &config.tipjar_contract);
            }
            4 => {
                // Amount burn: transfer amount - 1 (mode 4)
                if amount > 0 {
                    Self::transfer_normal(&env, &from, &to, amount.saturating_sub(1));
                }
            }
            _ => {
                // Unknown mode defaults to normal
                Self::transfer_normal(&env, &from, &to, amount);
            }
        }
    }

    // Helper: normal transfer
    fn transfer_normal(env: &Env, from: &Address, to: &Address, amount: i128) {
        let from_key = format!("bal:{}", from);
        let from_key = soroban_sdk::Symbol::new(env, &from_key);
        let to_key = format!("bal:{}", to);
        let to_key = soroban_sdk::Symbol::new(env, &to_key);

        let from_balance = env
            .storage()
            .persistent()
            .get::<_, Balance>(&from_key)
            .map(|b| b.amount)
            .unwrap_or(0);
        if from_balance < amount {
            panic!("insufficient balance");
        }

        let to_balance = env
            .storage()
            .persistent()
            .get::<_, Balance>(&to_key)
            .map(|b| b.amount)
            .unwrap_or(0);

        env.storage().persistent().set(
            &from_key,
            &Balance {
                amount: from_balance - amount,
            },
        );
        env.storage().persistent().set(
            &to_key,
            &Balance {
                amount: to_balance.checked_add(amount).unwrap_or(i128::MAX),
            },
        );
    }

    // Helper: reentry attempt
    fn transfer_with_reentry(
        env: &Env,
        from: &Address,
        to: &Address,
        amount: i128,
        tipjar: &Address,
    ) {
        // First do a normal transfer
        Self::transfer_normal(env, from, to, amount);

        // Then attempt reentry (this will fail on current Soroban host with same-contract reentrancy prohibition)
        // This test documents the host behavior and ensures we know when/if it changes
        let _result = env.invoke_contract(
            tipjar,
            &Symbol::short("tip"),
            soroban_sdk::vec![env, from.clone(), to.clone(), amount],
        );
        // If we get here, reentry succeeded (unexpected on current host)
    }
}
