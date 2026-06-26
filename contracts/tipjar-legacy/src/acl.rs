//! Access Control Lists for tip operations.
//!
//! Provides:
//! - Role hierarchy: SuperAdmin > Admin > Moderator > Creator > Viewer (built-in)
//! - Custom roles: any string name with a configurable hierarchy level
//! - Permission checks: per-address, per-permission-string gate
//! - Role assignment / revocation (admin-only for built-in; owner for custom)
//! - Change history: every grant/revoke/permission change is logged on-chain

use soroban_sdk::{contracttype, symbol_short, Address, Env, String};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Built-in role levels (higher = more privileged).
pub const LEVEL_SUPER_ADMIN: u32 = 100;
pub const LEVEL_ADMIN: u32 = 80;
pub const LEVEL_MODERATOR: u32 = 60;
pub const LEVEL_CREATOR: u32 = 40;
pub const LEVEL_VIEWER: u32 = 20;

// ── Types ────────────────────────────────────────────────────────────────────

/// A role definition: name + hierarchy level.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclRole {
    pub name: String,
    /// Hierarchy level; higher value = more privileged.
    pub level: u32,
}

/// One entry in the ACL change history.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclChangeEntry {
    /// Address whose role/permission changed.
    pub subject: Address,
    /// Human-readable description of the change (e.g. "grant:admin").
    pub action: String,
    pub timestamp: u64,
}

// ── Storage helpers ──────────────────────────────────────────────────────────

fn load_role(env: &Env, subject: &Address) -> Option<AclRole> {
    env.storage()
        .persistent()
        .get(&DataKey::AclRole(subject.clone()))
}

fn save_role(env: &Env, subject: &Address, role: &AclRole) {
    env.storage()
        .persistent()
        .set(&DataKey::AclRole(subject.clone()), role);
}

fn remove_role(env: &Env, subject: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::AclRole(subject.clone()));
}

fn has_permission_stored(env: &Env, subject: &Address, permission: &String) -> bool {
    env.storage()
        .persistent()
        .get::<DataKey, bool>(&DataKey::AclPermission(subject.clone(), permission.clone()))
        .unwrap_or(false)
}

fn set_permission_stored(env: &Env, subject: &Address, permission: &String, value: bool) {
    env.storage()
        .persistent()
        .set(&DataKey::AclPermission(subject.clone(), permission.clone()), &value);
}

fn change_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::AclChangeCount)
        .unwrap_or(0u64)
}

fn append_change(env: &Env, entry: &AclChangeEntry) {
    let idx = change_count(env);
    env.storage()
        .persistent()
        .set(&DataKey::AclChangeLog(idx), entry);
    env.storage()
        .persistent()
        .set(&DataKey::AclChangeCount, &(idx + 1));
}

// ── Built-in role helpers ────────────────────────────────────────────────────

fn builtin_role(env: &Env, name: &String) -> Option<AclRole> {
    let level = if *name == String::from_str(env, "super_admin") {
        LEVEL_SUPER_ADMIN
    } else if *name == String::from_str(env, "admin") {
        LEVEL_ADMIN
    } else if *name == String::from_str(env, "moderator") {
        LEVEL_MODERATOR
    } else if *name == String::from_str(env, "creator") {
        LEVEL_CREATOR
    } else if *name == String::from_str(env, "viewer") {
        LEVEL_VIEWER
    } else {
        return None;
    };
    Some(AclRole { name: name.clone(), level })
}

