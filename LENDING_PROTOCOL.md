# Peer-to-Peer Lending Protocol

Implements a lending protocol for tip tokens with collateral management, interest rate calculations, and liquidation mechanisms.

## Architecture

### Core Components

1. **Lending Pools**: Liquidity pools where lenders deposit tip tokens and earn interest
2. **Loan Positions**: Track active loans with borrower, amount, collateral, and accrued interest
3. **Collateral Management**: Validate collateral ratios and track liquidation thresholds
4. **Interest Rate Engine**: Dynamic interest based on pool utilization
5. **Liquidation Engine**: Automatic liquidation of undercollateralized loans

### Data Model

#### Pool
- `id`: Unique pool identifier
- `token`: Token address for this pool
- `total_liquidity`: Total tokens available for borrowing
- `total_borrowed`: Total amount currently lent out
- `accumulated_interest`: Total interest collected

#### Deposit (Lender Position)
- `lender`: Lender address
- `pool_id`: Associated pool
- `amount`: Tokens deposited
- `interest_accrued`: Total interest earned
- `deposit_timestamp`: When deposit was made

#### Loan (Borrower Position)
- `id`: Unique loan identifier
- `pool_id`: Associated pool
- `borrower`: Borrower address
- `amount`: Loan principal
- `collateral`: Collateral provided
- `interest_accrued`: Total interest accrued
- `borrow_timestamp`: When loan was created
- `status`: Active, Repaid, or Liquidated

## Protocol Parameters

| Parameter | Value | Purpose |
|-----------|-------|---------|
| Collateral Ratio | 150% | Minimum collateral required (borrow $1 → deposit $1.50) |
| Liquidation Threshold | 110% | Threshold below which loans are liquidatable |
| Base Interest Rate | 5% | Minimum annual interest rate |
| Max Interest Rate | 50% | Maximum annual interest rate |
| Interest Model | Linear | Rate = 5% + (utilization × 45%) |

## Interest Rate Calculation

Interest rates are calculated based on pool utilization:

```
utilization = total_borrowed / (total_borrowed + total_liquidity)
rate = 5% + (utilization × 45%)
```

- At 0% utilization: 5% annual rate
- At 50% utilization: ~27.5% annual rate  
- At 100% utilization: 50% annual rate

Interest accrual is calculated per-second:

```
interest = principal × (rate / 10000) × (seconds / 31536000)
```

## API Reference

### Pool Operations

#### `create_pool(env, token) -> Result<PoolId>`
Creates a new lending pool for a token.

**Parameters:**
- `env`: Soroban environment
- `token`: Token contract address

**Returns:** New `PoolId`

#### `get_pool(env, pool_id) -> Result<Pool>`
Retrieves pool state.

#### `deposit(env, pool_id, lender, amount) -> Result<()>`
Lender deposits tokens into a pool.

**Parameters:**
- `lender`: Lender address
- `amount`: Amount to deposit (must be > 0)

**Effects:**
- Transfers tokens from lender to contract
- Updates lender's deposit record
- Increases `total_liquidity`

#### `withdraw(env, pool_id, lender, amount) -> Result<()>`
Lender withdraws tokens and earned interest.

**Parameters:**
- `amount`: Amount to withdraw (must be ≤ deposit)

**Effects:**
- Calculates accrued interest based on time and current rate
- Transfers tokens + interest to lender
- Updates pool liquidity

### Loan Operations

#### `borrow(env, pool_id, borrower, amount, collateral) -> Result<LoanId>`
Borrower creates a loan with collateral.

**Parameters:**
- `borrower`: Borrower address
- `amount`: Loan amount (must be > 0)
- `collateral`: Collateral (must be ≥ 150% of amount)

**Validation:**
- `collateral ≥ (amount × 150) / 100`
- `pool.total_liquidity ≥ amount`

**Effects:**
- Transfers collateral from borrower to contract
- Transfers loan amount from contract to borrower
- Creates loan record with `Active` status
- Updates `total_borrowed` and `total_liquidity`

#### `repay(env, loan_id, amount) -> Result<()>`
Borrower repays a loan.

