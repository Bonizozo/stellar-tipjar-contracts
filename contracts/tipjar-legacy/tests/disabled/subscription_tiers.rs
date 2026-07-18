#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token, Address, Env, String,
};
use tipjar::{
    Subscription, SubscriptionStatus, SubscriptionTier, TierConfig, TipJarContract,
    TipJarContractClient, TipJarError,
};

const ONE_MONTH: u64 = 2_592_000;

// ── setup ─────────────────────────────────────────────────────────────────────

fn setup() -> (
    Env,
    TipJarContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let admin = Address::generate(&env);
    let contract_id = env.register(TipJarContract, ());
    let client = TipJarContractClient::new(&env, &contract_id);

    client.init(&admin);
    client.add_token(&admin, &token);

    (env, client, admin, token, token_admin)
}

fn mint(env: &Env, token_admin: &Address, token: &Address, user: &Address, amount: i128) {
    token::StellarAssetClient::new(env, token).mint(user, &amount);
}

fn balance(env: &Env, token: &Address, user: &Address) -> i128 {
    token::Client::new(env, token).balance(user)
}

// ── tier config ───────────────────────────────────────────────────────────────

#[test]
fn test_set_and_get_tier_config() {
    let (env, client, admin, _token, _ta) = setup();
    client.set_tier_config(
        &admin,
        &SubscriptionTier::Silver,
        &500,
        &String::from_str(&env, "Silver benefits"),
    );
    let cfg: TierConfig = client.get_tier_config(&SubscriptionTier::Silver).unwrap();
    assert_eq!(cfg.price, 500);
    assert_eq!(cfg.benefits, String::from_str(&env, "Silver benefits"));
}

#[test]
fn test_get_tier_config_returns_none_when_not_set() {
    let (_env, client, _admin, _token, _ta) = setup();
    assert!(client.get_tier_config(&SubscriptionTier::Gold).is_none());
}

#[test]
fn test_set_tier_config_unauthorized() {
    let (env, client, _admin, _token, _ta) = setup();
    let not_admin = Address::generate(&env);
    let result = client.try_set_tier_config(
        &not_admin,
        &SubscriptionTier::Bronze,
        &100,
        &String::from_str(&env, "Bronze"),
    );
    assert!(result.is_err());
}

#[test]
fn test_set_tier_config_rejects_zero_price() {
    let (env, client, admin, _token, _ta) = setup();
    let result = client.try_set_tier_config(
        &admin,
        &SubscriptionTier::Bronze,
        &0,
        &String::from_str(&env, "Bronze"),
    );
    assert!(result.is_err());
}

// ── get_tier_benefits ─────────────────────────────────────────────────────────

#[test]
fn test_get_tier_benefits_returns_description() {
    let (env, client, admin, _token, _ta) = setup();
    client.set_tier_config(
        &admin,
        &SubscriptionTier::Gold,
        &1000,
        &String::from_str(&env, "Gold: exclusive access"),
    );
    let benefits = client.get_tier_benefits(&SubscriptionTier::Gold).unwrap();
    assert_eq!(benefits, String::from_str(&env, "Gold: exclusive access"));
}

#[test]
fn test_get_tier_benefits_returns_none_when_not_configured() {
    let (_env, client, _admin, _token, _ta) = setup();
    assert!(client
        .get_tier_benefits(&SubscriptionTier::Silver)
        .is_none());
}

// ── create_tiered_subscription ────────────────────────────────────────────────

#[test]
fn test_create_tiered_subscription_uses_tier_price() {
    let (env, client, admin, token, ta) = setup();
    client.set_tier_config(
        &admin,
        &SubscriptionTier::Silver,
        &300,
        &String::from_str(&env, "Silver"),
    );
    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &ta, &token, &subscriber, 10_000);

    client.create_tiered_subscription(
        &subscriber,
        &creator,
        &token,
        &SubscriptionTier::Silver,
        &ONE_MONTH,
    );

    let sub: Subscription = client.get_subscription(&subscriber, &creator).unwrap();
    assert_eq!(sub.amount, 300);
    assert_eq!(sub.status, SubscriptionStatus::Active);
    assert_eq!(sub.tier, SubscriptionTier::Silver);
    assert!(sub.pending_tier.is_none());
}