fn caller_level(env: &Env, caller: &Address) -> u32 {
    load_role(env, caller).map(|r| r.level).unwrap_or(0)
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Assign a built-in or custom role to `subject`.
///
/// `caller` must hold a role with a strictly higher level than the role being
/// assigned (prevents privilege escalation).  For bootstrapping, the contract
/// admin stored in `DataKey::Admin` may always assign any role.
pub fn assign_role(env: &Env, caller: &Address, subject: &Address, role_name: &String) {
    caller.require_auth();

    let role = builtin_role(env, role_name)
        .or_else(|| {
            env.storage()
                .persistent()
                .get(&DataKey::AclCustomRole(role_name.clone()))
        })
        .expect("unknown role");

    // Admin bypass or hierarchy check.
    let admin: Option<Address> = env.storage().instance().get(&DataKey::Admin);
    let is_admin = admin.as_ref().map(|a| a == caller).unwrap_or(false);
    if !is_admin {
        assert!(caller_level(env, caller) > role.level, "insufficient privilege");
    }

    save_role(env, subject, &role);

    let action = String::from_str(env, "grant");
    append_change(env, &AclChangeEntry {
        subject: subject.clone(),
        action: action.clone(),
        timestamp: env.ledger().timestamp(),
    });
    env.events().publish(
        (symbol_short!("acl_grt"), subject.clone()),
        role_name.clone(),
    );
}

/// Revoke the role of `subject`.
///
/// `caller` must be the contract admin or hold a higher level than `subject`.
pub fn revoke_role(env: &Env, caller: &Address, subject: &Address) {
    caller.require_auth();

    let admin: Option<Address> = env.storage().instance().get(&DataKey::Admin);
    let is_admin = admin.as_ref().map(|a| a == caller).unwrap_or(false);
    if !is_admin {
        let subject_level = load_role(env, subject).map(|r| r.level).unwrap_or(0);
        assert!(caller_level(env, caller) > subject_level, "insufficient privilege");
    }

    remove_role(env, subject);

    let action = String::from_str(env, "revoke");
    append_change(env, &AclChangeEntry {
        subject: subject.clone(),
        action,
        timestamp: env.ledger().timestamp(),
    });
    env.events().publish(
        (symbol_short!("acl_rev"), subject.clone()),
        (),
    );
}

/// Define a new custom role with a given `level`.
/// Only the contract admin may create custom roles.
pub fn define_custom_role(env: &Env, caller: &Address, role_name: &String, level: u32) {
    caller.require_auth();
    let admin: Address = env.storage().instance().get(&DataKey::Admin).expect("no admin");
    assert!(admin == *caller, "admin only");

    let role = AclRole { name: role_name.clone(), level };
    env.storage()
        .persistent()
        .set(&DataKey::AclCustomRole(role_name.clone()), &role);

    let action = String::from_str(env, "define_role");
    append_change(env, &AclChangeEntry {
        subject: caller.clone(),
        action,
        timestamp: env.ledger().timestamp(),
    });
}

/// Grant an explicit permission string to `subject` (independent of role).
/// `caller` must be admin or hold a higher level than `subject`.
pub fn grant_permission(env: &Env, caller: &Address, subject: &Address, permission: &String) {
    caller.require_auth();
    let admin: Option<Address> = env.storage().instance().get(&DataKey::Admin);
    let is_admin = admin.as_ref().map(|a| a == caller).unwrap_or(false);
    if !is_admin {
        let subject_level = load_role(env, subject).map(|r| r.level).unwrap_or(0);
        assert!(caller_level(env, caller) > subject_level, "insufficient privilege");
    }

    set_permission_stored(env, subject, permission, true);

    let action = String::from_str(env, "perm_grant");
    append_change(env, &AclChangeEntry {
        subject: subject.clone(),
        action,
        timestamp: env.ledger().timestamp(),
    });
    env.events().publish(
        (symbol_short!("acl_prm"), subject.clone()),
        permission.clone(),
    );
}

/// Revoke an explicit permission from `subject`.
pub fn revoke_permission(env: &Env, caller: &Address, subject: &Address, permission: &String) {
    caller.require_auth();
    let admin: Option<Address> = env.storage().instance().get(&DataKey::Admin);
    let is_admin = admin.as_ref().map(|a| a == caller).unwrap_or(false);
    if !is_admin {
        let subject_level = load_role(env, subject).map(|r| r.level).unwrap_or(0);
        assert!(caller_level(env, caller) > subject_level, "insufficient privilege");
    }

    set_permission_stored(env, subject, permission, false);

    let action = String::from_str(env, "perm_revoke");
    append_change(env, &AclChangeEntry {
        subject: subject.clone(),
        action,
        timestamp: env.ledger().timestamp(),
    });
}

/// Returns `true` if `subject` has the explicit `permission` granted.
pub fn check_permission(env: &Env, subject: &Address, permission: &String) -> bool {
    has_permission_stored(env, subject, permission)
}

/// Returns the role assigned to `subject`, or `None`.
pub fn get_role(env: &Env, subject: &Address) -> Option<AclRole> {
    load_role(env, subject)
}

/// Returns `true` if `subject`'s role level is >= `required_level`.
pub fn has_min_level(env: &Env, subject: &Address, required_level: u32) -> bool {
    caller_level(env, subject) >= required_level
}

/// Returns a change history entry by index, or `None`.
pub fn get_change_entry(env: &Env, index: u64) -> Option<AclChangeEntry> {
    env.storage()
        .persistent()
        .get(&DataKey::AclChangeLog(index))
}

/// Returns the total number of change history entries.
pub fn get_change_count(env: &Env) -> u64 {
    change_count(env)
}
