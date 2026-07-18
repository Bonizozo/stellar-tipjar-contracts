//! Tip Options Trading System
//!
//! This module provides options trading functionality for tip tokens,
//! allowing users to trade call and put options with automated pricing
//! and exercise mechanisms.

pub mod exercise;
pub mod pricing;

use soroban_sdk::{contracttype, Address, Env};

/// Type of option contract
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum OptionType {
    Call,
    Put,
}

/// Status of an option contract
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum OptionStatus {
    Active,
    Exercised,
    Expired,
    Cancelled,
}

/// Option contract definition
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptionContract {
    /// Unique option ID
    pub option_id: u64,
    /// Option type (Call or Put)
    pub option_type: OptionType,
    /// Option writer (seller)
    pub writer: Address,
    /// Option holder (buyer)
    pub holder: Option<Address>,
    /// Underlying tip token
    pub token: Address,
    /// Strike price in base units
    pub strike_price: i128,
    /// Premium paid for the option
    pub premium: i128,
    /// Amount of tokens covered
    pub amount: i128,
    /// Expiration timestamp
    pub expiration: u64,
    /// Creation timestamp
    pub created_at: u64,
    /// Current status
    pub status: OptionStatus,
    /// Collateral locked by writer
    pub collateral: i128,
}

/// Option position tracking for an address
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptionPosition {
    /// Address holding the position
    pub address: Address,
    /// Total options written
    pub written_count: u32,
    /// Total options held
    pub held_count: u32,
    /// Total collateral locked
    pub total_collateral: i128,
    /// Total premiums earned
    pub premiums_earned: i128,
    /// Total premiums paid
    pub premiums_paid: i128,
}

/// Option pricing parameters
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PricingParams {
    /// Volatility in basis points (e.g., 5000 = 50%)
    pub volatility_bps: u32,
    /// Risk-free rate in basis points (e.g., 500 = 5%)
    pub risk_free_rate_bps: u32,
    /// Minimum premium in basis points of strike price
    pub min_premium_bps: u32,
    /// Maximum premium in basis points of strike price
    pub max_premium_bps: u32,
}

/// Storage keys for options
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Option contract by ID
    Option(u64),
    /// Option counter for ID generation
    OptionCounter,
    /// Options written by address
    WrittenOptions(Address),
    /// Options held by address
    HeldOptions(Address),
    /// Position tracking for address
    Position(Address),
    /// Pricing parameters
    PricingParams,
    /// Active options list
    ActiveOptions,
    /// Expired options list
    ExpiredOptions,
    /// Collateral locked per token per address
    Collateral(Address, Address),
}

/// Default pricing parameters
pub const DEFAULT_VOLATILITY_BPS: u32 = 5000; // 50%
pub const DEFAULT_RISK_FREE_RATE_BPS: u32 = 500; // 5%
pub const DEFAULT_MIN_PREMIUM_BPS: u32 = 100; // 1%
pub const DEFAULT_MAX_PREMIUM_BPS: u32 = 5000; // 50%

/// Collateral requirements
pub const CALL_COLLATERAL_RATIO: i128 = 10000; // 100% of amount
pub const PUT_COLLATERAL_RATIO: i128 = 10000; // 100% of strike * amount

/// Initialize options trading system
pub fn init_options(env: &Env) {
    let params = PricingParams {
        volatility_bps: DEFAULT_VOLATILITY_BPS,
        risk_free_rate_bps: DEFAULT_RISK_FREE_RATE_BPS,
        min_premium_bps: DEFAULT_MIN_PREMIUM_BPS,
        max_premium_bps: DEFAULT_MAX_PREMIUM_BPS,
    };

    env.storage()
        .persistent()
        .set(&DataKey::PricingParams, &params);

    env.storage().instance().set(&DataKey::OptionCounter, &0u64);
}

/// Get pricing parameters
pub fn get_pricing_params(env: &Env) -> PricingParams {
    env.storage()
        .persistent()
        .get(&DataKey::PricingParams)
        .unwrap_or(PricingParams {
            volatility_bps: DEFAULT_VOLATILITY_BPS,
            risk_free_rate_bps: DEFAULT_RISK_FREE_RATE_BPS,
            min_premium_bps: DEFAULT_MIN_PREMIUM_BPS,
            max_premium_bps: DEFAULT_MAX_PREMIUM_BPS,
        })
}

