# Lending Protocol Architecture

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Tipjar Lending System                     │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────────┐    ┌─────────────────┐                 │
│  │  Lending Pool   │    │  Loan Position  │                 │
│  ├─────────────────┤    ├─────────────────┤                 │
│  │ - Pool ID       │    │ - Loan ID       │                 │
│  │ - Token         │    │ - Borrower      │                 │
│  │ - Liquidity     │    │ - Amount        │                 │
│  │ - Borrowed      │    │ - Collateral    │                 │
│  │ - Interest      │    │ - Interest      │                 │
│  └────────┬────────┘    │ - Status        │                 │
│           │             └────────┬────────┘                 │
│           │                      │                           │
│  ┌────────▼──────────────────────▼──────────┐               │
│  │      Lender (Deposit Position)           │               │
│  ├──────────────────────────────────────────┤               │
│  │ - Lender Address                         │               │
│  │ - Amount Deposited                       │               │
│  │ - Interest Accrued                       │               │
│  │ - Deposit Timestamp                      │               │
│  └──────────────────────────────────────────┘               │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

## Module Organization

```
lending/
├── mod.rs
│   ├── Core Types
│   │   ├── PoolId
│   │   ├── Pool
│   │   ├── Deposit
│   │   ├── Loan
│   │   ├── LoanStatus
│   │   └── LendingKey (storage)
│   │
│   └── Error Types
│       └── LendingError
│
├── pool.rs
│   ├── create_pool(token) -> PoolId
│   ├── get_pool(pool_id) -> Pool
│   ├── deposit(pool_id, lender, amount)
│   ├── withdraw(pool_id, lender, amount)
│   ├── get_deposit(lender, pool_id) -> Deposit
│   └── set_pool(pool) [internal]
│
├── loan.rs
│   ├── borrow(pool_id, borrower, amount, collateral) -> loan_id
│   ├── repay(loan_id, amount)
│   ├── liquidate(loan_id)
│   ├── get_loan(loan_id) -> Loan
│   └── get_borrower_loans(borrower) -> Vec<loan_id>
│
└── interest.rs
    ├── calculate_rate(borrowed, liquidity) -> u32
    ├── calculate_interest(principal, rate, seconds) -> i128
    └── is_liquidatable(loan_amount, collateral) -> bool
```

## Data Flow Diagrams

### Deposit Flow
```
User (Lender)
    │
    ├─ deposit(pool_id, 1000)
    │
    ▼
[Contract]
    │
    ├─ Transfer: User ──1000──> Contract
    │
    ├─ Update Pool
    │   └─ total_liquidity += 1000
    │
    ├─ Update Deposit
    │   └─ amount = 1000
    │       interest_accrued = 0
    │       deposit_timestamp = now
    │
    └─ Store in Persistent Storage
        └─ LendingKey::Deposit(user, pool_id)
        └─ LendingKey::Pool(pool_id)
```

### Borrow Flow
```
User (Borrower)
    │
    ├─ borrow(pool_id, 1000, 1500)
    │
    ▼
[Validation]
    │
    ├─ Check: collateral >= amount * 1.50
    │   └─ 1500 >= 1000 * 1.50 ✓
    │
    └─ Check: pool.liquidity >= amount
        └─ pool.liquidity >= 1000 ✓
    │
    ▼
[Contract]
    │
    ├─ Transfer: User ──1500──> Contract (collateral)
    │
    ├─ Transfer: Contract ──1000──> User (loan)
    │
    ├─ Create Loan
    │   └─ amount = 1000
    │       collateral = 1500
    │       status = Active
    │       borrow_timestamp = now
    │
    ├─ Update Pool
    │   ├─ total_liquidity -= 1000
    │   └─ total_borrowed += 1000
    │
    └─ Store: LendingKey::Loan(loan_id)
              LendingKey::BorrowerLoans(user)
```

