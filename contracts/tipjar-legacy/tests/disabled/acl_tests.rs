#![cfg(test)]

extern crate std;

use soroban_sdk::{Address, Env, String};
use tipjar::{
    acl::{LEVEL_ADMIN, LEVEL_CREATOR, LEVEL_MODERATOR},
    TipJarContract, TipJarContractClient,
};

fn setup() -> (Env, TipJarContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TipJarContract);
    let client = TipJarContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract(token_admin);
    client.init(&admin, &0u32, &0u64);
    client.add_token(&admin, &token_id);

    (env, client, admin)
}

// ── role assignment ───────────────────────────────────────────────────────────

#[test]
fn test_assign_builtin_role() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);

    client.acl_assign_role(&admin, &user, &String::from_str(&env, "moderator"));

    let role = client.acl_get_role(&user).expect("role should be set");
    assert_eq!(role.level, LEVEL_MODERATOR);
}

#[test]
fn test_revoke_role_clears_assignment() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);

    client.acl_assign_role(&admin, &user, &String::from_str(&env, "creator"));
    client.acl_revoke_role(&admin, &user);

    assert!(client.acl_get_role(&user).is_none());
}

#[test]
fn test_no_role_returns_none() {
    let (env, client, _admin) = setup();
    let user = Address::generate(&env);
    assert!(client.acl_get_role(&user).is_none());
}

// ── role hierarchy ────────────────────────────────────────────────────────────

#[test]
fn test_has_min_level_true_for_sufficient_role() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);

    client.acl_assign_role(&admin, &user, &String::from_str(&env, "admin"));

    assert!(client.acl_has_min_level(&user, &LEVEL_ADMIN));
    assert!(client.acl_has_min_level(&user, &LEVEL_MODERATOR));
}

#[test]
fn test_has_min_level_false_for_insufficient_role() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);

    client.acl_assign_role(&admin, &user, &String::from_str(&env, "creator"));

    assert!(!client.acl_has_min_level(&user, &LEVEL_ADMIN));
}

// ── custom roles ──────────────────────────────────────────────────────────────

#[test]
fn test_define_and_assign_custom_role() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);
    let role_name = String::from_str(&env, "reviewer");

    client.acl_define_custom_role(&admin, &role_name, &30u32);
    client.acl_assign_role(&admin, &user, &role_name);

    let role = client.acl_get_role(&user).expect("role should be set");
    assert_eq!(role.level, 30);
}

// ── permission checks ─────────────────────────────────────────────────────────

#[test]
fn test_grant_and_check_permission() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);
    let perm = String::from_str(&env, "tip:withdraw");

    client.acl_grant_permission(&admin, &user, &perm);

    assert!(client.acl_check_permission(&user, &perm));
}

#[test]
fn test_revoke_permission() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);
    let perm = String::from_str(&env, "tip:withdraw");

    client.acl_grant_permission(&admin, &user, &perm);
    client.acl_revoke_permission(&admin, &user, &perm);

    assert!(!client.acl_check_permission(&user, &perm));
}

#[test]
fn test_check_permission_false_when_not_granted() {
    let (env, client, _admin) = setup();
    let user = Address::generate(&env);
    let perm = String::from_str(&env, "tip:withdraw");

    assert!(!client.acl_check_permission(&user, &perm));
}

// ── change history ────────────────────────────────────────────────────────────

#[test]
fn test_change_count_increments_on_each_action() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);

    assert_eq!(client.acl_get_change_count(), 0);

    client.acl_assign_role(&admin, &user, &String::from_str(&env, "creator"));
    assert_eq!(client.acl_get_change_count(), 1);

    client.acl_grant_permission(&admin, &user, &String::from_str(&env, "tip:read"));
    assert_eq!(client.acl_get_change_count(), 2);

    client.acl_revoke_role(&admin, &user);
    assert_eq!(client.acl_get_change_count(), 3);
}

#[test]
fn test_change_entry_records_subject() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);

    client.acl_assign_role(&admin, &user, &String::from_str(&env, "viewer"));

    let entry = client
        .acl_get_change_entry(&0u64)
        .expect("entry should exist");
    assert_eq!(entry.subject, user);
}

#[test]
fn test_change_entry_out_of_range_returns_none() {
    let (_env, client, _admin) = setup();
    assert!(client.acl_get_change_entry(&99u64).is_none());
}
