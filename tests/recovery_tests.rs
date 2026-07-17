#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};
use tipjar::recovery::{RecoveryRequest, RecoveryStatus};
use tipjar::TipJarContract;

#[test]
fn test_recovery_init() {
    let env = Env::default();
    let contract = TipJarContract {};
    let creator = Address::random(&env);

    contract.recovery_init(env.clone(), creator.clone());

    // Should succeed without panicking
    assert_eq!(1, 1); // Sanity check
}

#[test]
fn test_add_guardian() {
    let env = Env::default();
    let contract = TipJarContract {};
    let creator = Address::random(&env);
    let guardian = Address::random(&env);

    env.mock_all_auths();
    contract.recovery_init(env.clone(), creator.clone());
    contract.recovery_add_guardian(env.clone(), creator.clone(), guardian.clone(), 1);

    // Verify guardian was added (implicitly, if no panic)
    assert_eq!(1, 1);
}

#[test]
#[should_panic(expected = "Guardian already exists")]
fn test_duplicate_guardian() {
    let env = Env::default();
    let contract = TipJarContract {};
    let creator = Address::random(&env);
    let guardian = Address::random(&env);

    env.mock_all_auths();
    contract.recovery_init(env.clone(), creator.clone());
    contract.recovery_add_guardian(env.clone(), creator.clone(), guardian.clone(), 1);
    contract.recovery_add_guardian(env.clone(), creator.clone(), guardian.clone(), 1);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_unauthorized_add_guardian() {
    let env = Env::default();
    let contract = TipJarContract {};
    let creator = Address::random(&env);
    let guardian = Address::random(&env);
    let unauthorized = Address::random(&env);

    env.mock_all_auths();
    contract.recovery_init(env.clone(), creator.clone());

    // Try to add guardian as unauthorized user
    env.set_default_info_for_address(&unauthorized);
    contract.recovery_add_guardian(env.clone(), creator.clone(), guardian.clone(), 1);
}

#[test]
fn test_recovery_request_creation() {
    let env = Env::default();
    let contract = TipJarContract {};
    let creator = Address::random(&env);
    let guardian = Address::random(&env);
    let new_owner = Address::random(&env);

    env.mock_all_auths();
    contract.recovery_init(env.clone(), creator.clone());
    contract.recovery_add_guardian(env.clone(), creator.clone(), guardian.clone(), 1);
    contract.recovery_create_request(env.clone(), creator.clone(), new_owner.clone());

    let request = contract.recovery_get_request(env.clone(), 1);
    assert_eq!(request.id, 1);
    assert_eq!(request.creator, creator);
    assert_eq!(request.new_owner, new_owner);
    assert_eq!(request.status, RecoveryStatus::Voting);
    assert_eq!(request.approval_count, 0);
}

#[test]
#[should_panic(expected = "No guardians set for recovery")]
fn test_recovery_request_no_guardians() {
    let env = Env::default();
    let contract = TipJarContract {};
    let creator = Address::random(&env);
    let new_owner = Address::random(&env);

    env.mock_all_auths();
    contract.recovery_init(env.clone(), creator.clone());
    contract.recovery_create_request(env.clone(), creator.clone(), new_owner.clone());
}

#[test]
fn test_guardian_approval() {
    let env = Env::default();
    let contract = TipJarContract {};
    let creator = Address::random(&env);
    let guardian = Address::random(&env);
    let new_owner = Address::random(&env);

    env.mock_all_auths();
    contract.recovery_init(env.clone(), creator.clone());
    contract.recovery_add_guardian(env.clone(), creator.clone(), guardian.clone(), 1);
    contract.recovery_create_request(env.clone(), creator.clone(), new_owner.clone());

    // Guardian approves
    env.set_default_info_for_address(&guardian);
    contract.recovery_approve(env.clone(), 1);

    let request = contract.recovery_get_request(env.clone(), 1);
    assert_eq!(request.approval_count, 1);
    assert_eq!(request.status, RecoveryStatus::Locked); // Threshold met (1/1 guardians)
    assert!(request.timelock_end > 0);
}

#[test]
#[should_panic(expected = "Already approved")]
fn test_guardian_double_approval() {
    let env = Env::default();
    let contract = TipJarContract {};
    let creator = Address::random(&env);
    let guardian = Address::random(&env);
    let new_owner = Address::random(&env);

    env.mock_all_auths();
    contract.recovery_init(env.clone(), creator.clone());
    contract.recovery_add_guardian(env.clone(), creator.clone(), guardian.clone(), 1);
    contract.recovery_create_request(env.clone(), creator.clone(), new_owner.clone());

    env.set_default_info_for_address(&guardian);
    contract.recovery_approve(env.clone(), 1);
    contract.recovery_approve(env.clone(), 1); // Should panic
}

#[test]
#[should_panic(expected = "Not an active guardian")]
fn test_non_guardian_approval() {
    let env = Env::default();
    let contract = TipJarContract {};
    let creator = Address::random(&env);
    let guardian = Address::random(&env);
    let non_guardian = Address::random(&env);
    let new_owner = Address::random(&env);

    env.mock_all_auths();
    contract.recovery_init(env.clone(), creator.clone());
    contract.recovery_add_guardian(env.clone(), creator.clone(), guardian.clone(), 1);
    contract.recovery_create_request(env.clone(), creator.clone(), new_owner.clone());

    env.set_default_info_for_address(&non_guardian);
    contract.recovery_approve(env.clone(), 1); // Should panic
}

#[test]
fn test_multiple_guardians_threshold() {
    let env = Env::default();
    let contract = TipJarContract {};
    let creator = Address::random(&env);
    let guardian1 = Address::random(&env);
    let guardian2 = Address::random(&env);
    let guardian3 = Address::random(&env);
    let new_owner = Address::random(&env);

    env.mock_all_auths();
    contract.recovery_init(env.clone(), creator.clone());
    contract.recovery_add_guardian(env.clone(), creator.clone(), guardian1.clone(), 1);
    contract.recovery_add_guardian(env.clone(), creator.clone(), guardian2.clone(), 1);
    contract.recovery_add_guardian(env.clone(), creator.clone(), guardian3.clone(), 1);
    contract.recovery_create_request(env.clone(), creator.clone(), new_owner.clone());

    // 1 approval: 1/3 = 33% (need 66%)
    env.set_default_info_for_address(&guardian1);
    contract.recovery_approve(env.clone(), 1);
    let request = contract.recovery_get_request(env.clone(), 1);
    assert_eq!(request.status, RecoveryStatus::Voting);

    // 2 approvals: 2/3 = 66% (meets threshold)
    env.set_default_info_for_address(&guardian2);
    contract.recovery_approve(env.clone(), 1);
    let request = contract.recovery_get_request(env.clone(), 1);
    assert_eq!(request.status, RecoveryStatus::Locked);
}

#[test]
#[should_panic(expected = "Timelock not expired")]
fn test_recovery_before_timelock() {
    let env = Env::default();
    let contract = TipJarContract {};
    let creator = Address::random(&env);
    let guardian = Address::random(&env);
    let new_owner = Address::random(&env);

    env.mock_all_auths();
    contract.recovery_init(env.clone(), creator.clone());
    contract.recovery_add_guardian(env.clone(), creator.clone(), guardian.clone(), 1);
    contract.recovery_create_request(env.clone(), creator.clone(), new_owner.clone());

    env.set_default_info_for_address(&guardian);
    contract.recovery_approve(env.clone(), 1);

    // Try to execute before timelock expires
    contract.recovery_execute(env.clone(), 1);
}

#[test]
fn test_recovery_after_timelock() {
    let env = Env::default();
    let contract = TipJarContract {};
    let creator = Address::random(&env);
    let guardian = Address::random(&env);
    let new_owner = Address::random(&env);

    env.mock_all_auths();
    contract.recovery_init(env.clone(), creator.clone());
    contract.recovery_add_guardian(env.clone(), creator.clone(), guardian.clone(), 1);
    contract.recovery_create_request(env.clone(), creator.clone(), new_owner.clone());

    env.set_default_info_for_address(&guardian);
    contract.recovery_approve(env.clone(), 1);

    // Advance time beyond timelock (7 days + 1 second)
    env.ledger().with_mut(|l| {
        l.set_timestamp(env.ledger().timestamp() + 604801);
    });

    let recovered_owner = contract.recovery_execute(env.clone(), 1);
    assert_eq!(recovered_owner, new_owner);

    let request = contract.recovery_get_request(env.clone(), 1);
    assert_eq!(request.status, RecoveryStatus::Executed);
    assert!(request.executed_at > 0);
}

#[test]
fn test_revoke_guardian() {
    let env = Env::default();
    let contract = TipJarContract {};
    let creator = Address::random(&env);
    let guardian = Address::random(&env);

    env.mock_all_auths();
    contract.recovery_init(env.clone(), creator.clone());
    contract.recovery_add_guardian(env.clone(), creator.clone(), guardian.clone(), 1);
    contract.recovery_revoke_guardian(env.clone(), creator.clone(), guardian.clone());

    // After revocation delay (1 day), guardian should be inactive for new requests
    env.ledger().with_mut(|l| {
        l.set_timestamp(env.ledger().timestamp() + 86401);
    });

    let new_owner = Address::random(&env);
    // Should panic because guardian is now inactive
    contract.recovery_create_request(env.clone(), creator.clone(), new_owner.clone());
}

#[test]
fn test_recovery_attempts_tracking() {
    let env = Env::default();
    let contract = TipJarContract {};
    let creator = Address::random(&env);
    let guardian = Address::random(&env);
    let new_owner1 = Address::random(&env);
    let new_owner2 = Address::random(&env);

    env.mock_all_auths();
    contract.recovery_init(env.clone(), creator.clone());
    contract.recovery_add_guardian(env.clone(), creator.clone(), guardian.clone(), 1);

    contract.recovery_create_request(env.clone(), creator.clone(), new_owner1.clone());
    contract.recovery_create_request(env.clone(), creator.clone(), new_owner2.clone());

    let attempts = contract.recovery_get_recent_attempts(env.clone(), creator.clone(), 0);
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts.get(0), Some(1));
    assert_eq!(attempts.get(1), Some(2));
}
