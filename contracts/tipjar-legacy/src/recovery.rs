use soroban_sdk::{contracttype, Address, Env, Map, Vec};

/// Recovery request status
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryStatus {
    Voting,   // Guardians voting on recovery
    Locked,   // Timelock active, awaiting execution
    Executed, // Recovery completed
    Rejected, // Recovery rejected
}

/// Guardian record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Guardian {
    pub address: Address,
    pub weight: u32,        // Voting weight (typically 1)
    pub added_at: u64,      // Timestamp when added
    pub revocation_time: u64, // Time when revocation becomes effective (0 = active)
}

/// Recovery request
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryRequest {
    pub id: u64,
    pub creator: Address,
    pub new_owner: Address, // The account to recover to
    pub status: RecoveryStatus,
    pub approval_count: u32,
    pub created_at: u64,
    pub executed_at: u64,
    pub timelock_end: u64, // Unix timestamp when recovery can execute
}

/// Recovery attempt record for rate limiting
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryAttempt {
    pub request_id: u64,
    pub timestamp: u64,
}

#[derive(Clone)]
pub struct RecoveryConfig {
    pub approval_threshold: u32, // % of guardian weight needed (e.g., 66 = 66%)
    pub timelock_delay: u64,     // Seconds before recovery can execute
    pub guardian_revocation_delay: u64, // Seconds before guardian removal is effective
}

impl RecoveryConfig {
    pub fn default() -> Self {
        RecoveryConfig {
            approval_threshold: 66,
            timelock_delay: 604800, // 7 days
            guardian_revocation_delay: 86400, // 1 day
        }
    }
}

/// Initialize recovery system for a creator
pub fn init_recovery(env: &Env, creator: &Address) {
    let key = crate::DataKey::RecoveryGuardians(creator.clone());
    let guardians: Vec<Guardian> = Vec::new(env);
    env.storage().instance().set(&key, &guardians);
    
    let attempts_key = crate::DataKey::RecoveryAttempts(creator.clone());
    let attempts: Vec<RecoveryAttempt> = Vec::new(env);
    env.storage().instance().set(&attempts_key, &attempts);
    
    let counter_key = crate::DataKey::RecoveryCounter;
    env.storage().instance().set(&counter_key, &0u64);
}

/// Add a guardian for account recovery
pub fn add_guardian(env: &Env, creator: &Address, guardian: &Address, weight: u32) {
    let key = crate::DataKey::RecoveryGuardians(creator.clone());
    let mut guardians: Vec<Guardian> = env
        .storage()
        .instance()
        .get(&key)
        .unwrap_or(Vec::new(env));

    // Check guardian not already exists (active)
    for g in guardians.iter() {
        if g.address == *guardian && g.revocation_time == 0 {
            panic!("Guardian already exists");
        }
    }

    let new_guardian = Guardian {
        address: guardian.clone(),
        weight,
        added_at: env.ledger().timestamp(),
        revocation_time: 0,
    };
    guardians.push_back(new_guardian);
    env.storage().instance().set(&key, &guardians);
}

/// Initiate revocation of a guardian (with delay)
pub fn revoke_guardian(env: &Env, creator: &Address, guardian: &Address) {
    let key = crate::DataKey::RecoveryGuardians(creator.clone());
    let mut guardians: Vec<Guardian> = env
        .storage()
        .instance()
        .get(&key)
        .unwrap_or(Vec::new(env));

    let config = RecoveryConfig::default();
    let revocation_time = env.ledger().timestamp() + config.guardian_revocation_delay;

    for i in 0..guardians.len() {
        let g = guardians.get(i).unwrap();
        if g.address == *guardian && g.revocation_time == 0 {
            let updated = Guardian {
                revocation_time,
                ..g
            };
            guardians.set(i, updated);
            break;
        }
    }
    env.storage().instance().set(&key, &guardians);
}

/// Get active guardians for creator
fn get_active_guardians(env: &Env, creator: &Address) -> Vec<Guardian> {
    let key = crate::DataKey::RecoveryGuardians(creator.clone());
    let guardians: Vec<Guardian> = env
        .storage()
        .instance()
        .get(&key)
        .unwrap_or(Vec::new(env));

    let now = env.ledger().timestamp();
    let mut active = Vec::new(env);
    for g in guardians.iter() {
        if g.revocation_time == 0 || g.revocation_time > now {
            active.push_back(g);
        }
    }
    active
}

/// Get total weight of active guardians
fn get_total_guardian_weight(env: &Env, creator: &Address) -> u32 {
    get_active_guardians(env, creator)
        .iter()
        .fold(0u32, |acc, g| acc + g.weight)
}

