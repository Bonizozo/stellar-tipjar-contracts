//! Loan origination, repayment, and liquidation.

use soroban_sdk::{token, Address, Env, Vec};

use crate::TipJarError;
use super::{Loan, LoanStatus, PoolId, LendingKey};
use super::pool::{get_pool};
use super::interest::{calculate_rate, is_liquidatable};

const COLLATERAL_RATIO: i128 = 150; // 150% minimum collateral ratio
const LIQUIDATION_BONUS: i128 = 110; // 110% threshold for liquidation

/// Borrow tokens with collateral.
pub fn borrow(
    env: &Env,
    pool_id: PoolId,
    borrower: Address,
    amount: i128,
    collateral: i128,
) -> Result<u64, TipJarError> {
    if amount <= 0 || collateral <= 0 {
        return Err(TipJarError::InvalidAmount);
    }

    let mut pool = get_pool(env, pool_id)?;

    // Check collateral ratio: collateral >= (amount * 150) / 100
    let required_collateral = (amount * COLLATERAL_RATIO) / 100;
    if collateral < required_collateral {
        return Err(TipJarError::InsufficientCollateral);
    }

    if pool.total_liquidity < amount {
        return Err(TipJarError::InsufficientBalance);
    }

    // Get next loan ID
    let loan_counter: u64 = env
        .storage()
        .instance()
        .get(&(LendingKey::LoanCounter))
        .unwrap_or(0u64);
    let loan_id = loan_counter + 1;

    // Transfer collateral from borrower to contract
    let token_client = token::Client::new(env, &pool.token);
    token_client.transfer(&borrower, &env.current_contract_address(), &collateral);

    // Transfer loan amount from contract to borrower
    token_client.transfer(&env.current_contract_address(), &borrower, &amount);

    // Create loan record
    let loan = Loan {
        id: loan_id,
        pool_id,
        borrower: borrower.clone(),
        amount,
        collateral,
        interest_accrued: 0,
        borrow_timestamp: env.ledger().timestamp(),
        status: LoanStatus::Active,
    };

    // Update storage
    env.storage()
        .instance()
        .set(&(LendingKey::Loan(loan_id)), &loan);
    env.storage()
        .instance()
        .set(&(LendingKey::LoanCounter), &loan_id);

    // Add to borrower's loan list
    let mut borrower_loans: Vec<u64> = env
        .storage()
        .instance()
        .get(&(LendingKey::BorrowerLoans(borrower.clone())))
        .unwrap_or_else(|| Vec::new(env));
    borrower_loans.push_back(loan_id);
    env.storage()
        .instance()
        .set(&(LendingKey::BorrowerLoans(borrower.clone())), &borrower_loans);

    // Update pool
    pool.total_liquidity -= amount;
    pool.total_borrowed += amount;
    env.storage()
        .instance()
        .set(&(LendingKey::Pool(pool_id)), &pool);

    Ok(loan_id)
}

/// Repay a loan with accrued interest.
pub fn repay(env: &Env, loan_id: u64, amount: i128) -> Result<(), TipJarError> {
    if amount <= 0 {
        return Err(TipJarError::InvalidAmount);
    }

    let mut loan = get_loan(env, loan_id)?;

    if loan.status != LoanStatus::Active {
        return Err(TipJarError::LoanNotActive);
    }

    let mut pool = get_pool(env, loan.pool_id)?;

    // Calculate interest accrued
    let time_elapsed = env.ledger().timestamp() - loan.borrow_timestamp;
    let current_rate = calculate_rate(pool.total_borrowed, pool.total_liquidity);
    let interest_accrued = super::interest::calculate_interest(loan.amount, current_rate, time_elapsed);

    let total_owed = loan.amount + interest_accrued;

    if amount < total_owed {
        return Err(TipJarError::InsufficientBalance);
    }

    // Transfer repayment + collateral back to borrower
    let token_client = token::Client::new(env, &pool.token);
    token_client.transfer(&env.current_contract_address(), &loan.borrower, &loan.collateral);
    
    // If overpayment, return excess to borrower
    let overpayment = amount - total_owed;
    if overpayment > 0 {
        token_client.transfer(&env.current_contract_address(), &loan.borrower, &overpayment);
    }

    // Update loan and pool
    loan.status = LoanStatus::Repaid;
    loan.interest_accrued = interest_accrued;
    pool.total_borrowed -= loan.amount;
    pool.total_liquidity += amount;
    pool.accumulated_interest += interest_accrued;

    env.storage()
        .instance()
        .set(&(LendingKey::Loan(loan_id)), &loan);
    env.storage()
        .instance()
        .set(&(LendingKey::Pool(loan.pool_id)), &pool);

    Ok(())
}

/// Liquidate an undercollateralized loan.
pub fn liquidate(env: &Env, loan_id: u64) -> Result<(), TipJarError> {
    let mut loan = get_loan(env, loan_id)?;

    if loan.status != LoanStatus::Active {
        return Err(TipJarError::LoanNotActive);
    }

    // Check if liquidatable
    if !is_liquidatable(loan.amount, loan.collateral) {
        return Err(TipJarError::CannotLiquidate);
    }

    let pool = get_pool(env, loan.pool_id)?;

    // Transfer collateral to contract (kept for liquidation) and remaining to borrower if any
    let liquidation_proceeds = loan.collateral;
    let token_client = token::Client::new(env, &pool.token);

    // Return any excess collateral to borrower
    let threshold = (loan.amount * LIQUIDATION_BONUS) / 100;
    if liquidation_proceeds > threshold {
        let excess = liquidation_proceeds - threshold;
        token_client.transfer(&env.current_contract_address(), &loan.borrower, &excess);
    }

    // Update loan and pool
    loan.status = LoanStatus::Liquidated;
    let mut pool = pool.clone();
    pool.total_borrowed -= loan.amount;
    pool.total_liquidity += threshold;

    env.storage()
        .instance()
        .set(&(LendingKey::Loan(loan_id)), &loan);
    env.storage()
        .instance()
        .set(&(LendingKey::Pool(loan.pool_id)), &pool);

    Ok(())
}

/// Get loan by ID.
pub fn get_loan(env: &Env, loan_id: u64) -> Result<Loan, TipJarError> {
    env.storage()
        .instance()
        .get(&(LendingKey::Loan(loan_id)))
        .ok_or_else(|| {
            env.panic_with_error(TipJarError::InvalidAmount);
            TipJarError::InvalidAmount
        })
}

/// Get all loans for a borrower.
pub fn get_borrower_loans(env: &Env, borrower: &Address) -> Vec<u64> {
    env.storage()
        .instance()
        .get(&(LendingKey::BorrowerLoans(borrower.clone())))
        .unwrap_or_else(|| Vec::new(env))
}
