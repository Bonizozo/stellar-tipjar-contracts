# Social Recovery Quick Start Guide

## Overview

Social recovery allows TipJar creators to recover account access through a trusted network of guardians. This guide shows basic usage patterns.

## Basic Setup

### 1. Initialize Recovery (Creator)

```javascript
// Call once per creator account
await contract.recovery_init({
  creator: creatorAddress
});
```

### 2. Add Guardians (Creator Only)

```javascript
// Add trusted guardians (3 is recommended)
await contract.recovery_add_guardian({
  creator: creatorAddress,
  guardian: guardianAddress1,
  weight: 1  // voting weight
});

await contract.recovery_add_guardian({
  creator: creatorAddress,
  guardian: guardianAddress2,
  weight: 1
});

await contract.recovery_add_guardian({
  creator: creatorAddress,
  guardian: guardianAddress3,
  weight: 1
});
```

## Recovery Process

### 1. Create Recovery Request (Creator Lost Access)

```javascript
// Initiate recovery to new account
const newOwnerAddress = "GXXXXXX..."; // creator's new account

await contract.recovery_create_request({
  creator: creatorAddress,
  new_owner: newOwnerAddress
});

// Query the request ID
const request = await contract.recovery_get_request({
  request_id: 1
});
```

### 2. Guardians Vote (Guardian Action)

```javascript
// Each guardian approves the recovery
await contract.recovery_approve({
  request_id: 1
});
```

**Example with 3 guardians:**
- Guardian 1 approves (1/3 = 33%)
- Guardian 2 approves (2/3 = 66%) → **THRESHOLD MET** ✓
- Request automatically enters "Locked" state
- Guardian 3 can still approve (optional)

### 3. Wait for Timelock

After threshold is met:
- Request enters "Locked" state
- 7-day timelock begins
- Creator cannot do anything (recovery is in progress)

### 4. Execute Recovery (After Timelock)

```javascript
// Execute after 7 days have passed
const recoveredOwner = await contract.recovery_execute({
  request_id: 1
});

// Returns the new owner address
console.log(recoveredOwner); // GXXXXXX...
```

## Guardian Management

### Revoke a Guardian (Creator Only)

```javascript
// Start revocation process
await contract.recovery_revoke_guardian({
  creator: creatorAddress,
  guardian: guardianAddress
});

// Revocation becomes effective after 1 day
// During the 1-day delay, you can re-add to cancel
```

### Add Replacement Guardian

```javascript
await contract.recovery_add_guardian({
  creator: creatorAddress,
  guardian: newGuardianAddress,
  weight: 1
});
```

## Querying Information

### Get Recovery Request Details

```javascript
const request = await contract.recovery_get_request({
  request_id: 1
});

// Returns:
// {
//   id: 1,
//   creator: "G...",
//   new_owner: "G...",
//   status: "Locked",  // Voting, Locked, Executed, Rejected
//   approval_count: 2,
//   created_at: 1234567890,
//   executed_at: 0,
//   timelock_end: 1234567890 + 604800
// }
```

### Get Recent Attempts (Rate Limiting Check)

```javascript
const attemptIds = await contract.recovery_get_recent_attempts({
  creator: creatorAddress,
  since_timestamp: 0  // Unix timestamp
});

// Returns array of request IDs created after timestamp
```

## Recommended Practices

### 1. Choose Trusted Guardians
- Family members or close friends
- Different geographic locations (harder to compromise all)
- Technically literate (can understand voting)
- Long-term relationships

### 2. Communicate With Guardians
- Explain the role and responsibility
- Share contact info for coordination
- Have a backup communication plan
- Agree on voting criteria

### 3. Secure Your Backup
- Save new owner addresses in secure storage
- Document guardian setup
- Update recovery plan if guardians change
- Test with a dummy account first

### 4. Monitor Recovery Attempts
- Review `recovery_get_recent_attempts()` periodically
- If you see unexpected attempts, investigate
- Contact guardians if suspicious activity detected

### 5. Regular Maintenance
- Review and rotate guardians periodically
- Revoke inactive guardians
- Add new guardians if you don't trust current set
- Test recovery process annually