/// Create a recovery request
pub fn create_recovery_request(env: &Env, creator: &Address, new_owner: &Address) {
    // Check guardian exists
    let guardians = get_active_guardians(env, creator);
    if guardians.len() == 0 {
        panic!("No guardians set for recovery");
    }

    // Get and increment counter
    let counter_key = crate::DataKey::RecoveryCounter;
    let mut counter: u64 = env
        .storage()
        .instance()
        .get(&counter_key)
        .unwrap_or(0u64);
    counter += 1;
    env.storage().instance().set(&counter_key, &counter);

    let config = RecoveryConfig::default();
    let request = RecoveryRequest {
        id: counter,
        creator: creator.clone(),
        new_owner: new_owner.clone(),
        status: RecoveryStatus::Voting,
        approval_count: 0,
        created_at: env.ledger().timestamp(),
        executed_at: 0,
        timelock_end: 0,
    };

    let key = crate::DataKey::RecoveryRequest(counter);
    env.storage().instance().set(&key, &request);

    // Record attempt
    let attempts_key = crate::DataKey::RecoveryAttempts(creator.clone());
    let mut attempts: Vec<RecoveryAttempt> = env
        .storage()
        .instance()
        .get(&attempts_key)
        .unwrap_or(Vec::new(env));
    
    attempts.push_back(RecoveryAttempt {
        request_id: counter,
        timestamp: env.ledger().timestamp(),
    });
    env.storage().instance().set(&attempts_key, &attempts);

    // Emit event
    env.events().publish(
        ("recovery", "request_created"),
        (creator.clone(), counter, new_owner.clone()),
    );
}

/// Guardian approves a recovery request
pub fn approve_recovery(env: &Env, request_id: u64, guardian: &Address) {
    let request_key = crate::DataKey::RecoveryRequest(request_id);
    let mut request: RecoveryRequest = env
        .storage()
        .instance()
        .get(&request_key)
        .expect("Recovery request not found");

    if request.status != RecoveryStatus::Voting {
        panic!("Recovery not in voting phase");
    }

    // Verify guardian is active
    let guardians = get_active_guardians(env, &request.creator);
    let mut is_guardian = false;
    for g in guardians.iter() {
        if g.address == *guardian {
            is_guardian = true;
            break;
        }
    }
    if !is_guardian {
        panic!("Not an active guardian");
    }

    // Check if already approved (prevent double voting)
    let approval_key = crate::DataKey::RecoveryApproval(request_id, guardian.clone());
    if env.storage().instance().has(&approval_key) {
        panic!("Guardian already approved");
    }

    // Record approval
    env.storage().instance().set(&approval_key, &true);
    request.approval_count += 1;

    // Check if threshold met
    let total_weight = get_total_guardian_weight(env, &request.creator);
    let config = RecoveryConfig::default();
    let required_weight = (total_weight * config.approval_threshold) / 100;

    if request.approval_count >= required_weight {
        // Move to locked state
        request.status = RecoveryStatus::Locked;
        request.timelock_end = env.ledger().timestamp() + config.timelock_delay;
        
        env.events().publish(
            ("recovery", "threshold_reached"),
            (request.creator.clone(), request_id),
        );
    }

    env.storage().instance().set(&request_key, &request);

    env.events()
        .publish(("recovery", "approved"), (request_id, guardian.clone()));
}

/// Execute recovery after timelock expires
pub fn execute_recovery(env: &Env, request_id: u64) -> Address {
    let request_key = crate::DataKey::RecoveryRequest(request_id);
    let mut request: RecoveryRequest = env
        .storage()
        .instance()
        .get(&request_key)
        .expect("Recovery request not found");

    if request.status != RecoveryStatus::Locked {
        panic!("Recovery not ready for execution");
    }

    let now = env.ledger().timestamp();
    if now < request.timelock_end {
        panic!("Timelock not expired");
    }

    request.status = RecoveryStatus::Executed;
    request.executed_at = now;
    env.storage().instance().set(&request_key, &request);

    env.events().publish(
        ("recovery", "executed"),
        (request.creator.clone(), request_id, request.new_owner.clone()),
    );

    request.new_owner.clone()
}

/// Get recovery request details
pub fn get_recovery_request(env: &Env, request_id: u64) -> RecoveryRequest {
    let key = crate::DataKey::RecoveryRequest(request_id);
    env.storage()
        .instance()
        .get(&key)
        .expect("Recovery request not found")
}

/// Get recent recovery attempts (for rate limiting)
pub fn get_recent_attempts(env: &Env, creator: &Address, since_timestamp: u64) -> Vec<u64> {
    let attempts_key = crate::DataKey::RecoveryAttempts(creator.clone());
    let attempts: Vec<RecoveryAttempt> = env
        .storage()
        .instance()
        .get(&attempts_key)
        .unwrap_or(Vec::new(env));

    let mut result = Vec::new(env);
    for attempt in attempts.iter() {
        if attempt.timestamp >= since_timestamp {
            result.push_back(attempt.request_id);
        }
    }
    result
}
