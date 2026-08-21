//! Lending pool creation and lender operations.

use soroban_sdk::{token, Address, Env};

use super::interest::calculate_rate;
use super::{Deposit, LendingKey, Pool, PoolId};
use crate::TipJarError;

/// Create a new lending pool for a token.
pub fn create_pool(env: &Env, token: Address) -> Result<PoolId, TipJarError> {
    let counter: u64 = env
        .storage()
        .instance()
        .get(&(LendingKey::PoolCounter))
        .unwrap_or(0u64);

    let pool_id = PoolId(counter + 1);
    let pool = Pool {
        id: pool_id,
        token: token.clone(),
        total_liquidity: 0,
        total_borrowed: 0,
        accumulated_interest: 0,
    };

    env.storage()
        .instance()
        .set(&(LendingKey::Pool(pool_id)), &pool);
    env.storage()
        .instance()
        .set(&(LendingKey::PoolCounter), &(counter + 1));

    Ok(pool_id)
}

/// Get pool by ID.
pub fn get_pool(env: &Env, pool_id: PoolId) -> Result<Pool, TipJarError> {
    env.storage()
        .instance()
        .get(&(LendingKey::Pool(pool_id)))
        .ok_or_else(|| env.panic_with_error(TipJarError::InvalidAmount))
}

/// Update pool state.
fn set_pool(env: &Env, pool: &Pool) {
    env.storage()
        .instance()
        .set(&(LendingKey::Pool(pool.id)), pool);
}

/// Lender deposits tokens into a pool.
pub fn deposit(
    env: &Env,
    pool_id: PoolId,
    lender: Address,
    amount: i128,
) -> Result<(), TipJarError> {
    if amount <= 0 {
        return Err(TipJarError::InvalidAmount);
    }

    let mut pool = get_pool(env, pool_id)?;
    let mut deposit = get_or_create_deposit(env, &lender, pool_id);

    // Transfer tokens from lender to contract
    let token_client = token::Client::new(env, &pool.token);
    token_client.transfer(&lender, &env.current_contract_address(), &amount);

    // Update storage
    pool.total_liquidity += amount;
    deposit.amount += amount;
    deposit.deposit_timestamp = env.ledger().timestamp();

    set_pool(env, &pool);
    env.storage()
        .instance()
        .set(&(LendingKey::Deposit(lender.clone(), pool_id)), &deposit);

    Ok(())
}

/// Lender withdraws tokens and interest from a pool.
pub fn withdraw(
    env: &Env,
    pool_id: PoolId,
    lender: Address,
    amount: i128,
) -> Result<(), TipJarError> {
    if amount <= 0 {
        return Err(TipJarError::InvalidAmount);
    }

    let mut pool = get_pool(env, pool_id)?;
    let mut deposit = get_deposit(env, &lender, pool_id)?;

    if deposit.amount < amount {
        return Err(TipJarError::InsufficientBalance);
    }

    // Calculate interest accrued
    let current_rate = calculate_rate(pool.total_borrowed, pool.total_liquidity);
    let time_elapsed = env.ledger().timestamp() - deposit.deposit_timestamp;
    let interest_accrued =
        super::interest::calculate_interest(deposit.amount, current_rate, time_elapsed);

    let total_to_withdraw = amount + interest_accrued;
    if pool.total_liquidity < total_to_withdraw {
        return Err(TipJarError::InsufficientBalance);
    }

    // Transfer tokens back to lender
    let token_client = token::Client::new(env, &pool.token);
    token_client.transfer(&env.current_contract_address(), &lender, &total_to_withdraw);

    // Update storage
    pool.total_liquidity -= total_to_withdraw;
    deposit.amount -= amount;
    deposit.interest_accrued += interest_accrued;

    if deposit.amount == 0 {
        env.storage()
            .instance()
            .remove(&(LendingKey::Deposit(lender.clone(), pool_id)));
    } else {
        deposit.deposit_timestamp = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&(LendingKey::Deposit(lender.clone(), pool_id)), &deposit);
    }

    set_pool(env, &pool);
    Ok(())
}

/// Get lender's deposit or create empty one.
fn get_or_create_deposit(env: &Env, lender: &Address, pool_id: PoolId) -> Deposit {
    env.storage()
        .instance()
        .get(&(LendingKey::Deposit(lender.clone(), pool_id)))
        .unwrap_or_else(|| Deposit {
            lender: lender.clone(),
            pool_id,
            amount: 0,
            interest_accrued: 0,
            deposit_timestamp: env.ledger().timestamp(),
        })
}

/// Get lender's deposit.
pub fn get_deposit(env: &Env, lender: &Address, pool_id: PoolId) -> Result<Deposit, TipJarError> {
    env.storage()
        .instance()
        .get(&(LendingKey::Deposit(lender.clone(), pool_id)))
        .ok_or_else(|| env.panic_with_error(TipJarError::NothingToWithdraw))
}
