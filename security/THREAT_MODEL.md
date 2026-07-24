# TipJar Security Threat Model

## Executive Summary

This document outlines the TipJar contract's trust assumptions, security invariants, and accepted risks. It complements the security properties documented in `docs/SECURITY.md` and is essential reading for integrators and auditors.

---

## Trust Assumptions

### 1. Token Contracts (Whitelisted Only)

**Assumption**: Every whitelisted token correctly implements [SEP-41](https://github.com/stellar/rs-soroban-sdk) token semantics.

**What "correct" means**:
- `transfer(from, to, amount)` moves exactly `amount` tokens from `from` to `to` on success
- Return success only if the transfer actually occurred
- If the token panics, the entire transaction rolls back (Soroban atomicity)
- Do not attempt same-contract reentrancy

**Why this matters**: The TipJar contract trusts the transfer result without independently verifying balances. A misbehaving token can:
- **Panic**: Rolls back the entire transaction (safe but DoS vector)
- **Silent No-Op**: Return success without moving tokens (balance ledger becomes incorrect)
- **Burn Amount**: Transfer less than requested (creator withdrawal fails later)
- **Reenter**: Exploit call ordering (mitigated by checks-effects-interactions and host prohibition)

**Mitigation**:
1. **Allowlist enforcement**: Only the Admin can whitelist tokens via `add_token()`. This forces a manual audit step.
2. **Checks-effects-interactions pattern**: All state mutations (balance updates) occur before calling the token transfer, ensuring storage consistency even if the token panics.
3. **Atomic transactions**: Soroban's transaction model rolls back all storage on revert, preventing partial-state scenarios.

**Accepted Risk**: If a whitelisted token is **silently malicious** (accepts transfers but doesn't move funds), there is no client-side or contract-side mechanism to detect it. The only prevention is rigorous auditing of tokens before whitelisting.

### 2. Admin Key (Contract Initialization)

**Assumption**: The Admin key is protected and never compromised.

**Admin capabilities**:
- `add_token()`: Whitelist new tokens (fundamental security gate)
- `remove_token()`: Delist a malicious or compromised token
- `grant_role()`: Assign Creator, Moderator, and Admin roles
- `revoke_role()`: Remove role assignments
- `pause()`: Emergency pause all state-changing operations
- `approve_refund()`: Approve refund requests past the grace period

**Impact of compromise**: An attacker controlling the Admin key can:
- Whitelist a malicious token, enabling theft from all creators
- Revoke Creator roles, freezing withdrawals
- Pause the contract indefinitely
- Approve refunds for arbitrary tips (returning funds to attackers)

**Mitigation**:
1. **Hardware security module (HSM) or multisig**: Store the admin key in a hardware wallet or require multisig approval for privileged operations.
2. **Monitoring**: Log all `grant_role`, `add_token`, and `remove_token` calls; alert on unexpected changes.
3. **Timelocks**: For mainnet deployments, require a timelock (e.g., 48-hour delay) before token removals take effect, allowing creators to withdraw first.

### 3. Guardian (Emergency Pause)

**Assumption**: At least one authorized Admin or Moderator will invoke `pause()` if a vulnerability is discovered.

**Impact of bypass**: Without pause capability, a zero-day exploit could drain all escrowed balances before the team can respond.

**Mitigation**:
1. **Runbook**: Document exactly who can pause and under what conditions.
2. **Monitoring**: Alert on suspicious patterns (e.g., bulk withdrawals from cold addresses).
3. **Incident response**: Have a pre-signed pause transaction ready for rapid deployment.

---

## Storage Invariants

All of the following must remain true after every successful transaction:

### Invariant 1: Balance Accounting

**For every creator and whitelisted token**:
```
CreatorBalance(creator, token) >= 0
CreatorBalance(creator, token) <= CreatorTotal(creator, token)
```

**Why**: CreatorBalance is withdrawn amount is always non-negative and never exceeds total received.

**Violations and causes**:
- Underflow: Impossible (Soroban uses `i128`, all arithmetic is saturating or checked)
- Overflow: Impossible (saturating arithmetic caps at `i128::MAX`)
- Unauthorized write: Possible if RBAC is bypassed (mitigated by `require_auth()`)
- Misbehaving token: Does not violate this invariant directly; if a token panics, the entire transaction rolls back

### Invariant 2: Total Tips Never Decrease

**For every creator and whitelisted token**:
```
CreatorTotal(creator, token) only increases (never decreases)
```

**Why**: This tracks cumulative historical tips; refunds should not reduce the historical total.

**Current status**: ✅ Enforced by `tip()` and `tip_with_message()` which only add to the total.

### Invariant 3: Authorization Gate

**For every state-changing call**:
1. The relevant address must have called `require_auth()` (on-chain signature verification)
2. The address must hold the required role (stored in `UserRole(address)`)

**Example**:
- `tip(sender, creator, ...)` requires `sender.require_auth()` (no role check)
- `withdraw(creator, ...)` requires `creator.require_auth()` AND `Creator` role
- `add_token(admin, ...)` requires `admin.require_auth()` AND `Admin` role

**Violations and causes**:
- Missing `require_auth()`: Possible if the contract code is patched (mitigated by upgrade procedure)
- RBAC bypass: Possible if `grant_role()` is miscalled (all role changes are admin-only, so this requires admin compromise)

### Invariant 4: Locked Tips Remain Locked

**For every LockedTip with `unlock_timestamp`**:
```
LockedTip(creator, tip_id).unlock_timestamp > current_ledger_timestamp
  => withdraw_locked(creator, tip_id) must panic with TipStillLocked
```

**Why**: Time-locked tips enforce a commitment period before withdrawal.

**Current status**: ✅ Enforced by `withdraw_locked()` which checks `unlock_timestamp > env.ledger().timestamp()`.

### Invariant 5: Whitelisted Token Gate

**For every `tip()` call**:
```
token must be in TokenWhitelist
  => tip succeeds
token is NOT in TokenWhitelist
  => tip panics with TokenNotWhitelisted
```

**Why**: Only pre-approved tokens can be used, preventing griefing with junk tokens.

**Current status**: ✅ Enforced by `Self::is_whitelisted()` check before transfer.

---

## Checks-Effects-Interactions Pattern

The TipJar enforces strict ordering of checks, state mutations, and external calls to maximize resistance against reentrancy and partial-state exploits:

### `tip()` Order

1. **Checks** ✅
   - Contract not paused
   - Amount > 0
   - Token is whitelisted
   - Sender has authorized this call (`sender.require_auth()`)

2. **Effects** ✅
   - Update storage: `CreatorBalance(creator, token) += amount`
   - Update storage: `CreatorTotal(creator, token) += amount`
   - Record tip for refund tracking

3. **Interactions** ✅
   - Call `token.transfer(sender, contract_address, amount)`
   - Emit events

**Consequence**: If the token transfer panics, the contract storage is rolled back atomically by Soroban. The creator's balance was never actually updated at the contract level.

### `withdraw()` Order

1. **Checks** ✅
   - Contract not paused
   - Creator has authorized this call (`creator.require_auth()`)
   - Creator holds the `Creator` role
   - Balance > 0

2. **Effects** ✅
   - Update storage: `CreatorBalance(creator, token) = 0`

3. **Interactions** ✅
   - Call `token.transfer(contract_address, creator, amount)`
   - Emit events

**Consequence**: The balance is zeroed BEFORE the token transfer. Even if the token panics, the storage state is consistent (balance was already zeroed). When the transaction rolls back, the balance is restored automatically by Soroban, maintaining the invariant.

---

## Accepted Risks

### Risk 1: Silent Token Malfunction

**Scenario**: A token is whitelisted and then goes malicious, silently accepting transfers but not moving funds.

**Impact**: Creators' balances appear to grow, but when they try to withdraw, the token transfer fails with insufficient balance. The creator's stored balance is now unreachable.

**Probability**: Low (requires token maintainer to turn malicious after deployment)

**Mitigation**:
- Require periodic audits of whitelisted tokens
- Provide `remove_token()` to delist a broken token (though this doesn't recover lost balance)
- Document this risk in the allowlist process

**Acceptance rationale**: This is an inherent property of the SEP-41 trust model. Trusting a token contract requires trusting its maintainer. There is no client-side or contract-side mechanism to verify a token's behavior without inspecting its code or running it against known-good test cases.

### Risk 2: Reentrancy Under Future Host Changes

**Scenario**: A future version of Soroban allows same-contract reentrancy (relaxes the current prohibition).

**Impact**: A malicious token could call `tip()` or `withdraw()` recursively during a transfer, potentially exploiting timing-dependent logic or iterator invalidation.

**Current status**: Soroban's current host prohibits same-contract reentrancy, so this is not exploitable today. The `security/` test suite includes a reentry test that will fail if this changes, triggering a code review.

**Mitigation**:
- All state mutations occur before external calls (checks-effects-interactions), minimizing reentrancy surface
- The test suite will alert us immediately if the host behavior changes
- If reentry is permitted in the future, we can add reentrancy guards (e.g., mutexes) if needed

**Acceptance rationale**: Guarding against hypothetical future host changes is economically unjustifiable today. The test suite ensures we detect the change and can respond quickly.

### Risk 3: Arithmetic Overflow Under Extreme Scenarios

**Scenario**: A creator accumulates tips totaling i128::MAX, then receives another tip.

**Impact**: The `checked_add` operation would either saturate or panic, depending on implementation. If it panics, the transaction rolls back and the creator's balance remains unchanged.

**Mitigation**:
- Soroban's arithmetic uses checked operations that panic on overflow
- Saturation is not used for balance operations (which would hide overflow)
- The test suite includes a `test_i128_max_adjacent_balance` case to verify this behavior

**Acceptance rationale**: The probability of accumulating i128::MAX tokens (2^126 units) is negligible in practice. If it occurs, the safe behavior (panic) prevents silent data corruption.

### Risk 4: Unauthorized Refund Approval

**Scenario**: An admin key is compromised and used to approve refunds for arbitrary tips.

**Impact**: Legitimate creator funds are returned to attackers.

**Mitigation**:
- Store the admin key in a hardware security module (HSM)
- Require multisig approval for refund operations on mainnet
- Monitor all `approve_refund()` calls via event logs

**Acceptance rationale**: This risk is inherent to all smart contracts with administrative privileges. Hardware security and process controls are the only practical mitigations.

---

## Testing Strategy

The `security/` workspace contains an adversarial token contract that simulates the four major failure modes:

1. **Panic on Transfer** (`mode = 1`): Reverts the entire transaction
2. **Silent No-Op** (`mode = 2`): Returns success without moving funds
3. **Reentry Attempt** (`mode = 3`): Tries to call `tip()` during transfer (documents host behavior)
4. **Amount Burn** (`mode = 4`): Transfers less than requested

For each mode, the test suite verifies:
- Storage invariants are maintained or transactions are fully rolled back
- Checks-effects-interactions ordering is correct
- Authorization requirements are enforced
- Arithmetic edge cases are handled safely

Run the security test suite:

```bash
cargo test -p security -- --nocapture
```

---

## Recommendations for Integrators

### Before Mainnet Deployment

1. **Audit the Admin Key**: Use a multisig wallet, HSM, or governance contract to control the admin key. Never use a single unencrypted private key.
2. **Establish a Token Allowlist Process**: Document how new tokens are vetted before whitelisting. Require code review, test deployments, and community feedback.
3. **Plan for Emergencies**: Pre-sign a pause transaction. Assign clear escalation procedures for who can invoke pause and under what conditions.
4. **Monitor Balances**: Track off-chain the total amount escrowed by the contract. Alert if any creator's balance unexpectedly increases or if large withdrawals fail.

### During Maintenance

1. **Regularly Review Whitelisted Tokens**: Ensure token maintainers are still active and the code hasn't changed suspiciously.
2. **Test Withdrawals**: Periodically withdraw a test amount to verify the token transfer still works.
3. **Log Everything**: Emit events for all role changes and token operations. Send events to a centralized logging service.

### After Any Vulnerability Report

1. **Reproduce**: Run the security test suite with the reported scenario.
2. **Pause**: If confirmed, invoke `pause()` immediately.
3. **Investigate**: Review storage snapshots and event logs to understand the impact.
4. **Fix & Re-audit**: Update the contract code and re-run the security suite.
5. **Upgrade**: Deploy the patched contract WASM via the `upgrade()` function.
6. **Unpause**: After confidence is restored, invoke `unpause()`.

---

## Glossary

- **CEI Pattern**: Checks-Effects-Interactions. A design principle where checks come first, state mutations second, and external calls last.
- **SEP-41**: Stellar Enhancement Proposal for token contract interface.
- **Atomicity**: A transaction either fully succeeds or fully reverts; no partial state is observable.
- **Whitelisting**: An allowlist of tokens explicitly approved by the admin.
- **Reentrancy**: A scenario where a contract calls another contract which calls the first contract back, possibly with state in flux.
- **TTL**: Time to Live. Soroban's ledger entry expiry mechanism.

---

## References

- [Soroban Official Docs](https://developers.stellar.org/docs/learn/smart-contracts)
- [SEP-41 Standard](https://github.com/stellar/stellar-protocol/blob/master/core/cap-0046-01.md)
- [Stellar Asset Contract](https://developers.stellar.org/docs/learn/smart-contracts/stellar-asset-contract)
- [OWASP Smart Contract Security](https://owasp.org/www-community/attacks/reentrancy)