**Parameters:**
- `amount`: Amount to repay (must be ≥ principal + interest)

**Effects:**
- Calculates accrued interest
- Transfers collateral + excess (if overpaid) back to borrower
- Updates pool `total_borrowed` and `total_liquidity`
- Sets loan status to `Repaid`

#### `liquidate(env, loan_id) -> Result<()>`
Liquidate an undercollateralized loan.

**Conditions:**
- Loan must be `Active`
- `collateral < (loan_amount × 110) / 100`

**Effects:**
- Converts collateral to pool liquidity
- Returns excess collateral (above 110% threshold) to borrower
- Sets loan status to `Liquidated`

#### `get_loan(env, loan_id) -> Result<Loan>`
Retrieves loan details.

#### `get_borrower_loans(env, borrower) -> Vec<LoanId>`
Get all loan IDs for a borrower.

## Example Flows

### Deposit Flow
```
1. Lender calls deposit(pool_id, 1000)
2. 1000 tokens transferred from lender → contract
3. Deposit recorded: amount=1000, interest_accrued=0
4. Pool updated: total_liquidity += 1000
```

### Borrow Flow
```
1. Borrower calls borrow(pool_id, 1000, 1500)
2. Validate: 1500 ≥ (1000 × 150 / 100) = 1500 ✓
3. Collateral transferred: borrower → contract (1500)
4. Loan amount transferred: contract → borrower (1000)
5. Loan created: amount=1000, collateral=1500, status=Active
6. Pool updated: total_borrowed += 1000, total_liquidity -= 1000
```

### Repay Flow
```
1. Borrower calls repay(loan_id, 1050)
2. Calculate interest accrued over time
3. Verify: 1050 ≥ 1000 + interest ✓
4. Collateral + overpayment returned to borrower
5. Loan status = Repaid
6. Pool updated: total_borrowed -= 1000, total_liquidity += 1050
```

### Liquidation Flow
```
1. Loan: amount=1000, collateral=1080 (108% ratio)
2. Check: 1080 < (1000 × 110 / 100) = 1100 ✓ Liquidatable
3. Collateral -> pool (1000 threshold)
4. Excess (80) returned to borrower
5. Loan status = Liquidated
6. Pool updated: total_borrowed -= 1000, total_liquidity += 1000
```

## Security Considerations

### Collateral Management
- Minimum 150% collateral ratio prevents under-collateralization
- All collateral held by contract, not borrower
- Liquidation threshold (110%) provides liquidator buffer

### Interest Accrual
- Per-second calculation prevents interest compression attacks
- Rates capped at 50% prevents predatory lending
- Interest only accrues on active loans

### Liquidation
- Automatic liquidation prevents cascade failures
- Excess collateral returned to borrower (no punishment fees)
- Cannot liquidate safe loans (>110% ratio)

## Integration Guide

### Adding to Existing Contract

1. Import lending module:
```rust
use crate::lending::{pool, loan, PoolId};
```

2. Expose lending functions:
```rust
#[contract]
pub struct LendingContract;

#[contractimpl]
impl LendingContract {
    pub fn create_pool(env: Env, token: Address) -> Result<PoolId> {
        pool::create_pool(&env, token)
    }
    
    pub fn borrow(
        env: Env,
        pool_id: PoolId,
        borrower: Address,
        amount: i128,
        collateral: i128,
    ) -> Result<u64> {
        loan::borrow(&env, pool_id, borrower, amount, collateral)
    }
    // ... other functions
}
```

### Testing

Run lending tests:
```bash
cargo test --test lending_tests
```

Tests cover:
- Pool creation and lifecycle
- Deposit/withdraw operations
- Loan origination with collateral validation
- Repayment and liquidation
- Interest calculations
- Boundary conditions

## Future Enhancements

1. **Variable Interest Models**: Support different rate curves
2. **Flash Loans**: Uncollateralized lending with immediate repayment requirement
3. **Oracle Integration**: Real-time collateral price feeds
4. **Governance**: DAO control over protocol parameters
5. **Multi-Collateral**: Accept multiple token types as collateral
6. **Liquidation Auctions**: Competitive liquidation mechanism