### Repay Flow
```
User (Borrower)
    │
    ├─ repay(loan_id, 1050)
    │
    ▼
[Calculation]
    │
    ├─ Get Loan
    │
    ├─ Calculate Interest
    │   ├─ time_elapsed = now - borrow_timestamp
    │   ├─ current_rate = f(utilization)
    │   └─ interest = principal * rate * time / year
    │
    └─ Verify: amount >= principal + interest
        └─ 1050 >= 1000 + interest ✓
    │
    ▼
[Contract]
    │
    ├─ Transfer: Contract ──1500──> User (collateral)
    │
    ├─ Transfer: Contract ──50──> User (overpayment)
    │
    ├─ Update Loan
    │   ├─ status = Repaid
    │   └─ interest_accrued = calculated
    │
    ├─ Update Pool
    │   ├─ total_borrowed -= 1000
    │   ├─ total_liquidity += 1050
    │   └─ accumulated_interest += interest
    │
    └─ Store: Updated Loan and Pool
```

### Liquidation Flow
```
Liquidator
    │
    ├─ liquidate(loan_id)
    │
    ▼
[Validation]
    │
    ├─ Get Loan
    │
    ├─ Check: collateral < (loan_amount * 110 / 100)
    │   └─ Example: 1080 < (1000 * 110 / 100) = 1100 ✓
    │
    └─ Check: status == Active
    │
    ▼
[Contract]
    │
    ├─ Calculate Excess
    │   └─ excess = collateral - (loan * 110 / 100)
    │       excess = 1080 - 1100 = -20 (none)
    │
    ├─ Transfer: Contract ──(if excess)──> Borrower
    │
    ├─ Update Loan
    │   └─ status = Liquidated
    │
    ├─ Update Pool
    │   ├─ total_borrowed -= 1000
    │   └─ total_liquidity += 1080
    │
    └─ Store: Updated Loan and Pool
```

## Storage Layout

### Instance Storage Keys

```
Lending Storage Structure:
├── PoolCounter: u64
│   └─ Global counter for pool IDs
│
├── Pool(PoolId) -> Pool
│   ├─ id: PoolId
│   ├─ token: Address
│   ├─ total_liquidity: i128
│   ├─ total_borrowed: i128
│   └─ accumulated_interest: i128
│
├── Deposit(Address, PoolId) -> Deposit
│   ├─ lender: Address
│   ├─ pool_id: PoolId
│   ├─ amount: i128
│   ├─ interest_accrued: i128
│   └─ deposit_timestamp: u64
│
├── LoanCounter: u64
│   └─ Global counter for loan IDs
│
├── Loan(u64) -> Loan
│   ├─ id: u64
│   ├─ pool_id: PoolId
│   ├─ borrower: Address
│   ├─ amount: i128
│   ├─ collateral: i128
│   ├─ interest_accrued: i128
│   ├─ borrow_timestamp: u64
│   └─ status: LoanStatus
│
└── BorrowerLoans(Address) -> Vec<u64>
    └─ List of loan IDs for a borrower
```

## State Transitions

### Loan Lifecycle
```
                    ┌─ Active ◄──┐
                    │             │
    create_loan()  /  repay_ok    \  liquidate()
                  /                \
              [Active]          [Repaid]
                  │
                  │ liquidate()
                  │
              [Liquidated]
```

### Deposit Lifecycle
```
            ┌─ exists ─────────┐
            │                  │
   deposit()│               withdraw()
            │                  │
        [Empty]◄──────────────►[Has Balance]
            │                  │
            └─────────→ removed
              (amount = 0)
```

## Interest Calculation Pipeline

```
Time t0: Loan created
         ├─ loan.amount = 1000
         ├─ loan.borrow_timestamp = t0
         └─ interest_accrued = 0

Time t0+30 days: Interest accrues
         ├─ time_elapsed = 30 days
         ├─ utilization = total_borrowed / (total_borrowed + total_liquidity)
         ├─ rate = 5000 + (utilization * 45000)
         ├─ interest = 1000 * rate / 10000 * (30*86400) / 31536000
         └─ interest ≈ 41 tokens (at 50% utilization)

Time t0+60 days: More interest
         ├─ time_elapsed = 30 days (since last calculation)
         ├─ compounded interest ≈ 82 tokens
         └─ total_accrued ≈ 82 tokens

Repay at t0+60 days:
         ├─ Required: 1000 + 82 = 1082
         ├─ User pays 1082+ 
         ├─ Repayment processed
         └─ Interest distributed to lenders
```

## Utilization-Based Interest Model

