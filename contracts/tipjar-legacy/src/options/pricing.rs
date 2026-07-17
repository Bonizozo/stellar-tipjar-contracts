//! Option Pricing Module
//!
//! Implements simplified Black-Scholes-inspired pricing for tip token options.
//! Uses approximations suitable for on-chain computation.

use soroban_sdk::Env;

use super::{OptionType, PricingParams};

/// Calculate option premium using simplified pricing model
///
/// This uses a simplified approximation of Black-Scholes pricing
/// suitable for on-chain computation without floating point math.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `option_type` - Call or Put
/// * `spot_price` - Current market price
/// * `strike_price` - Option strike price
/// * `amount` - Number of tokens
/// * `time_to_expiry` - Seconds until expiration
/// * `params` - Pricing parameters (volatility, risk-free rate, etc.)
///
/// # Returns
/// Premium amount in base units
pub fn calculate_premium(
    env: &Env,
    option_type: OptionType,
    spot_price: i128,
    strike_price: i128,
    amount: i128,
    time_to_expiry: u64,
    params: &PricingParams,
) -> i128 {
    // Validate inputs
    if spot_price <= 0 || strike_price <= 0 || amount <= 0 {
        return 0;
    }

    // Calculate intrinsic value
    let intrinsic_value = calculate_intrinsic_value(option_type, spot_price, strike_price, amount);

    // Calculate time value
    let time_value = calculate_time_value(
        env,
        spot_price,
        strike_price,
        amount,
        time_to_expiry,
        params,
    );

    // Total premium is intrinsic + time value
    let total_premium = intrinsic_value.checked_add(time_value).unwrap_or(i128::MAX);

    // Apply min/max bounds
    let min_premium = (strike_price * amount * params.min_premium_bps as i128) / 10_000_000_000;
    let max_premium = (strike_price * amount * params.max_premium_bps as i128) / 10_000_000_000;

    total_premium.max(min_premium).min(max_premium)
}

/// Calculate intrinsic value of an option
///
/// Intrinsic value is the immediate exercise value:
/// - Call: max(spot - strike, 0) * amount
/// - Put: max(strike - spot, 0) * amount
fn calculate_intrinsic_value(
    option_type: OptionType,
    spot_price: i128,
    strike_price: i128,
    amount: i128,
) -> i128 {
    let value_per_unit = match option_type {
        OptionType::Call => {
            if spot_price > strike_price {
                spot_price - strike_price
            } else {
                0
            }
        }
        OptionType::Put => {
            if strike_price > spot_price {
                strike_price - spot_price
            } else {
                0
            }
        }
    };

    value_per_unit.checked_mul(amount).unwrap_or(i128::MAX) / 1_000_000 // Normalize for precision
}

/// Calculate time value of an option
///
/// Uses a simplified model based on:
/// - Volatility
/// - Time to expiration
/// - Moneyness (spot vs strike)
fn calculate_time_value(
    _env: &Env,
    spot_price: i128,
    strike_price: i128,
    amount: i128,
    time_to_expiry: u64,
    params: &PricingParams,
) -> i128 {
    // Convert time to years (approximation: 365.25 days)
    let seconds_per_year: u64 = 31_557_600;
    let time_factor = if time_to_expiry >= seconds_per_year {
        10_000 // Cap at 1 year = 100%
    } else {
        (time_to_expiry as i128 * 10_000) / seconds_per_year as i128
    };

    // Calculate moneyness factor (how close spot is to strike)
    let moneyness = if spot_price > strike_price {
        (spot_price * 10_000) / strike_price
    } else {
        (strike_price * 10_000) / spot_price
    };

    // At-the-money options have highest time value
    // Adjust based on how far from ATM
    let atm_adjustment = if moneyness > 10_000 {
        // Out of the money or in the money
        let deviation = moneyness - 10_000;
        // Reduce time value as we move away from ATM
        10_000 - (deviation / 10).min(5000)
    } else {
        10_000
    };

    // Base time value calculation
    // time_value = spot * amount * volatility * sqrt(time) * atm_adjustment
    // Simplified: time_value = spot * amount * volatility * time_factor * atm_adjustment / 100_000_000

    let base_value = spot_price.checked_mul(amount).unwrap_or(i128::MAX) / 1_000_000; // Normalize

    let volatility_adjusted = base_value
        .checked_mul(params.volatility_bps as i128)
        .unwrap_or(i128::MAX)
        / 10_000;

    let time_adjusted = volatility_adjusted
        .checked_mul(time_factor)
        .unwrap_or(i128::MAX)
        / 10_000;

    let final_value = time_adjusted
        .checked_mul(atm_adjustment)
        .unwrap_or(i128::MAX)
        / 10_000;

    final_value.max(0)
}