## Example Scenario

### Alice's Recovery Setup

**Step 1: Setup (Day 1)**
```
Alice calls recovery_init()
Alice adds guardians: Bob, Charlie, Diana (weight 1 each)
```

**Step 2: Lost Access (Day 30)**
```
Alice loses her private key
Alice creates recovery request to new account (from backup)
```

**Step 3: Guardian Voting (Day 31)**
```
Bob receives notification, reviews request, approves
Charlie approves
Threshold met! Request → "Locked" state
Diana also approves (optional)
```

**Step 4: Timelock (Days 31-38)**
```
Wait 7 days
Alice's new account is ready
```

**Step 5: Execution (Day 38)**
```
Anyone can call recovery_execute()
Alice recovers account with new_owner address
Alice transfers tips to another account if needed
```

## Security Considerations

### ✅ Safe Practices
- 66% guardian consensus prevents takeover
- 7-day timelock allows response time
- 1-day guardian revocation delay prevents accidents
- Double-voting is impossible

### ⚠️ Risks to Be Aware Of
- All 3 guardians compromised = account takeover
- Lost contact with all guardians = cannot recover
- Guardians gossip about your recovery = reduced privacy
- Timelock is fixed (cannot be shortened in emergency)

### 🛡️ Mitigations
- Choose geographically dispersed guardians
- Maintain backup recovery methods
- Don't add malicious addresses as guardians
- Review approval requests carefully

## Troubleshooting

### "No guardians set for recovery"
- You haven't added any guardians yet
- **Fix**: Call `recovery_add_guardian()` first

### "Guardian already exists"
- That address is already a guardian
- **Fix**: Use different address or revoke first

### "Guardian already approved"
- You already voted on this request
- **Fix**: Wait for other guardians or start new request

### "Timelock not expired"
- Less than 7 days have passed since threshold
- **Fix**: Wait until timelock_end timestamp

### "Not an active guardian"
- You're not a guardian or you've been revoked
- **Fix**: Ask creator to re-add you

## Advanced: Custom Thresholds

The default configuration uses:
- **66% approval threshold** (2 of 3 guardians)
- **7-day timelock**
- **1-day revocation delay**

To modify these, edit `RecoveryConfig::default()` in `recovery.rs`:

```rust
impl RecoveryConfig {
    pub fn default() -> Self {
        RecoveryConfig {
            approval_threshold: 66,  // ← change this
            timelock_delay: 604800,  // ← change this
            guardian_revocation_delay: 86400,  // ← change this
        }
    }
}
```

## Event Monitoring

Subscribe to recovery events:

```javascript
// Listen for recovery events
contract.on("recovery:request_created", (creator, requestId, newOwner) => {
  console.log(`Recovery initiated: ${creator}`);
});

contract.on("recovery:approved", (requestId, guardian) => {
  console.log(`Guardian approved: ${guardian}`);
});

contract.on("recovery:threshold_reached", (creator, requestId) => {
  console.log(`Recovery locked in: ${creator}`);
});

contract.on("recovery:executed", (creator, requestId, newOwner) => {
  console.log(`Recovery completed: ${creator}`);
});
```

## FAQ

**Q: Can I change guardians after setup?**
A: Yes, revoke old guardians and add new ones.

**Q: What if I need to recover immediately (< 7 days)?**
A: Not possible by design. Timelock is security feature. Plan ahead.

**Q: Can a guardian see my private tips?**
A: No, guardians only see that recovery was initiated. They don't see financial details.

**Q: What if I change my mind during recovery?**
A: Before timelock expires, create a new request to a different address. Timelock applies per request.

**Q: Can I have weighted guardians (some worth more)?**
A: Yes, set different `weight` values when adding guardians.

**Q: How many guardians should I have?**
A: Recommend 3-5. More guardians = harder to compromise, but harder to coordinate.

## Resources

- **Full Documentation**: See `RECOVERY.md`
- **Technical Spec**: See `contracts/tipjar/src/recovery.rs`
- **Test Suite**: See `tests/recovery_tests.rs`
- **Changelog**: See `CHANGELOG_RECOVERY.md`
