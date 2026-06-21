# Social Recovery Mechanism

## Overview

The Stellar Tip Jar contracts now include a social recovery mechanism that allows account owners to recover access to their creator accounts through a network of trusted guardians. This feature provides enhanced security by enabling account recovery without relying solely on private key management.

## Architecture

### Core Components

#### 1. **Guardian System**
- Creators can designate trusted guardians (family members, friends, other creators)
- Each guardian has a weight (voting power) - typically 1 per guardian
- Guardians are tracked in contract storage with metadata:
  - Guardian address
  - Voting weight
  - Addition timestamp
  - Revocation effective time (0 = active)

#### 2. **Recovery Request**
A recovery request represents a single recovery attempt with:
- Unique request ID (auto-incrementing)
- Creator seeking recovery
- Target new owner address
- Status (Voting → Locked → Executed/Rejected)
- Guardian approval count
- Timelock expiration timestamp

#### 3. **Multi-Signature Voting**
- Guardians vote to approve recovery requests
- 66% approval threshold required (configurable)
- Each guardian can only vote once per request
- Voting phase continues until threshold is met

#### 4. **Timelock Mechanism**
- After voting threshold is met, request enters "Locked" state
- 7-day timelock delay before execution (configurable)
- Prevents immediate account takeover
- Allows creator to contest recovery request

#### 5. **Attempt Tracking & Rate Limiting**
- All recovery attempts recorded with timestamps
- Query historical attempts to detect attack patterns
- Rate limiting enforced at application level

## Data Model

### Storage Keys

```rust
DataKey::RecoveryGuardians(Address)      // Vec<Guardian> keyed by creator
DataKey::RecoveryRequest(u64)             // RecoveryRequest keyed by request ID
DataKey::RecoveryApproval(u64, Address)   // bool keyed by (request_id, guardian)
DataKey::RecoveryAttempts(Address)        // Vec<RecoveryAttempt> keyed by creator
DataKey::RecoveryCounter                  // u64 global counter for request IDs
```

### Data Structures

```rust
pub enum RecoveryStatus {
    Voting,    // Guardians voting
    Locked,    // Timelock active
    Executed,  // Recovery completed
    Rejected,  // Recovery failed
}

pub struct Guardian {
    pub address: Address,
    pub weight: u32,
    pub added_at: u64,
    pub revocation_time: u64,  // 0 = active, >0 = revoked
}

pub struct RecoveryRequest {
    pub id: u64,
    pub creator: Address,
    pub new_owner: Address,
    pub status: RecoveryStatus,
    pub approval_count: u32,
    pub created_at: u64,
    pub executed_at: u64,
    pub timelock_end: u64,
}

pub struct RecoveryAttempt {
    pub request_id: u64,
    pub timestamp: u64,
}
```

## Configuration

Default recovery configuration:

```rust
pub struct RecoveryConfig {
    pub approval_threshold: u32 = 66,              // 66% of guardian weight
    pub timelock_delay: u64 = 604800,              // 7 days in seconds
    pub guardian_revocation_delay: u64 = 86400,    // 1 day in seconds
}
```

## API Reference

### Public Methods

#### `recovery_init(creator: Address)`
Initialize recovery system for a creator.
- Sets up empty guardian list and attempt tracking
- **Authorization**: Any account (creator setup is permissionless)

#### `recovery_add_guardian(creator: Address, guardian: Address, weight: u32)`
Add a trusted guardian.
- Only callable by creator
- Guardian cannot already exist (or must be fully revoked)
- Weight typically 1, can vary for weighted voting
- **Authorization**: Creator only

#### `recovery_revoke_guardian(creator: Address, guardian: Address)`
Initiate guardian revocation with delay.
- Revocation becomes effective after 1-day delay
- Prevents accidental locks from revocation
- Can be reversed by re-adding guardian before delay expires
- **Authorization**: Creator only

#### `recovery_create_request(creator: Address, new_owner: Address)`
Create a recovery request to claim new owner address.
- Creator must exist
- At least one active guardian required
- Records attempt for rate limiting
- Emits `("recovery", "request_created")` event
- **Authorization**: Creator only

#### `recovery_approve(request_id: u64)`
Guardian approves a recovery request.
- Only active guardians can approve
- Each guardian votes once per request
- Automatically transitions to Locked state if threshold met
- Emits `("recovery", "approved")` event
- **Authorization**: Active guardian only

