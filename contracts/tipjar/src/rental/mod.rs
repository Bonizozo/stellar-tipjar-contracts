//! Tip Rental Module
//!
//! Provides temporary access to tip-gated content and benefits via time-based rentals.
//! Creators list content for rent; supporters pay a fee for a fixed duration.

use soroban_sdk::{contracterror, contracttype, panic_with_error, Address, Env, Vec};

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum RentalError {
    /// Rental listing not found.
    ListingNotFound = 1,
    /// Rental record not found.
    RentalNotFound = 2,
    /// Rental has already expired.
    RentalExpired = 3,
    /// Rental is still active; cannot re-rent until it expires.
    RentalStillActive = 4,
    /// Fee amount must be greater than zero.
    InvalidFee = 5,
    /// Duration must be greater than zero.
    InvalidDuration = 6,
    /// Caller is not the listing creator.
    Unauthorized = 7,
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// A creator's listing that supporters can rent for temporary access.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RentalListing {
    /// Unique listing ID.
    pub listing_id: u64,
    /// Creator who owns the content.
    pub creator: Address,
    /// Token used for payment.
    pub token: Address,
    /// Fee charged per rental period (in token units).
    pub fee_per_period: i128,
    /// Duration of one rental period in seconds.
    pub period_seconds: u64,
    /// Whether the listing is currently accepting rentals.
    pub active: bool,
    /// Timestamp when the listing was created.
    pub created_at: u64,
}

