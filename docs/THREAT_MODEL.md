# Threat Model - TipJar Contract

## Asset Identification

- **Escrowed Tokens**: Creator balances held in contract storage
- **Admin Privileges**: Token whitelisting, role management, pause functionality
- **User Funds**: Sender tokens during tip transactions

## Threat Categories

### 1. Reentrancy Attacks
**Threat**: Malicious token contract calls back into TipJar during transfer
**Mitigation**: 
- State updates before external calls (checks-effects-interactions pattern)
- Reentrancy guard on critical functions
- Soroban's single-threaded execution model limits reentrancy risk

### 2. Integer Overflow/Underflow
**Threat**: Arithmetic operations exceed i128 bounds
**Mitigation**:
- Use checked arithmetic operations
- Validate amounts before operations
- Enforce MAX_TIP_AMOUNT limits

### 3. Access Control Bypass
**Threat**: Unauthorized users execute privileged operations
**Mitigation**:
- Mandatory `require_auth()` on all state-changing functions
- Role-based access control (Admin, Moderator, Creator)
- Admin key protection

### 4. Token Whitelisting Bypass
**Threat**: Malicious tokens accepted for tips
**Mitigation**:
- Explicit token whitelist maintained by admin
- All tips must use whitelisted tokens
- Regular audit of whitelisted tokens

### 5. Locked Tip Bypass
**Threat**: Early withdrawal of locked tips
**Mitigation**:
- Timestamp validation enforced at withdrawal
- Ledger time comparison prevents premature access

### 6. Batch Operation Abuse
**Threat**: Large batch operations cause DoS or resource exhaustion
**Mitigation**:
- Batch size limits enforced
- Per-operation gas cost tracking

### 7. Emergency Pause Misuse
**Threat**: Unauthorized pause or prolonged pause
**Mitigation**:
- Only Admin/Moderator can pause
- Pause state is queryable
- Clear unpause procedures

## Risk Levels

| Threat | Severity | Likelihood | Mitigation Status |
|--------|----------|------------|-------------------|
| Reentrancy | High | Low | Implemented |
| Integer Overflow | High | Low | Implemented |
| Access Control Bypass | Critical | Low | Implemented |
| Token Whitelisting Bypass | High | Low | Implemented |
| Locked Tip Bypass | Medium | Low | Implemented |
| Batch DoS | Medium | Medium | Implemented |
| Pause Misuse | Medium | Low | Implemented |

## Assumptions

1. Soroban runtime prevents arbitrary code execution
2. Stellar network consensus is secure
3. Admin key is properly protected
4. Token contracts follow Stellar standards
5. Ledger timestamps are accurate and monotonic