/// Update pricing parameters (admin only)
pub fn update_pricing_params(env: &Env, params: &PricingParams) {
    env.storage()
        .persistent()
        .set(&DataKey::PricingParams, params);
}

/// Get option contract by ID
pub fn get_option(env: &Env, option_id: u64) -> Option<OptionContract> {
    env.storage().persistent().get(&DataKey::Option(option_id))
}

/// Get option contract or panic
pub fn get_option_or_panic(env: &Env, option_id: u64) -> OptionContract {
    get_option(env, option_id).expect("Option not found")
}

/// Update option contract
pub fn update_option(env: &Env, option: &OptionContract) {
    env.storage()
        .persistent()
        .set(&DataKey::Option(option.option_id), option);
}

/// Get position for an address
pub fn get_position(env: &Env, address: &Address) -> OptionPosition {
    env.storage()
        .persistent()
        .get(&DataKey::Position(address.clone()))
        .unwrap_or(OptionPosition {
            address: address.clone(),
            written_count: 0,
            held_count: 0,
            total_collateral: 0,
            premiums_earned: 0,
            premiums_paid: 0,
        })
}

/// Update position for an address
pub fn update_position(env: &Env, position: &OptionPosition) {
    env.storage()
        .persistent()
        .set(&DataKey::Position(position.address.clone()), position);
}

/// Calculate required collateral for an option
pub fn calculate_collateral(option_type: OptionType, strike_price: i128, amount: i128) -> i128 {
    match option_type {
        OptionType::Call => {
            // For calls, collateral is the full amount of tokens
            amount
        }
        OptionType::Put => {
            // For puts, collateral is strike_price * amount
            strike_price
                .checked_mul(amount)
                .expect("Collateral calculation overflow")
                / 1_000_000 // Normalize for precision
        }
    }
}

/// Add option to written list
pub fn add_written_option(env: &Env, writer: &Address, option_id: u64) {
    let mut options: soroban_sdk::Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::WrittenOptions(writer.clone()))
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    if !options.contains(option_id) {
        options.push_back(option_id);
        env.storage()
            .persistent()
            .set(&DataKey::WrittenOptions(writer.clone()), &options);
    }
}

/// Add option to held list
pub fn add_held_option(env: &Env, holder: &Address, option_id: u64) {
    let mut options: soroban_sdk::Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::HeldOptions(holder.clone()))
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    if !options.contains(option_id) {
        options.push_back(option_id);
        env.storage()
            .persistent()
            .set(&DataKey::HeldOptions(holder.clone()), &options);
    }
}

/// Remove option from held list
pub fn remove_held_option(env: &Env, holder: &Address, option_id: u64) {
    let mut options: soroban_sdk::Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::HeldOptions(holder.clone()))
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    if let Some(index) = options.iter().position(|id| id == option_id) {
        options.remove(index as u32);
        env.storage()
            .persistent()
            .set(&DataKey::HeldOptions(holder.clone()), &options);
    }
}

/// Add option to active list
pub fn add_active_option(env: &Env, option_id: u64) {
    let mut options: soroban_sdk::Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::ActiveOptions)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    if !options.contains(option_id) {
        options.push_back(option_id);
        env.storage()
            .persistent()
            .set(&DataKey::ActiveOptions, &options);
    }
}

/// Remove option from active list
pub fn remove_active_option(env: &Env, option_id: u64) {
    let mut options: soroban_sdk::Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::ActiveOptions)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    if let Some(index) = options.iter().position(|id| id == option_id) {
        options.remove(index as u32);
        env.storage()
            .persistent()
            .set(&DataKey::ActiveOptions, &options);
    }
}

/// Get collateral locked for an address and token
pub fn get_locked_collateral(env: &Env, address: &Address, token: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::Collateral(address.clone(), token.clone()))
        .unwrap_or(0)
}

/// Update locked collateral
pub fn update_locked_collateral(env: &Env, address: &Address, token: &Address, amount: i128) {
    env.storage().persistent().set(
        &DataKey::Collateral(address.clone(), token.clone()),
        &amount,
    );
}