/// Calculate option payoff at exercise
///
/// # Arguments
/// * `option_type` - Call or Put
/// * `spot_price` - Current market price at exercise
/// * `strike_price` - Option strike price
/// * `amount` - Number of tokens
///
/// # Returns
/// Payoff amount (0 if out of the money)
pub fn calculate_payoff(
    option_type: OptionType,
    spot_price: i128,
    strike_price: i128,
    amount: i128,
) -> i128 {
    calculate_intrinsic_value(option_type, spot_price, strike_price, amount)
}

/// Estimate implied volatility from market prices
///
/// This is a simplified estimation that could be enhanced with
/// more sophisticated algorithms in the future.
pub fn estimate_volatility(recent_prices: &[i128], window_seconds: u64) -> u32 {
    if recent_prices.len() < 2 {
        return DEFAULT_VOLATILITY_BPS;
    }

    // Calculate returns
    let mut sum_squared_returns: i128 = 0;
    let mut count: i128 = 0;

    for i in 1..recent_prices.len() {
        let prev = recent_prices[i - 1];
        let curr = recent_prices[i];

        if prev > 0 {
            // Calculate return as (curr - prev) / prev
            let return_val = ((curr - prev) * 10_000) / prev;
            let squared = (return_val * return_val) / 10_000;
            sum_squared_returns = sum_squared_returns.saturating_add(squared);
            count += 1;
        }
    }

    if count == 0 {
        return DEFAULT_VOLATILITY_BPS;
    }

    // Variance = average of squared returns
    let variance = sum_squared_returns / count;

    // Annualize the variance
    let periods_per_year = 31_557_600 / window_seconds.max(1);
    let annualized_variance = variance.saturating_mul(periods_per_year as i128);

    // Approximate square root for volatility
    // Using a simple approximation: sqrt(x) ≈ x / sqrt_approx
    let volatility = approximate_sqrt(annualized_variance);

    // Cap between reasonable bounds
    volatility.min(20_000).max(100) as u32 // 1% to 200%
}

/// Approximate square root using Newton's method
/// Returns result scaled by 10_000
fn approximate_sqrt(x: i128) -> i128 {
    if x <= 0 {
        return 0;
    }
    if x == 1 {
        return 10_000;
    }

    // Initial guess
    let mut guess = x / 2;
    if guess == 0 {
        guess = 1;
    }

    // Newton's method iterations
    for _ in 0..10 {
        let next_guess = (guess + x / guess) / 2;
        if (next_guess - guess).abs() < 10 {
            break;
        }
        guess = next_guess;
    }

    guess
}

/// Default volatility in basis points
const DEFAULT_VOLATILITY_BPS: u32 = 5000; // 50%

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intrinsic_value_call() {
        // Call option: spot > strike
        let value = calculate_intrinsic_value(
            OptionType::Call,
            1_200_000,  // spot
            1_000_000,  // strike
            10_000_000, // amount
        );
        assert!(value > 0);

        // Call option: spot < strike (out of money)
        let value = calculate_intrinsic_value(OptionType::Call, 800_000, 1_000_000, 10_000_000);
        assert_eq!(value, 0);
    }

    #[test]
    fn test_intrinsic_value_put() {
        // Put option: strike > spot
        let value = calculate_intrinsic_value(OptionType::Put, 800_000, 1_000_000, 10_000_000);
        assert!(value > 0);

        // Put option: strike < spot (out of money)
        let value = calculate_intrinsic_value(OptionType::Put, 1_200_000, 1_000_000, 10_000_000);
        assert_eq!(value, 0);
    }

    #[test]
    fn test_approximate_sqrt() {
        assert_eq!(approximate_sqrt(0), 0);
        assert_eq!(approximate_sqrt(1), 10_000);

        let sqrt_4 = approximate_sqrt(4);
        assert!((1..=3).contains(&sqrt_4));
    }
}