#[test]
fn test_create_tiered_subscription_fails_when_tier_not_configured() {
    let (env, client, _admin, token, ta) = setup();
    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &ta, &token, &subscriber, 10_000);

    let result = client.try_create_tiered_subscription(
        &subscriber,
        &creator,
        &token,
        &SubscriptionTier::Gold,
        &ONE_MONTH,
    );
    assert!(result.is_err());
}

// ── upgrade_subscription ──────────────────────────────────────────────────────

#[test]
fn test_upgrade_executes_immediate_payment_at_new_price() {
    let (env, client, admin, token, ta) = setup();
    client.set_tier_config(
        &admin,
        &SubscriptionTier::Bronze,
        &100,
        &String::from_str(&env, "Bronze"),
    );
    client.set_tier_config(
        &admin,
        &SubscriptionTier::Gold,
        &500,
        &String::from_str(&env, "Gold"),
    );
    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &ta, &token, &subscriber, 10_000);

    client.create_tiered_subscription(
        &subscriber,
        &creator,
        &token,
        &SubscriptionTier::Bronze,
        &ONE_MONTH,
    );

    client.upgrade_subscription(&subscriber, &creator, &SubscriptionTier::Gold);

    // Subscriber paid 500 immediately.
    assert_eq!(balance(&env, &token, &subscriber), 9_500);

    let sub: Subscription = client.get_subscription(&subscriber, &creator).unwrap();
    assert_eq!(sub.tier, SubscriptionTier::Gold);
    assert_eq!(sub.amount, 500);
    assert!(sub.pending_tier.is_none());
}

#[test]
fn test_upgrade_fails_when_tier_not_configured() {
    let (env, client, admin, token, ta) = setup();
    client.set_tier_config(
        &admin,
        &SubscriptionTier::Bronze,
        &100,
        &String::from_str(&env, "Bronze"),
    );
    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &ta, &token, &subscriber, 10_000);

    client.create_tiered_subscription(
        &subscriber,
        &creator,
        &token,
        &SubscriptionTier::Bronze,
        &ONE_MONTH,
    );

    let result = client.try_upgrade_subscription(&subscriber, &creator, &SubscriptionTier::Gold);
    assert!(result.is_err());
}

// ── downgrade_subscription ────────────────────────────────────────────────────

#[test]
fn test_downgrade_schedules_pending_tier() {
    let (env, client, admin, token, ta) = setup();
    client.set_tier_config(
        &admin,
        &SubscriptionTier::Gold,
        &500,
        &String::from_str(&env, "Gold"),
    );
    client.set_tier_config(
        &admin,
        &SubscriptionTier::Bronze,
        &100,
        &String::from_str(&env, "Bronze"),
    );
    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &ta, &token, &subscriber, 10_000);

    client.create_tiered_subscription(
        &subscriber,
        &creator,
        &token,
        &SubscriptionTier::Gold,
        &ONE_MONTH,
    );

    client.downgrade_subscription(&subscriber, &creator, &SubscriptionTier::Bronze);

    let sub: Subscription = client.get_subscription(&subscriber, &creator).unwrap();
    // Current tier unchanged until next payment.
    assert_eq!(sub.tier, SubscriptionTier::Gold);
    assert_eq!(sub.amount, 500);
    assert_eq!(sub.pending_tier, Some(SubscriptionTier::Bronze));
}

#[test]
fn test_downgrade_applied_at_next_payment() {
    let (env, client, admin, token, ta) = setup();
    client.set_tier_config(
        &admin,
        &SubscriptionTier::Gold,
        &500,
        &String::from_str(&env, "Gold"),
    );
    client.set_tier_config(
        &admin,
        &SubscriptionTier::Bronze,
        &100,
        &String::from_str(&env, "Bronze"),
    );
    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &ta, &token, &subscriber, 10_000);

    client.create_tiered_subscription(
        &subscriber,
        &creator,
        &token,
        &SubscriptionTier::Gold,
        &ONE_MONTH,
    );

    // Execute first payment at Gold price.
    client.execute_subscription_payment(&subscriber, &creator);
    assert_eq!(balance(&env, &token, &subscriber), 9_500);

    // Schedule downgrade.
    client.downgrade_subscription(&subscriber, &creator, &SubscriptionTier::Bronze);

    // Advance time and execute second payment — should apply Bronze price.
    env.ledger().with_mut(|li| li.timestamp += ONE_MONTH);
    client.execute_subscription_payment(&subscriber, &creator);

    // Second payment was 100 (Bronze), not 500 (Gold).
    assert_eq!(balance(&env, &token, &subscriber), 9_400);

    let sub: Subscription = client.get_subscription(&subscriber, &creator).unwrap();
    assert_eq!(sub.tier, SubscriptionTier::Bronze);
    assert_eq!(sub.amount, 100);
    assert!(sub.pending_tier.is_none());
}

