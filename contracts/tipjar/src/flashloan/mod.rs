//! Flash loan execution logic.

pub mod receiver;

use soroban_sdk::{panic_with_error, symbol_short, token, Address, Bytes, Env};

use crate::{DataKey, TipJarError};

use receiver::FlashLoanReceiverClient;

/// Flash loan fee in basis points (`9` = 0.09%).
pub const FLASH_LOAN_FEE_BPS: i128 = 9;

/// Executes a flash loan and verifies repayment with fee.
pub fn flash_loan(env: &Env, receiver: &Address, token: &Address, amount: i128, params: &Bytes) {
    if amount <= 0 {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    enter_guard(env);

    let balance_before = get_token_balance(env, token);
    let fee = calculate_fee(amount);

    transfer_token(env, token, receiver, amount);

    let receiver_client = FlashLoanReceiverClient::new(env, receiver);
    receiver_client.execute_operation(token, &amount, &fee, params);

    let required_balance = balance_before + fee;
    let balance_after = get_token_balance(env, token);

    if balance_after < required_balance {
        clear_guard(env);
        panic_with_error!(env, TipJarError::FlashLoanNotRepaid);
    }

    clear_guard(env);

    env.events().publish(
        (symbol_short!("flashln"), receiver.clone()),
        (token.clone(), amount, fee),
    );
}

/// Calculates loan fee in basis points.
pub fn calculate_fee(amount: i128) -> i128 {
    amount * FLASH_LOAN_FEE_BPS / 10_000
}

fn get_token_balance(env: &Env, token_address: &Address) -> i128 {
    let token_client = token::Client::new(env, token_address);
    token_client.balance(&env.current_contract_address())
}

fn transfer_token(env: &Env, token_address: &Address, to: &Address, amount: i128) {
    let token_client = token::Client::new(env, token_address);
    token_client.transfer(&env.current_contract_address(), to, &amount);
}

fn enter_guard(env: &Env) {
    if env
        .storage()
        .instance()
        .get(&DataKey::FlashLoanGuard)
        .unwrap_or(false)
    {
        panic_with_error!(env, TipJarError::FlashLoanReentrant);
    }

    env.storage().instance().set(&DataKey::FlashLoanGuard, &true);
}

fn clear_guard(env: &Env) {
    env.storage().instance().set(&DataKey::FlashLoanGuard, &false);
}

#[cfg(test)]
mod tests {
    use super::calculate_fee;

    #[test]
    fn fee_calculation_matches_spec() {
        assert_eq!(calculate_fee(1_000_000), 900);
        assert_eq!(calculate_fee(10_000), 9);
    }
}