/// An active or expired rental record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RentalRecord {
    /// Unique rental ID.
    pub rental_id: u64,
    /// Listing this rental belongs to.
    pub listing_id: u64,
    /// Renter address.
    pub renter: Address,
    /// Creator address (denormalised for quick lookup).
    pub creator: Address,
    /// Fee paid.
    pub fee_paid: i128,
    /// Timestamp when access starts.
    pub start_time: u64,
    /// Timestamp when access expires.
    pub expires_at: u64,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RentalKey {
    /// Next listing ID counter.
    ListingCtr,
    /// Next rental ID counter.
    RentalCtr,
    /// Listing record keyed by listing_id.
    Listing(u64),
    /// Rental record keyed by rental_id.
    Record(u64),
    /// List of rental IDs for a renter.
    RenterHistory(Address),
    /// List of listing IDs for a creator.
    CreatorListings(Address),
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Create a new rental listing. Only the creator may call.
///
/// Emits `("rental_list",)` with data `(listing_id, creator, fee_per_period, period_seconds)`.
pub fn create_listing(
    env: &Env,
    creator: Address,
    token: Address,
    fee_per_period: i128,
    period_seconds: u64,
) -> Result<u64, RentalError> {
    creator.require_auth();

    if fee_per_period <= 0 {
        panic_with_error!(env, RentalError::InvalidFee);
    }
    if period_seconds == 0 {
        panic_with_error!(env, RentalError::InvalidDuration);
    }

    let listing_id = next_id(env, &RentalKey::ListingCtr);
    let now = env.ledger().timestamp();

    let listing = RentalListing {
        listing_id,
        creator: creator.clone(),
        token,
        fee_per_period,
        period_seconds,
        active: true,
        created_at: now,
    };

    env.storage()
        .persistent()
        .set(&RentalKey::Listing(listing_id), &listing);

    // Track listing under creator
    let mut creator_listings: Vec<u64> = env
        .storage()
        .persistent()
        .get(&RentalKey::CreatorListings(creator.clone()))
        .unwrap_or_else(|| Vec::new(env));
    creator_listings.push_back(listing_id);
    env.storage()
        .persistent()
        .set(&RentalKey::CreatorListings(creator.clone()), &creator_listings);

    env.events().publish(
        (soroban_sdk::symbol_short!("rnt_list"),),
        (listing_id, creator, fee_per_period, period_seconds),
    );

    Ok(listing_id)
}

/// Rent a listing. Transfers the fee from `renter` to the contract (held for creator).
///
/// Emits `("rental_start",)` with data `(rental_id, listing_id, renter, expires_at)`.
pub fn rent(
    env: &Env,
    renter: Address,
    listing_id: u64,
    creator_balance_key_fn: impl Fn(&Address, &Address) -> soroban_sdk::Val,
) -> Result<u64, RentalError> {
    renter.require_auth();

    let listing: RentalListing = env
        .storage()
        .persistent()
        .get(&RentalKey::Listing(listing_id))
        .ok_or(RentalError::ListingNotFound)?;

    if !listing.active {
        return Err(RentalError::ListingNotFound);
    }

    let now = env.ledger().timestamp();
    let expires_at = now.saturating_add(listing.period_seconds);

    let rental_id = next_id(env, &RentalKey::RentalCtr);

    let record = RentalRecord {
        rental_id,
        listing_id,
        renter: renter.clone(),
        creator: listing.creator.clone(),
        fee_paid: listing.fee_per_period,
        start_time: now,
        expires_at,
    };

    env.storage()
        .persistent()
        .set(&RentalKey::Record(rental_id), &record);

    // Track in renter history
    let mut history: Vec<u64> = env
        .storage()
        .persistent()
        .get(&RentalKey::RenterHistory(renter.clone()))
        .unwrap_or_else(|| Vec::new(env));
    history.push_back(rental_id);
    env.storage()
        .persistent()
        .set(&RentalKey::RenterHistory(renter.clone()), &history);

    // Transfer fee into contract via token client
    soroban_sdk::token::Client::new(env, &listing.token).transfer(
        &renter,
        &env.current_contract_address(),
        &listing.fee_per_period,
    );

    // Credit creator balance (reuse contract's persistent storage pattern)
    let bal_key = soroban_sdk::IntoVal::<Env, soroban_sdk::Val>::into_val(
        &(
            soroban_sdk::symbol_short!("cr_bal"),
            listing.creator.clone(),
            listing.token.clone(),
        ),
        env,
    );
    let current: i128 = env
        .storage()
        .persistent()
        .get(&bal_key)
        .unwrap_or(0i128);
    env.storage()
        .persistent()
        .set(&bal_key, &(current + listing.fee_per_period));

    env.events().publish(
        (soroban_sdk::symbol_short!("rnt_start"),),
        (rental_id, listing_id, renter, expires_at),
    );

    Ok(rental_id)
}

/// Check whether a renter currently has active access for a listing.
pub fn has_active_access(env: &Env, renter: &Address, listing_id: u64) -> bool {
    let now = env.ledger().timestamp();
    let history: Vec<u64> = env
        .storage()
        .persistent()
        .get(&RentalKey::RenterHistory(renter.clone()))
        .unwrap_or_else(|| Vec::new(env));

    for rental_id in history.iter() {
        if let Some(record) = env
            .storage()
            .persistent()
            .get::<RentalKey, RentalRecord>(&RentalKey::Record(rental_id))
        {
            if record.listing_id == listing_id && record.expires_at > now {
                return true;
            }
        }
    }
    false
}

/// Deactivate a listing. Only the creator may call.
///
/// Emits `("rental_deact",)` with data `listing_id`.
pub fn deactivate_listing(
    env: &Env,
    creator: Address,
    listing_id: u64,
) -> Result<(), RentalError> {
    creator.require_auth();

    let mut listing: RentalListing = env
        .storage()
        .persistent()
        .get(&RentalKey::Listing(listing_id))
        .ok_or(RentalError::ListingNotFound)?;

    if listing.creator != creator {
        panic_with_error!(env, RentalError::Unauthorized);
    }

    listing.active = false;
    env.storage()
        .persistent()
        .set(&RentalKey::Listing(listing_id), &listing);

    env.events()
        .publish((soroban_sdk::symbol_short!("rnt_dact"),), listing_id);

    Ok(())
}

/// Get a rental listing by ID.
pub fn get_listing(env: &Env, listing_id: u64) -> Option<RentalListing> {
    env.storage()
        .persistent()
        .get(&RentalKey::Listing(listing_id))
}

/// Get a rental record by ID.
pub fn get_rental(env: &Env, rental_id: u64) -> Option<RentalRecord> {
    env.storage()
        .persistent()
        .get(&RentalKey::Record(rental_id))
}

/// Get all rental IDs for a renter (history).
pub fn get_renter_history(env: &Env, renter: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&RentalKey::RenterHistory(renter.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Get all listing IDs for a creator.
pub fn get_creator_listings(env: &Env, creator: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&RentalKey::CreatorListings(creator.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Calculate the rental fee for a given number of periods.
pub fn calculate_fee(fee_per_period: i128, periods: u32) -> i128 {
    fee_per_period.saturating_mul(periods as i128)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn next_id(env: &Env, key: &RentalKey) -> u64 {
    let id: u64 = env.storage().persistent().get(key).unwrap_or(0);
    env.storage().persistent().set(key, &(id + 1));
    id
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rental_error_uniqueness() {
        assert_ne!(RentalError::ListingNotFound as u32, RentalError::RentalNotFound as u32);
        assert_ne!(RentalError::RentalExpired as u32, RentalError::RentalStillActive as u32);
        assert_ne!(RentalError::InvalidFee as u32, RentalError::InvalidDuration as u32);
    }

    #[test]
    fn test_calculate_fee() {
        assert_eq!(calculate_fee(100, 3), 300);
        assert_eq!(calculate_fee(0, 5), 0);
        assert_eq!(calculate_fee(50, 0), 0);
    }
}
