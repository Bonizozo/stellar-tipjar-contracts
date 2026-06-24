#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Env as _},
    Address, Env,
};

use tipjar::rental::{
    self, calculate_fee, RentalError, RentalKey,
};

// ── helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let creator = Address::generate(&env);
    let token = Address::generate(&env);
    (env, creator, token)
}

// ── listing tests ─────────────────────────────────────────────────────────────

#[test]
fn test_create_listing_success() {
    let (env, creator, token) = setup();

    let listing_id = rental::create_listing(&env, creator.clone(), token.clone(), 100, 86_400)
        .expect("create_listing should succeed");

    let listing = rental::get_listing(&env, listing_id).expect("listing should exist");
    assert_eq!(listing.listing_id, listing_id);
    assert_eq!(listing.creator, creator);
    assert_eq!(listing.fee_per_period, 100);
    assert_eq!(listing.period_seconds, 86_400);
    assert!(listing.active);
}

#[test]
fn test_create_listing_invalid_fee() {
    let (env, creator, token) = setup();
    let result = rental::create_listing(&env, creator, token, 0, 86_400);
    // Should panic with InvalidFee — we verify via should_panic
}

#[test]
#[should_panic]
fn test_create_listing_zero_fee_panics() {
    let (env, creator, token) = setup();
    rental::create_listing(&env, creator, token, 0, 86_400).unwrap();
}

#[test]
#[should_panic]
fn test_create_listing_zero_duration_panics() {
    let (env, creator, token) = setup();
    rental::create_listing(&env, creator, token, 100, 0).unwrap();
}

#[test]
fn test_creator_listings_tracked() {
    let (env, creator, token) = setup();

    rental::create_listing(&env, creator.clone(), token.clone(), 100, 86_400).unwrap();
    rental::create_listing(&env, creator.clone(), token.clone(), 200, 3_600).unwrap();

    let listings = rental::get_creator_listings(&env, &creator);
    assert_eq!(listings.len(), 2);
}

// ── deactivation tests ────────────────────────────────────────────────────────

#[test]
fn test_deactivate_listing() {
    let (env, creator, token) = setup();

    let listing_id =
        rental::create_listing(&env, creator.clone(), token, 100, 86_400).unwrap();

    rental::deactivate_listing(&env, creator.clone(), listing_id)
        .expect("deactivate should succeed");

    let listing = rental::get_listing(&env, listing_id).unwrap();
    assert!(!listing.active);
}

#[test]
#[should_panic]
fn test_deactivate_listing_wrong_creator_panics() {
    let (env, creator, token) = setup();
    let other = Address::generate(&env);

    let listing_id =
        rental::create_listing(&env, creator.clone(), token, 100, 86_400).unwrap();

    rental::deactivate_listing(&env, other, listing_id).unwrap();
}

// ── fee calculation tests ─────────────────────────────────────────────────────

#[test]
fn test_calculate_fee_single_period() {
    assert_eq!(calculate_fee(500, 1), 500);
}

#[test]
fn test_calculate_fee_multiple_periods() {
    assert_eq!(calculate_fee(100, 7), 700);
}

#[test]
fn test_calculate_fee_zero_periods() {
    assert_eq!(calculate_fee(100, 0), 0);
}

// ── access check tests ────────────────────────────────────────────────────────

#[test]
fn test_has_active_access_no_rental() {
    let (env, creator, token) = setup();
    let renter = Address::generate(&env);

    let listing_id =
        rental::create_listing(&env, creator, token, 100, 86_400).unwrap();

    assert!(!rental::has_active_access(&env, &renter, listing_id));
}

// ── history tests ─────────────────────────────────────────────────────────────

#[test]
fn test_renter_history_empty_initially() {
    let (env, _, _) = setup();
    let renter = Address::generate(&env);
    let history = rental::get_renter_history(&env, &renter);
    assert_eq!(history.len(), 0);
}

#[test]
fn test_get_nonexistent_listing_returns_none() {
    let (env, _, _) = setup();
    assert!(rental::get_listing(&env, 999).is_none());
}

#[test]
fn test_get_nonexistent_rental_returns_none() {
    let (env, _, _) = setup();
    assert!(rental::get_rental(&env, 999).is_none());
}
