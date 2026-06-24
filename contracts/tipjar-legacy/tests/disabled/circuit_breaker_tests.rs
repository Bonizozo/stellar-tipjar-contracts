#![cfg(test)]

extern crate std;

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    Address, Env,
};
use tipjar::{CircuitBreakerConfig, TipJarContract, TipJarContractClient, TipJarError};

fn setup() -> (
    Env,
    TipJarContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TipJarContract);
    let client = TipJarContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);

    // Deploy a mock token.
    let token_id = env.register_stellar_asset_contract(token_admin.clone());

    // Init TipJar (version 0, refund window 0)
    client.init(&admin, &0u32, &0u64);
    client.add_token(&admin, &token_id);

    // Fund sender with tokens.
    let sender = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&sender, &1_000_000i128);

    (env, client, admin, sender, token_id)
}

#[test]
fn test_circuit_breaker_single_tip_spike() {
    let (env, client, admin, sender, token) = setup();
    let creator = Address::generate(&env);

    // Configure circuit breaker: 1000 max single tip, 1 hour window, 30 min cooldown
    let config = CircuitBreakerConfig {
        max_single_tip: 1000,
        max_volume_window: 5000,
        window_seconds: 3600,
        cooldown_seconds: 1800,
        enabled: true,
    };
    client.set_circuit_breaker_config(&admin, &config);

    // Tip within limit should succeed
    client.tip(&sender, &creator, &token, &500i128);
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 500i128);

    // Tip exceeding limit should trigger breaker and fail
    let result = client.try_tip(&sender, &creator, &token, &1001i128);
    assert!(result.is_err());

    // Subsequent tip should fail due to halt even if within limit
    let result2 = client.try_tip(&sender, &creator, &token, &100i128);
    assert!(result2.is_err());

    // Check state
    let state = client.get_circuit_breaker_state().unwrap();
    assert!(state.halted_until > env.ledger().timestamp());
    assert_eq!(state.trigger_count, 1);
}

#[test]
fn test_circuit_breaker_volume_spike() {
    let (env, client, admin, sender, token) = setup();
    let creator = Address::generate(&env);

    let config = CircuitBreakerConfig {
        max_single_tip: 10000,
        max_volume_window: 2000,
        window_seconds: 3600,
        cooldown_seconds: 1800,
        enabled: true,
    };
    client.set_circuit_breaker_config(&admin, &config);

    // Tip 1: 1500 (total 1500) - OK
    client.tip(&sender, &creator, &token, &1500i128);

    // Tip 2: 600 (total 2100) - Should trigger volume breaker
    let result = client.try_tip(&sender, &creator, &token, &600i128);
    assert!(result.is_err());

    let state = client.get_circuit_breaker_state().unwrap();
    assert!(state.halted_until > env.ledger().timestamp());
    assert_eq!(state.trigger_count, 1);
}

#[test]
fn test_circuit_breaker_cooldown_expiry() {
    let (env, client, admin, sender, token) = setup();
    let creator = Address::generate(&env);

    let config = CircuitBreakerConfig {
        max_single_tip: 1000,
        max_volume_window: 5000,
        window_seconds: 3600,
        cooldown_seconds: 1800,
        enabled: true,
    };
    client.set_circuit_breaker_config(&admin, &config);

    // Trigger breaker
    let _ = client.try_tip(&sender, &creator, &token, &1500i128);

    // Advance time past cooldown (1800s)
    env.ledger().with_mut(|l| l.timestamp += 1801);

    // Should succeed now
    client.tip(&sender, &creator, &token, &100i128);
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 100i128);
}

#[test]
fn test_manual_trigger_and_reset() {
    let (env, client, admin, sender, token) = setup();
    let creator = Address::generate(&env);

    let config = CircuitBreakerConfig {
        max_single_tip: 1000,
        max_volume_window: 5000,
        window_seconds: 3600,
        cooldown_seconds: 1800,
        enabled: true,
    };
    client.set_circuit_breaker_config(&admin, &config);

    // Manual trigger
    client.trigger_circuit_breaker(&admin, &symbol_short!("manual"));

    let result = client.try_tip(&sender, &creator, &token, &100i128);
    assert!(result.is_err());

    // Manual reset
    client.reset_circuit_breaker(&admin);

    client.tip(&sender, &creator, &token, &100i128);
    assert_eq!(client.get_withdrawable_balance(&creator, &token), 100i128);
}