```
Interest Rate Chart:
┌─────────────────────────────────────────┐
│ Rate (%)                                │
│ 50 │                        ╱────────    │
│    │                       ╱             │
│ 40 │                      ╱              │
│    │                     ╱               │
│ 30 │                    ╱                │
│    │                   ╱                 │
│ 20 │                  ╱                  │
│    │                 ╱                   │
│ 10 │                ╱                    │
│    │               ╱                     │
│  5 │──────────────╱                      │
│    │             ╱                       │
│  0 └─────────────────────────────────────│
│    0%   20%  40%  60%  80%  100% Utilization
│
│ Linear formula: rate = 5% + utilization*45%
│ - 0% util:  5%   (min)
│ - 50% util: 27.5%
│ - 100% util: 50% (max)
└─────────────────────────────────────────┘
```

## Collateral Management

```
Collateral Ratio States:

┌──────────────────────────────────────────┐
│ Collateral / Loan Ratio                  │
│                                          │
│ 200% ├─ Super Safe                       │
│      │                                   │
│ 150% ├─ Minimum (deposit requirement)    │
│      │                                   │
│ 110% ├─ Liquidation Threshold            │
│      │                                   │
│ 100% ├─ Undercollateralized              │
│      │                                   │
│   0% └─────────────────────────────────  │
│                                          │
│  Safe ◄─────────────► Liquidatable       │
└──────────────────────────────────────────┘

Rules:
- Borrow requires: collateral ≥ 150%
- Liquidate allowed: collateral < 110%
- Safe zone: collateral ≥ 110%
```

## Error Handling Flow

```
User Action
    │
    ▼
[Input Validation]
    │
    ├─ Amount > 0?
    │   └─ No: InvalidAmount error
    │
    ├─ Authorized?
    │   └─ No: Unauthorized error
    │
    └─ Sufficient funds?
        └─ No: InsufficientBalance error
    │
    ▼
[State Checks]
    │
    ├─ Pool exists?
    │   └─ No: PoolNotFound error
    │
    ├─ Loan exists?
    │   └─ No: LoanNotFound error
    │
    ├─ Loan active?
    │   └─ No: LoanNotActive error
    │
    └─ Liquidatable?
        └─ No: CannotLiquidate error
    │
    ▼
[Contract Logic]
    │
    ├─ Update state
    ├─ Transfer tokens
    ├─ Store results
    │
    └─ Return success or error
```

## Performance Characteristics

| Operation | Time Complexity | Space | Gas |
|-----------|-----------------|-------|-----|
| create_pool | O(1) | O(1) | Low |
| get_pool | O(1) | O(1) | Low |
| deposit | O(1) | O(1) | Medium |
| withdraw | O(1) | O(1) | Medium |
| borrow | O(1) | O(1) | Medium |
| repay | O(1) | O(1) | Medium |
| liquidate | O(1) | O(1) | Medium |
| calculate_rate | O(1) | O(1) | Low |
| calculate_interest | O(1) | O(1) | Low |

Note: All operations are O(1) - no loops over variable-sized collections.

## Scalability Considerations

### Current Limitations
- One pool per token (by design)
- Maximum i128 values: ~1.7 × 10^38 tokens
- Interest precision: 18 decimal places
- Supports unlimited lenders and borrowers

### Future Scaling
- Multiple pools per token (different collateral types)
- Cross-chain communication
- Sharded pool management
- Batch operations for efficiency

## Security Model

```
Trust Boundaries:
┌────────────────────────────────────────┐
│       Contract Account                 │
│  (Holds all tokens & collateral)       │
│                                        │
│  ┌──────────────────────────────────┐  │
│  │ Storage                          │  │
│  │ - Pools and loans immutable      │  │
│  │ - Atomic updates                 │  │
│  │ - Authorized access only         │  │
│  └──────────────────────────────────┘  │
└────────────────────────────────────────┘
        ▲                        ▲
        │                        │
    [Lenders]              [Borrowers]
        │                        │
        └────────────┬───────────┘
                     │
                 [Users]
                     │
                 Trust boundary:
                 - Authorized tx only
                 - Verified collateral
                 - Rate limits applied
```

## Summary

The lending protocol provides:
- **Efficient**: All operations O(1), minimal gas usage
- **Secure**: Collateral held by contract, rate-limited
- **Scalable**: Supports unlimited participants
- **Transparent**: All calculations deterministic
- **Composable**: Integrates with existing tipjar contract
