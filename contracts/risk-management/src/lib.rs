#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, Address,
    Env,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskProfile {
    pub owner: Address,
    pub exposure: i128,
    pub risk_score: u32,
    pub alert_threshold: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskMetrics {
    pub total_exposure: i128,
    pub high_risk_count: u32,
    pub last_updated: u64,
}

#[contracttype]
pub enum DataKey {
    Profile(Address),
    Metrics,
    Admin,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RiskError {
    Unauthorized = 1,
    NotRegistered = 2,
    ExposureLimitExceeded = 3,
    AlreadyRegistered = 4,
    InvalidThreshold = 5,
}

#[contract]
pub struct RiskManagementContract;

#[contractimpl]
impl RiskManagementContract {
    pub fn init(env: Env, admin: Address) {
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(
            &DataKey::Metrics,
            &RiskMetrics {
                total_exposure: 0,
                high_risk_count: 0,
                last_updated: env.ledger().timestamp(),
            },
        );
    }

    pub fn register_participant(env: Env, caller: Address, alert_threshold: i128) {
        caller.require_auth();
        if alert_threshold <= 0 {
            panic_with_error!(&env, RiskError::InvalidThreshold);
        }
        if env
            .storage()
            .persistent()
            .has(&DataKey::Profile(caller.clone()))
        {
            panic_with_error!(&env, RiskError::AlreadyRegistered);
        }
        let profile = RiskProfile {
            owner: caller.clone(),
            exposure: 0,
            risk_score: 0,
            alert_threshold,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Profile(caller), &profile);
    }

    pub fn update_exposure(env: Env, caller: Address, delta: i128) {
        caller.require_auth();

        let mut profile: RiskProfile = env
            .storage()
            .persistent()
            .get(&DataKey::Profile(caller.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, RiskError::NotRegistered));

        let old_exposure = profile.exposure;
        profile.exposure += delta;

        // risk_score = (exposure * 100) / alert_threshold, capped at 100
        let raw_score = if profile.alert_threshold > 0 {
            (profile.exposure * 100) / profile.alert_threshold
        } else {
            100
        };
        profile.risk_score = if raw_score > 100 { 100 } else if raw_score < 0 { 0 } else { raw_score as u32 };

        env.storage()
            .persistent()
            .set(&DataKey::Profile(caller.clone()), &profile);

        // Update global metrics
        let mut metrics: RiskMetrics = env
            .storage()
            .persistent()
            .get(&DataKey::Metrics)
            .unwrap_or(RiskMetrics {
                total_exposure: 0,
                high_risk_count: 0,
                last_updated: 0,
            });
        metrics.total_exposure = metrics.total_exposure - old_exposure + profile.exposure;
        metrics.last_updated = env.ledger().timestamp();
        env.storage()
            .persistent()
            .set(&DataKey::Metrics, &metrics);

        env.events().publish(
            (symbol_short!("risk"), symbol_short!("exp_upd")),
            (caller.clone(), profile.exposure, profile.risk_score),
        );

        // Emit alert if threshold crossed
        if profile.exposure > profile.alert_threshold {
            env.events().publish(
                (symbol_short!("risk"), symbol_short!("alert")),
                (caller, profile.exposure, profile.alert_threshold),
            );
        }
    }

    pub fn check_exposure(env: Env, owner: Address) -> RiskProfile {
        env.storage()
            .persistent()
            .get(&DataKey::Profile(owner))
            .unwrap_or_else(|| panic_with_error!(&env, RiskError::NotRegistered))
    }

    pub fn enforce_limit(env: Env, caller: Address) {
        caller.require_auth();

        let profile: RiskProfile = env
            .storage()
            .persistent()
            .get(&DataKey::Profile(caller))
            .unwrap_or_else(|| panic_with_error!(&env, RiskError::NotRegistered));

        if profile.exposure > profile.alert_threshold {
            panic_with_error!(&env, RiskError::ExposureLimitExceeded);
        }
    }

    pub fn get_risk_metrics(env: Env) -> RiskMetrics {
        env.storage()
            .persistent()
            .get(&DataKey::Metrics)
            .unwrap_or(RiskMetrics {
                total_exposure: 0,
                high_risk_count: 0,
                last_updated: 0,
            })
    }

    pub fn reset_exposure(env: Env, admin: Address, owner: Address) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(&env, RiskError::Unauthorized));

        if stored_admin != admin {
            panic_with_error!(&env, RiskError::Unauthorized);
        }

        let mut profile: RiskProfile = env
            .storage()
            .persistent()
            .get(&DataKey::Profile(owner.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, RiskError::NotRegistered));

        let old_exposure = profile.exposure;
        profile.exposure = 0;
        profile.risk_score = 0;

        env.storage()
            .persistent()
            .set(&DataKey::Profile(owner), &profile);

        let mut metrics: RiskMetrics = env
            .storage()
            .persistent()
            .get(&DataKey::Metrics)
            .unwrap_or(RiskMetrics {
                total_exposure: 0,
                high_risk_count: 0,
                last_updated: 0,
            });
        metrics.total_exposure -= old_exposure;
        metrics.last_updated = env.ledger().timestamp();
        env.storage()
            .persistent()
            .set(&DataKey::Metrics, &metrics);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RiskManagementContract, ());
        let admin = Address::generate(&env);
        let client = RiskManagementContractClient::new(&env, &contract_id);
        client.init(&admin);
        (env, contract_id, admin)
    }

    #[test]
    fn test_exposure_update_recalculates_risk_score() {
        let (env, contract_id, _admin) = setup();
        let client = RiskManagementContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        client.register_participant(&user, &1000i128);
        client.update_exposure(&user, &500i128);

        let profile = client.check_exposure(&user);
        assert_eq!(profile.exposure, 500);
        assert_eq!(profile.risk_score, 50); // 500*100/1000 = 50
    }

    #[test]
    fn test_risk_score_capped_at_100() {
        let (env, contract_id, _admin) = setup();
        let client = RiskManagementContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        client.register_participant(&user, &100i128);
        client.update_exposure(&user, &200i128); // 200% -> capped at 100

        let profile = client.check_exposure(&user);
        assert_eq!(profile.risk_score, 100);
    }

    #[test]
    #[should_panic]
    fn test_enforce_limit_blocks_when_exceeded() {
        let (env, contract_id, _admin) = setup();
        let client = RiskManagementContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        client.register_participant(&user, &100i128);
        client.update_exposure(&user, &200i128);
        client.enforce_limit(&user);
    }

    #[test]
    fn test_enforce_limit_passes_when_under() {
        let (env, contract_id, _admin) = setup();
        let client = RiskManagementContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        client.register_participant(&user, &1000i128);
        client.update_exposure(&user, &500i128);
        client.enforce_limit(&user); // should not panic
    }

    #[test]
    #[should_panic]
    fn test_non_admin_reset_fails() {
        let (env, contract_id, _admin) = setup();
        let client = RiskManagementContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        let non_admin = Address::generate(&env);
        client.register_participant(&user, &1000i128);
        client.update_exposure(&user, &500i128);
        client.reset_exposure(&non_admin, &user);
    }

    #[test]
    fn test_admin_reset_succeeds() {
        let (env, contract_id, admin) = setup();
        let client = RiskManagementContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        client.register_participant(&user, &1000i128);
        client.update_exposure(&user, &500i128);
        client.reset_exposure(&admin, &user);

        let profile = client.check_exposure(&user);
        assert_eq!(profile.exposure, 0);
        assert_eq!(profile.risk_score, 0);
    }

    #[test]
    fn test_get_risk_metrics() {
        let (env, contract_id, _admin) = setup();
        let client = RiskManagementContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        client.register_participant(&user, &1000i128);
        client.update_exposure(&user, &300i128);

        let metrics = client.get_risk_metrics();
        assert_eq!(metrics.total_exposure, 300);
    }
}
