# Lending Protocol Quick Start

## 5-Minute Overview

The lending protocol enables peer-to-peer lending of tip tokens with:
- **Lender deposits** → earn interest yield
- **Borrower loans** → borrow with 150% collateral
- **Dynamic rates** → 5-50% APY based on pool utilization
- **Liquidations** → automatic closure when collateral < 110% of loan

## Core Components

```
Lending Pool (1 per token)
  ├─ Lenders deposit tokens → earn interest
  ├─ Borrowers borrow tokens → provide collateral
  └─ Interest pool accumulates over time

Loan Position (per borrower)
  ├─ Principal amount
  ├─ Collateral (held by contract)
  └─ Status: Active, Repaid, or Liquidated
```

## Basic Flow

### 1. Create Pool
```
Admin creates pool for token:
result = create_pool(tip_token_address)
pool_id = result
```

### 2. Deposit (Lender)
```
Supporter deposits to earn interest:
deposit(pool_id, supporter_address, 1000)
// Supporter now earns interest as pool lends
```

### 3. Borrow (Borrower)
```
Creator borrows against collateral:
loan_id = borrow(pool_id, creator_address, 1000, 1500)
// Receives 1000 tokens, collateral held by contract
// Collateral ratio: 1500/1000 = 150% ✓
```

### 4. Repay (Borrower)
```
Creator repays loan + interest:
repay(loan_id, 1050)
// Collateral returned, loan closed
```

### 5. Withdraw (Lender)
```
Supporter withdraws principal + earned interest:
withdraw(pool_id, supporter_address, 1000)
// Receives 1000 + interest earned over time
```

## Interest Rates

| Pool Utilization | Annual Rate |
|------------------|-------------|
| 0% (no loans) | 5% |
| 25% loans | 16.25% |
| 50% loans | 27.5% |
| 75% loans | 38.75% |
| 100% (full) | 50% |

Rates update automatically as pool fills with loans.

## Collateral Requirements

**Minimum: 150%**
- Borrow $100 → require $150 collateral
- Borrow $1000 → require $1500 collateral

**Liquidation: 110%**
- $100 loan + $150 collateral = 150% (safe)
- $100 loan + $110 collateral = 110% (at threshold)
- $100 loan + $100 collateral = 100% (liquidatable!)

## Common Operations

### Deposit & Earn Interest
```
1. Transfer tokens to contract
2. Call deposit(pool_id, lender, amount)
3. Interest accrues automatically
4. Withdraw anytime to receive principal + interest
```

### Borrow with Collateral
```
1. Transfer collateral to contract
2. Call borrow(pool_id, borrower, amount, collateral)
3. Receive loan amount
4. Interest accumulates each second
5. Repay to get collateral back
```

### Liquidate Unsafe Loan
```
1. Check if loan_collateral < loan_amount * 1.10
2. Call liquidate(loan_id)
3. Collateral becomes pool liquidity
4. Excess returned to borrower
5. Loan closed
```

## Error Conditions

| Error | Cause | Solution |
|-------|-------|----------|
| `InvalidAmount` | Amount ≤ 0 | Use positive amounts |
| `InsufficientCollateral` | Collateral < 150% | Increase collateral |
| `InsufficientLiquidity` | Pool empty | Wait for deposits |
| `InsufficientBalance` | Withdraw > available | Reduce amount |
| `LoanNotActive` | Loan already closed | Check loan status |
| `CannotLiquidate` | Loan is safe | Only liquidate unsafe loans |

## Key Numbers to Remember

| Parameter | Value | Why |
|-----------|-------|-----|
| Min Collateral | 150% | Prevents under-collateralization |
| Liquidation | 110% | Safety buffer for liquidators |
| Min Rate | 5% | Base rate for zero utilization |
| Max Rate | 50% | Cap on maximum interest |
| Interest Calc | Per-second | Precise, no compression attacks |

## Real-World Examples

### Example 1: Lender Strategy
```
1. I have 10,000 tip tokens
2. I deposit 5,000 into the pool (earn 5% APY minimum)
3. Each year, I earn at least 250 tokens in interest
4. I keep 5,000 liquid for other uses
5. At 50% utilization, I earn 27.5% on 5,000 = 1,375 tokens/year
```

### Example 2: Borrower Strategy
```
1. I need 1,000 tip tokens urgently
2. I have 1,500 in a different token
3. I borrow 1,000, post 1,500 collateral
4. I use the 1,000 tokens for my project
5. Over 6 months, interest compounds
6. I repay ~1,075 (1,000 + interest)
7. Get my 1,500 collateral back
```

### Example 3: Liquidation
```
1. Loan created: 500 tokens, 750 collateral (150%)
2. Price of collateral drops (oracle update)
3. Collateral now worth only 600 (120% ratio)
4. Still safe, no liquidation

5. Collateral drops further to 520 (104% ratio)
6. Below 110% threshold: LIQUIDATABLE
7. Anyone calls liquidate()
8. 550 collateral becomes pool liquidity (at 110% threshold)
9. Borrower gets back 30 excess
10. Loan closed
```

## Security Notes

1. **Your collateral is safe** - only liquidated if < 110% of loan
2. **Interest is fair** - capped at 50% max, adjusts with utilization
3. **No hidden fees** - only interest, no admin cuts
4. **Contract holds all assets** - not borrower custody
5. **Instant liquidation** - undercollateralized loans closed quickly

## Testing the Protocol

Run all lending tests:
```bash
cargo test --test lending_tests
```

Tests cover:
- ✅ Pool creation
- ✅ Deposit/withdraw
- ✅ Borrow/repay
- ✅ Liquidation
- ✅ Interest calculations
- ✅ Edge cases

## Integration Checklist

- [ ] Import lending module: `use tipjar::lending::{pool, loan}`
- [ ] Create pool for tip token: `pool::create_pool(env, token)`
- [ ] Expose pool functions in contract
- [ ] Expose loan functions in contract
- [ ] Add lending UI components
- [ ] Test with testnet tokens
- [ ] Deploy to mainnet

## Support & Documentation

- **Full docs**: See `LENDING_PROTOCOL.md`
- **API reference**: See `LENDING_PROTOCOL.md#API-Reference`
- **Implementation**: See `IMPLEMENTATION_SUMMARY.md`
- **Examples**: See `contracts/tipjar/examples/lending_example.rs`
- **Tests**: See `contracts/tipjar/tests/lending_tests.rs`