#### `recovery_execute(request_id: u64) -> Address`
Execute recovery after timelock expires.
- Request must be in Locked state
- Current time must exceed timelock_end
- Returns the new owner address
- Emits `("recovery", "executed")` event
- **Authorization**: Any account (time-based access)

#### `recovery_get_request(request_id: u64) -> RecoveryRequest`
Query recovery request details (read-only).

#### `recovery_get_recent_attempts(creator: Address, since_timestamp: u64) -> Vec<u64>`
Get recovery request IDs since timestamp (read-only).
- Used for rate limiting analysis
- Returns request IDs matching criteria

## Security Features

### 1. **Multi-Signature Requirement**
- 66% guardian consensus prevents single-point-of-failure
- Difficult for attacker to compromise majority of guardians

### 2. **Timelock Delay**
- 7-day window allows creator to notice and respond
- Can be extended by application layer if needed
- Provides time for guardian coordination

### 3. **Guardian Revocation Delay**
- 1-day delay prevents accidental revocation
- Allows creator to cancel if made in error
- Limits attacker's ability to quickly remove guardians

### 4. **Approval Verification**
- Each guardian can only vote once per request
- Active guardian status verified at approval time
- Prevents voting by revoked guardians

### 5. **Attempt Tracking**
- Historical record of recovery attempts
- Can detect coordinated attacks
- Enables off-chain rate limiting policies

### 6. **Authorization Checks**
- Creator can only add guardians and create requests
- Non-creators cannot initiate recovery
- Guardians independently approve requests

## Event Emissions

The recovery system emits events for transparency:

```rust
("recovery", "request_created") → (creator: Address, request_id: u64, new_owner: Address)
("recovery", "approved") → (request_id: u64, guardian: Address)
("recovery", "threshold_reached") → (creator: Address, request_id: u64)
("recovery", "executed") → (creator: Address, request_id: u64, new_owner: Address)
```

## Usage Flow

### Setup Phase
1. Creator calls `recovery_init()`
2. Creator calls `recovery_add_guardian()` for each trusted person (e.g., 3 guardians)

### Recovery Phase
1. Creator calls `recovery_create_request(new_owner_address)`
2. Each guardian reviews and calls `recovery_approve(request_id)`
3. After 2 of 3 guardians approve (66%+), request enters Locked state
4. After 7-day timelock expires, anyone calls `recovery_execute(request_id)`
5. Creator recovers access with new owner address

### Guardian Management
1. Creator calls `recovery_revoke_guardian(guardian_address)` to start revocation
2. After 1 day, guardian becomes inactive for new requests
3. Creator adds replacement guardian with `recovery_add_guardian()`

## Failure Modes & Mitigations

| Scenario | Mitigation |
|----------|-----------|
| Attacker compromises one guardian | 66% threshold prevents single-guardian takeover |
| Attacker compromises 2 of 3 guardians | Still need creator's own approval via request creation |
| Attacker compromises all guardians | Create new recovery request; timelocked for 7 days |
| Creator loses private key | Can recover access if ≥2 guardians are uncompromised |
| Accidental revocation of guardian | 1-day delay allows reversal |
| Double-voting attack | Checked at approval time; prevented |

## Future Enhancements

1. **Guardian Tiers**: Different approval thresholds for different security levels
2. **Social Recovery Bonds**: Economic incentive for guardian diligence
3. **Recovery Contests**: Allow creator to cancel recovery if detected early
4. **Guardian Key Rotation**: Guardians can update their addresses
5. **Time-Based Guardian Expiry**: Automatically expire guardians after set duration
6. **Delegated Recovery**: Guardians can delegate voting to other addresses

## Testing

Comprehensive test suite in `tests/recovery_tests.rs` covers:

- Guardian initialization
- Adding/revoking guardians
- Recovery request creation
- Single and multi-guardian voting
- Threshold calculation
- Timelock enforcement
- Duplicate prevention
- Authorization checks
- Attempt tracking
- Edge cases and error conditions

Run tests with:
```bash
cargo test recovery_tests
```

## Integration with TipJar

The recovery system integrates with the main contract:

1. **Creator Account Recovery**: Recover access to creator profiles and balances
2. **Tip Withdrawal Control**: Guardian system can eventually gate large withdrawals
3. **Governance Integration**: Recovered accounts can participate in governance
4. **Audit Trail**: Recovery events logged for compliance

## References

- Social Recovery: https://vitalik.ca/general/2021/01/11/recovery.html
- Multisig Wallets: https://en.wikipedia.org/wiki/Multi-signature
- Account Recovery: https://support.google.com/accounts/answer/7071300
