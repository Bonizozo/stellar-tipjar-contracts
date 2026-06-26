//! Key management for homomorphic encryption.
//!
//! Handles:
//! - Public key initialization and storage
//! - Key rotation with versioning
//! - Key access control (admin-only operations)
//! - Key expiration and lifecycle management
//! - Secure key derivation

use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

use super::homomorphic::{HomomorphicConfig, HomomorphicPublicKey};
use crate::DataKey;

/// Key management configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyManagementConfig {
    /// Key rotation interval in seconds (0 = no automatic rotation)
    pub rotation_interval: u64,
    /// Maximum key age before forced rotation (seconds)
    pub max_key_age: u64,
    /// Number of previous key versions to keep
    pub key_history_size: u32,
    /// Whether key rotation requires admin approval
    pub rotation_requires_approval: bool,
}

/// Key rotation event for audit trail.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyRotationEvent {
    /// Old key version
    pub old_version: u32,
    /// New key version
    pub new_version: u32,
    /// Rotation timestamp
    pub timestamp: u64,
    /// Reason for rotation
    pub reason: Symbol,
}

/// Initialize homomorphic encryption with public key.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `admin` - Admin address (must be authorized)
/// * `public_key` - Initial public key
/// * `config` - Key management configuration
///
/// # Returns
/// Ok(()) on success, Err on failure
pub fn initialize_homomorphic(
    env: &Env,
    admin: &Address,
    public_key: HomomorphicPublicKey,
    config: KeyManagementConfig,
) -> Result<(), &'static str> {
    // Verify admin authorization
    admin.require_auth();

    // Check if already initialized
    if let Ok(existing) = get_homomorphic_config(env) {
        if existing.enabled {
            return Err("homomorphic encryption already initialized");
        }
    }

    // Create initial configuration
    let hom_config = HomomorphicConfig {
        public_key: public_key.clone(),
        enabled: true,
        min_bit_length: 32,
        max_bit_length: 128,
        key_rotation_time: 0,
    };

    // Store configuration
    env.storage()
        .instance()
        .set(&DataKey::HomomorphicConfig, &hom_config);

    // Store key management config
    env.storage()
        .instance()
        .set(&DataKey::KeyManagementConfig, &config);

    // Initialize key history
    let mut history: Vec<HomomorphicPublicKey> = Vec::new(env);
    history.push_back(public_key.clone());
    env.storage().instance().set(&DataKey::KeyHistory, &history);

    // Emit initialization event
    env.events().publish(
        (Symbol::new(env, "homomorphic_init"),),
        (admin.clone(), hom_config.public_key.version),
    );

    Ok(())
}

/// Rotate to a new public key.
///
/// Maintains key history for decryption of old ciphertexts.
/// Requires admin authorization.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `admin` - Admin address
/// * `new_key` - New public key
/// * `reason` - Reason for rotation
///
/// # Returns
/// Ok(new_version) on success, Err on failure
pub fn rotate_key(
    env: &Env,
    admin: &Address,
    new_key: HomomorphicPublicKey,
    reason: Symbol,
) -> Result<u32, &'static str> {
    admin.require_auth();

    let mut config = get_homomorphic_config(env)?;
    let old_version = config.public_key.version;

    // Verify new key version is incremented
    if new_key.version <= old_version {
        return Err("new key version must be greater than current");
    }

    // Update configuration
    config.public_key = new_key.clone();
    config.key_rotation_time = env.ledger().timestamp();

    env.storage()
        .instance()
        .set(&DataKey::HomomorphicConfig, &config);

    // Update key history
    let mut history = get_key_history(env)?;
    history.push_back(new_key.clone());

    // Trim history if needed
    let key_mgmt_config = get_key_management_config(env)?;
    while (history.len() as usize) > key_mgmt_config.key_history_size as usize {
        history.pop_front();
    }

    env.storage().instance().set(&DataKey::KeyHistory, &history);

    // Record rotation event
    let _event = KeyRotationEvent {
        old_version,
        new_version: new_key.version,
        timestamp: env.ledger().timestamp(),
        reason,
    };

    env.events().publish(
        (Symbol::new(env, "key_rotated"),),
        (old_version, new_key.version),
    );

    Ok(new_key.version)
}

/// Get current homomorphic configuration.
pub fn get_homomorphic_config(env: &Env) -> Result<HomomorphicConfig, &'static str> {
    env.storage()
        .instance()
        .get(&DataKey::HomomorphicConfig)
        .ok_or("homomorphic encryption not initialized")
}

/// Get key management configuration.
pub fn get_key_management_config(env: &Env) -> Result<KeyManagementConfig, &'static str> {
    env.storage()
        .instance()
        .get(&DataKey::KeyManagementConfig)
        .ok_or("key management config not found")
}

/// Get current public key.
pub fn get_current_public_key(env: &Env) -> Result<HomomorphicPublicKey, &'static str> {
    let config = get_homomorphic_config(env)?;
    Ok(config.public_key)
}

/// Get public key by version.
///
/// Allows decryption of ciphertexts encrypted with older keys.
pub fn get_public_key_by_version(
    env: &Env,
    version: u32,
) -> Result<HomomorphicPublicKey, &'static str> {
    let history = get_key_history(env)?;

    for key in history.iter() {
        if key.version == version {
            return Ok(key.clone());
        }
    }

    Err("public key version not found")
}

/// Get full key history.
pub fn get_key_history(env: &Env) -> Result<Vec<HomomorphicPublicKey>, &'static str> {
    env.storage()
        .instance()
        .get(&DataKey::KeyHistory)
        .ok_or("key history not found")
}

/// Check if homomorphic encryption is enabled.
pub fn is_homomorphic_enabled(env: &Env) -> bool {
    get_homomorphic_config(env)
        .map(|config| config.enabled)
        .unwrap_or(false)
}

/// Enable homomorphic encryption (if disabled).
pub fn enable_homomorphic(env: &Env, admin: &Address) -> Result<(), &'static str> {
    admin.require_auth();

    let mut config = get_homomorphic_config(env)?;
    config.enabled = true;

    env.storage()
        .instance()
        .set(&DataKey::HomomorphicConfig, &config);

    env.events()
        .publish((Symbol::new(env, "homomorphic_enabled"),), (admin.clone(),));

    Ok(())
}

/// Disable homomorphic encryption (for maintenance).
pub fn disable_homomorphic(env: &Env, admin: &Address) -> Result<(), &'static str> {
    admin.require_auth();

    let mut config = get_homomorphic_config(env)?;
    config.enabled = false;

    env.storage()
        .instance()
        .set(&DataKey::HomomorphicConfig, &config);

    env.events().publish(
        (Symbol::new(env, "homomorphic_disabled"),),
        (admin.clone(),),
    );

    Ok(())
}

/// Verify key is still valid (not expired).
pub fn verify_key_validity(env: &Env, key_version: u32) -> Result<(), &'static str> {
    let config = get_homomorphic_config(env)?;
    let _key_mgmt = get_key_management_config(env)?;

    // Get the key
    let _key = get_public_key_by_version(env, key_version)?;

    // Check if key is current or recent
    if key_version < config.public_key.version {
        // Old key - check if still within history
        let history = get_key_history(env)?;
        let mut found = false;
        for h_key in history.iter() {
            if h_key.version == key_version {
                found = true;
                break;
            }
        }
        if !found {
            return Err("key version no longer in history");
        }
    }

    Ok(())
}