#[test]
fn test_downgrade_fails_when_tier_not_configured() {
    let (env, client, admin, token, ta) = setup();
    client.set_tier_config(
        &admin,
        &SubscriptionTier::Gold,
        &500,
        &String::from_str(&env, "Gold"),
    );
    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &ta, &token, &subscriber, 10_000);

    client.create_tiered_subscription(
        &subscriber,
        &creator,
        &token,
        &SubscriptionTier::Gold,
        &ONE_MONTH,
    );

    let result =
        client.try_downgrade_subscription(&subscriber, &creator, &SubscriptionTier::Bronze);
    assert!(result.is_err());
}

// ── status tracking ───────────────────────────────────────────────────────────

#[test]
fn test_tiered_subscription_pause_resume_cancel() {
    let (env, client, admin, token, ta) = setup();
    client.set_tier_config(
        &admin,
        &SubscriptionTier::Silver,
        &200,
        &String::from_str(&env, "Silver"),
    );
    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &ta, &token, &subscriber, 10_000);

    client.create_tiered_subscription(
        &subscriber,
        &creator,
        &token,
        &SubscriptionTier::Silver,
        &ONE_MONTH,
    );

    client.pause_subscription(&subscriber, &creator);
    let sub: Subscription = client.get_subscription(&subscriber, &creator).unwrap();
    assert_eq!(sub.status, SubscriptionStatus::Paused);

    client.resume_subscription(&subscriber, &creator);
    let sub: Subscription = client.get_subscription(&subscriber, &creator).unwrap();
    assert_eq!(sub.status, SubscriptionStatus::Active);

    client.cancel_subscription(&subscriber, &creator);
    let sub: Subscription = client.get_subscription(&subscriber, &creator).unwrap();
    assert_eq!(sub.status, SubscriptionStatus::Cancelled);
}

// ── recurring payments ────────────────────────────────────────────────────────

#[test]
fn test_recurring_payments_at_tier_price() {
    let (env, client, admin, token, ta) = setup();
    client.set_tier_config(
        &admin,
        &SubscriptionTier::Silver,
        &250,
        &String::from_str(&env, "Silver"),
    );
    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &ta, &token, &subscriber, 10_000);

    client.create_tiered_subscription(
        &subscriber,
        &creator,
        &token,
        &SubscriptionTier::Silver,
        &ONE_MONTH,
    );

    // First payment.
    client.execute_subscription_payment(&subscriber, &creator);
    assert_eq!(balance(&env, &token, &subscriber), 9_750);

    // Second payment after interval.
    env.ledger().with_mut(|li| li.timestamp += ONE_MONTH);
    client.execute_subscription_payment(&subscriber, &creator);
    assert_eq!(balance(&env, &token, &subscriber), 9_500);
}

#[test]
fn test_payment_not_due_before_interval() {
    let (env, client, admin, token, ta) = setup();
    client.set_tier_config(
        &admin,
        &SubscriptionTier::Bronze,
        &100,
        &String::from_str(&env, "Bronze"),
    );
    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &ta, &token, &subscriber, 10_000);

    client.create_tiered_subscription(
        &subscriber,
        &creator,
        &token,
        &SubscriptionTier::Bronze,
        &ONE_MONTH,
    );

    client.execute_subscription_payment(&subscriber, &creator);

    // Advance less than the full interval.
    env.ledger().with_mut(|li| li.timestamp += ONE_MONTH - 1);
    let result = client.try_execute_subscription_payment(&subscriber, &creator);
    assert!(result.is_err());
}
