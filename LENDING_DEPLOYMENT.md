# Lending Protocol Deployment Guide

## Pre-Deployment Checklist

### Code Review
- [ ] Audit collateral ratio calculations
- [ ] Verify interest accrual formulas
- [ ] Test liquidation conditions
- [ ] Review storage access patterns
- [ ] Verify error handling coverage
- [ ] Check token transfer safety

### Testing
- [ ] Run full test suite: `cargo test --test lending_tests`
- [ ] Test on Stellar testnet with real tokens
- [ ] Simulate high-utilization scenarios
- [ ] Test liquidation edge cases
- [ ] Verify interest calculations over time
- [ ] Test concurrent operations

### Documentation
- [ ] Update main README with lending feature
- [ ] Add lending to CONTRIBUTING.md guidelines
- [ ] Document all error codes
- [ ] Create troubleshooting guide
- [ ] Add FAQ section

### Security
- [ ] Review collateral validation logic
- [ ] Verify no reentrancy vulnerabilities
- [ ] Check integer overflow protection
- [ ] Validate authorization checks
- [ ] Test all error paths

## Deployment Steps

### 1. Prepare Production Build
```bash
# Build with release optimizations
cargo build -p tipjar --target wasm32-unknown-unknown --release

# Verify build output
ls -lh target/wasm32-unknown-unknown/release/tipjar.wasm
```

### 2. Initialize Lending System
```bash
# On Testnet first
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- create_pool \
  --token <TOKEN_ADDRESS>
```

### 3. Deploy to Testnet
```bash
# Deploy contract
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/tipjar.wasm \
  --network testnet

# Note the contract ID for testing
```

### 4. Testnet Validation
```bash
# Create test pool
stellar contract invoke --id <ID> --network testnet -- create_pool --token <TOKEN>

# Test deposit
stellar contract invoke --id <ID> --network testnet -- deposit --pool 1 --amount 1000

# Test borrow
stellar contract invoke --id <ID> --network testnet -- borrow --pool 1 --amount 500 --collateral 750

# Test repay
stellar contract invoke --id <ID> --network testnet -- repay --loan 1 --amount 550
```

### 5. Monitor Testnet
- [ ] Monitor pool creation
- [ ] Track deposit/withdrawal flows
- [ ] Observe interest calculations
- [ ] Test liquidation scenarios
- [ ] Verify error handling

### 6. Production Deployment
```bash
# After successful testnet testing

# Deploy mainnet contract
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/tipjar.wasm \
  --network public

# Record contract ID and time
```

### 7. Initialize Mainnet Pools
```bash
# Create pools for each supported tip token
stellar contract invoke \
  --id <MAINNET_CONTRACT_ID> \
  --network public \
  -- create_pool \
  --token <TIP_TOKEN_ADDRESS>
```

## Configuration

### Protocol Parameters (in `lending/mod.rs`)

```rust
const COLLATERAL_RATIO: i128 = 150;      // 150% minimum
const LIQUIDATION_BONUS: i128 = 110;     // 110% threshold
const BASE_INTEREST_RATE: u32 = 5000;    // 5% annual
const MAX_INTEREST_RATE: u32 = 50000;    // 50% annual
```

### To Adjust Parameters:
1. Edit constants in `lending/mod.rs`
2. Update interest.rs calculation functions
3. Rebuild and redeploy
4. Update documentation

## Monitoring & Maintenance

### Key Metrics to Track

```
Pool Health:
- Total liquidity per pool
- Total borrowed per pool
- Utilization ratio (borrowed / (borrowed + liquidity))
- Average interest rate
- Number of active loans

Risk Metrics:
- Number of underwater loans (< 110% collateral)
- Liquidations per day
- Average time to liquidation
- Liquidation success rate

User Metrics:
- Active lenders per pool
- Active borrowers per pool
- Average deposit size
- Average loan size
- Repayment rate
```

### Monitoring Queries

