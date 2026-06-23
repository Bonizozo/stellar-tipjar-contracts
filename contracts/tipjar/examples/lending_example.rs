//! Example usage of the lending protocol.
//! 
//! This demonstrates how to integrate and use the lending protocol
//! for peer-to-peer lending of tip tokens.

use soroban_sdk::{contract, contractimpl, Address, Env};

// These would be imported from tipjar crate in real usage
// use tipjar::lending::{pool, loan, PoolId};
// use tipjar::TipJarError;

/// Example lending application contract
#[contract]
pub struct LendingApp;

#[contractimpl]
impl LendingApp {
    /// Initialize a new lending pool for a token
    /// 
    /// # Example
    /// ```
    /// let pool_id = create_lending_pool(env, token_address);
    /// // Pool now accepts deposits and loans
    /// ```
    pub fn create_lending_pool(env: Env, token: Address) -> u64 {
        // In real usage:
        // pool::create_pool(&env, token)
        //     .expect("Failed to create pool")
        //     .0
        0
    }

    /// Deposit tokens into a lending pool to earn interest
    /// 
    /// # Example
    /// ```
    /// deposit_to_pool(env, pool_id, lender_address, 1000);
    /// // Lender now earns interest as the pool generates lending income
    /// ```
    pub fn deposit_to_pool(
        env: Env,
        pool_id: u64,
        lender: Address,
        amount: i128,
    ) -> Result<(), String> {
        // In real usage:
        // pool::deposit(&env, PoolId(pool_id), lender, amount)
        //     .map_err(|_| "Deposit failed".to_string())
        Ok(())
    }

    /// Withdraw tokens and earned interest from a lending pool
    /// 
    /// # Example
    /// ```
    /// withdraw_from_pool(env, pool_id, lender_address, 500);
    /// // Lender receives 500 + accrued interest
    /// ```
    pub fn withdraw_from_pool(
        env: Env,
        pool_id: u64,
        lender: Address,
        amount: i128,
    ) -> Result<(), String> {
        // In real usage:
        // pool::withdraw(&env, PoolId(pool_id), lender, amount)
        //     .map_err(|_| "Withdrawal failed".to_string())
        Ok(())
    }

    /// Borrow tokens from a pool with collateral
    /// 
    /// # Requirements
    /// - Collateral must be at least 150% of the loan amount
    /// - Pool must have sufficient liquidity
    /// 
    /// # Example
    /// ```
    /// // Borrow 1000 tokens with 1500 collateral (150% ratio)
    /// let loan_id = borrow_from_pool(
    ///     env, 
    ///     pool_id, 
    ///     borrower_address, 
    ///     1000,  // amount
    ///     1500   // collateral
    /// );
    /// // Borrower receives 1000 tokens, collateral is held by contract
    /// ```
    pub fn borrow_from_pool(
        env: Env,
        pool_id: u64,
        borrower: Address,
        amount: i128,
        collateral: i128,
    ) -> Result<u64, String> {
        // In real usage:
        // loan::borrow(&env, PoolId(pool_id), borrower, amount, collateral)
        //     .map_err(|_| "Borrow failed".to_string())
        Ok(0)
    }

    /// Repay a loan with accrued interest
    /// 
    /// # Example
    /// ```
    /// // Repay a 1000 token loan with interest
    /// repay_loan(env, loan_id, 1050);
    /// // Borrower receives collateral back
    /// ```
    pub fn repay_loan(env: Env, loan_id: u64, amount: i128) -> Result<(), String> {
        // In real usage:
        // loan::repay(&env, loan_id, amount)
        //     .map_err(|_| "Repay failed".to_string())
        Ok(())
    }

    /// Liquidate an undercollateralized loan
    /// 
    /// When a loan's collateral falls below 110% of the loan amount,
    /// it becomes liquidatable. This function seizes the collateral
    /// and returns excess to the borrower.
    /// 
    /// # Example
    /// ```
    /// // Loan: 1000 amount, 1080 collateral (108% ratio, undercollateralized)
    /// liquidate_loan(env, loan_id);
    /// // Collateral becomes pool liquidity, excess returned to borrower
    /// ```
    pub fn liquidate_loan(env: Env, loan_id: u64) -> Result<(), String> {
        // In real usage:
        // loan::liquidate(&env, loan_id)
        //     .map_err(|_| "Liquidation failed".to_string())
        Ok(())
    }

    /// Get current pool state
    pub fn get_pool_state(env: Env, pool_id: u64) -> Result<(i128, i128, i128), String> {
        // In real usage:
        // let pool = pool::get_pool(&env, PoolId(pool_id))
        //     .map_err(|_| "Pool not found".to_string())?;
        // Ok((pool.total_liquidity, pool.total_borrowed, pool.accumulated_interest))
        Ok((0, 0, 0))
    }

    /// Get current interest rate for a pool (in basis points, e.g., 5000 = 5%)
    pub fn get_pool_interest_rate(env: Env, pool_id: u64) -> Result<u32, String> {
        // In real usage:
        // let pool = pool::get_pool(&env, PoolId(pool_id))
        //     .map_err(|_| "Pool not found".to_string())?;
        // let rate = lending::interest::calculate_rate(pool.total_borrowed, pool.total_liquidity);
        // Ok(rate)
        Ok(0)
    }
}

/// Usage Scenario
/// 
/// 1. Creator creates a lending pool for their tip token
///    pool_id = create_lending_pool(env, tip_token)
/// 
/// 2. Supporters (lenders) deposit tokens to earn yield
///    deposit_to_pool(env, pool_id, supporter_address, 1000)
///    // Supporter earns interest as the pool lends out tokens
/// 
/// 3. Other creators (borrowers) borrow against collateral
///    loan_id = borrow_from_pool(env, pool_id, creator_address, 1000, 1500)
///    // Creator receives 1000 tokens, must maintain 1500 collateral
/// 
/// 4. Borrower repays the loan with accrued interest
///    repay_loan(env, loan_id, 1050)  // 1000 principal + ~50 interest
///    // Collateral returned to borrower
/// 
/// 5. If collateral value drops below 110% of loan, liquidation occurs
///    liquidate_loan(env, loan_id)
///    // Collateral seized, excess returned to borrower
/// 
/// 6. Lender can withdraw their deposit + interest
///    withdraw_from_pool(env, pool_id, supporter_address, 1000)
///    // Returns 1000 + earned interest
