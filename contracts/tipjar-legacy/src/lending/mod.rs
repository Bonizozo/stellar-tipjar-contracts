//! Peer-to-peer lending protocol for tip tokens with collateral and interest.

use soroban_sdk::{contracttype, Address};

pub mod interest;
pub mod loan;
pub mod pool;

/// Unique lending pool identifier.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PoolId(pub u64);

/// Status of a loan position.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoanStatus {
    Active,
    Repaid,
    Liquidated,
}

/// Lending pool with liquidity and borrowing state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pool {
    pub id: PoolId,
    pub token: Address,
    pub total_liquidity: i128,
    pub total_borrowed: i128,
    pub accumulated_interest: i128,
}

/// Lender deposit with interest tracking.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deposit {
    pub lender: Address,
    pub pool_id: PoolId,
    pub amount: i128,
    pub interest_accrued: i128,
    pub deposit_timestamp: u64,
}

/// Borrower loan with collateral tracking.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Loan {
    pub id: u64,
    pub pool_id: PoolId,
    pub borrower: Address,
    pub amount: i128,
    pub collateral: i128,
    pub interest_accrued: i128,
    pub borrow_timestamp: u64,
    pub status: LoanStatus,
}

/// Extended DataKey variants for lending.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LendingKey {
    /// Global pool counter (u64).
    PoolCounter,
    /// Pool by ID (PoolId -> Pool).
    Pool(PoolId),
    /// Deposit by (lender, pool_id).
    Deposit(Address, PoolId),
    /// Loan by ID (u64 -> Loan).
    Loan(u64),
    /// Global loan counter (u64).
    LoanCounter,
    /// List of loan IDs for borrower.
    BorrowerLoans(Address),
}

/// Lending protocol errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LendingError {
    PoolNotFound = 100,
    InsufficientLiquidity = 101,
    InsufficientCollateral = 102,
    LoanNotFound = 103,
    LoanNotActive = 104,
    CannotLiquidate = 105,
    InvalidAmount = 106,
}