```bash
# Get pool state
stellar contract invoke --id <ID> --network public -- get_pool_state --pool 1

# Get interest rate
stellar contract invoke --id <ID> --network public -- get_pool_interest_rate --pool 1

# Get borrower loans
stellar contract invoke --id <ID> --network public -- get_borrower_loans --borrower <ADDRESS>
```

### Alert Thresholds

| Condition | Threshold | Action |
|-----------|-----------|--------|
| High Utilization | > 80% | Monitor closely |
| Low Liquidity | < $10k | Warning |
| Liquidatable Loans | > 5% | Investigate |
| Interest Rate | > 40% | Very high risk |
| Failed Transactions | > 1% | Check contract |

## Incident Response

### If Liquidation Fails
1. Check loan status
2. Verify collateral amount
3. Confirm threshold calculation
4. Manual liquidation if needed
5. Log incident for analysis

### If Interest Calculation Errors
1. Verify timestamp accuracy
2. Check rate calculation
3. Validate per-second accrual
4. Reset interest if needed
5. Notify affected users

### If Pool Becomes Insolvent
1. Pause new borrowing
2. Prioritize repayments
3. Liquidate all safe positions
4. Investigate root cause
5. Deploy fix

## Rollback Plan

If critical issues found:

```bash
# Stop operations
stellar contract invoke --id <ID> --network public -- pause

# Deploy previous version if needed
stellar contract deploy \
  --wasm <PREVIOUS_WASM> \
  --network public \
  --id <ID>

# Recover state if possible
# Contact Stellar support for assistance
```

## Upgrade Path

Future upgrades:

1. **V2 Features**: Oracle integration, multi-collateral
2. **V3 Features**: Flash loans, auctions, governance
3. **V4+**: Advanced strategies, staking, cross-chain

Upgrade process:
1. Test thoroughly on testnet
2. Create upgrade proposal
3. Community governance approval
4. Deploy new contract or upgrade existing
5. Migrate liquidity if needed

## Documentation Updates

After deployment, update:

- [ ] README.md with lending info
- [ ] API documentation
- [ ] Testnet addresses
- [ ] Mainnet addresses
- [ ] Rate limits and constraints
- [ ] Troubleshooting guide
- [ ] FAQ section
- [ ] User guides

## Post-Launch Support

### First Month
- [ ] Daily monitoring of operations
- [ ] Weekly performance reports
- [ ] Quick response to issues
- [ ] Community feedback collection

### Months 2-3
- [ ] Weekly monitoring
- [ ] Monthly reports
- [ ] Bug fixes as needed
- [ ] Feature requests collection

### Ongoing
- [ ] Monthly health checks
- [ ] Parameter optimization
- [ ] Security audits
- [ ] Feature planning

## Success Criteria

After deployment, success is measured by:

- ✅ > 90% uptime
- ✅ < 1% transaction failure rate
- ✅ Interest calculations accurate to 0.01%
- ✅ Liquidations execute within 1 block
- ✅ User satisfaction > 4.5/5
- ✅ No critical security issues
- ✅ TVL growth > 10% per month
- ✅ Utilization rate 40-60%

## Contacts & Escalation

### Technical Issues
- Smart contract team
- Stellar developer support

### Security Issues
- Security team
- Stellar security committee

### Community Issues
- Community manager
- Developer relations

## Emergency Contacts

| Role | Contact |
|------|---------|
| Contract Admin | [address] |
| Technical Lead | [contact] |
| Security Lead | [contact] |
| Community Lead | [contact] |

## Resources

- Stellar Documentation: https://developers.stellar.org/
- Soroban Docs: https://soroban.stellar.org/
- Token Interface: https://github.com/stellar/rs-soroban-sdk/
- Contract Examples: https://github.com/stellar/soroban-examples/

## Sign-Off

- [ ] Technical review approved
- [ ] Security review approved
- [ ] Legal review approved
- [ ] Community input received
- [ ] Ready for deployment

**Deployment Date:** ________________

**Deployed By:** ________________

**Testnet Address:** ________________

**Mainnet Address:** ________________
