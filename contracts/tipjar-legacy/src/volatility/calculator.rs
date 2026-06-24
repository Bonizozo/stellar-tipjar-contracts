//! Volatility metric calculations.
//!
//! All values are scaled by `PRECISION` (1_000_000) to preserve integer
//! precision throughout.  The final volatility index is expressed in
//! basis points (10 000 = 100 %).

use soroban_sdk::{Env, Vec};

use super::{VolObservation, BPS_DENOM, PRECISION};

/// Compute the population mean and variance of a window of observations.
///
/// Returns `(mean × PRECISION, variance × PRECISION)`.
/// Both are 0 when the window has fewer than 2 observations.
pub fn compute_mean_variance(_env: &Env, window: &Vec<VolObservation>) -> (i128, i128) {
    let n = window.len() as i128;
    if n < 2 {
        if n == 1 {
            let amt = window.get(0).unwrap().amount;
            return (amt * PRECISION, 0);
        }
        return (0, 0);
    }

    // ── Pass 1: mean ─────────────────────────────────────────────────────────
    let mut sum: i128 = 0;
    for i in 0..window.len() {
        sum += window.get(i).unwrap().amount;
    }
    // mean scaled by PRECISION
    let mean_scaled = sum * PRECISION / n;

    // ── Pass 2: population variance ──────────────────────────────────────────
    // variance = Σ (x_i - mean)² / n
    // We work in PRECISION-scaled space to avoid losing small differences.
    let mut sq_sum: i128 = 0;
    for i in 0..window.len() {
        let x = window.get(i).unwrap().amount * PRECISION;
        let diff = x - mean_scaled;
        // diff² / PRECISION keeps the scale at PRECISION
        sq_sum += diff / PRECISION * diff / PRECISION;
    }
    let variance_scaled = sq_sum / n;

    (mean_scaled, variance_scaled)
}

/// Convert variance (× PRECISION) and mean (× PRECISION) to a volatility
/// index expressed in basis points.
///
/// `volatility_bps = sqrt(variance) / mean × BPS_DENOM`
///
/// This is the coefficient of variation (CV) expressed in basis points —
/// a dimensionless measure of relative dispersion.
/// Returns 0 when mean is 0.
pub fn variance_to_volatility_bps(mean_scaled: i128, variance_scaled: i128) -> i128 {
    if mean_scaled <= 0 || variance_scaled <= 0 {
        return 0;
    }

    let std_dev = integer_sqrt(variance_scaled);
    // volatility_bps = (std_dev / mean) × BPS_DENOM
    // Both std_dev and mean are already × PRECISION, so they cancel.
    std_dev * BPS_DENOM / mean_scaled
}

/// Integer square root via Newton's method (floor).
pub fn integer_sqrt(n: i128) -> i128 {
    if n <= 0 {
        return 0;
    }
    if n == 1 {
        return 1;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Compute the simple moving average of the last `window` observations.
/// Returns the average × PRECISION, or 0 if the window is empty.
pub fn simple_moving_average(window: &Vec<VolObservation>) -> i128 {
    let n = window.len() as i128;
    if n == 0 {
        return 0;
    }
    let mut sum: i128 = 0;
    for i in 0..window.len() {
        sum += window.get(i).unwrap().amount;
    }
    sum * PRECISION / n
}

/// Compute the maximum drawdown within the window as basis points.
///
/// `max_drawdown_bps = (peak - trough) / peak × BPS_DENOM`
///
/// Returns 0 if the window has fewer than 2 observations or peak is 0.
pub fn max_drawdown_bps(window: &Vec<VolObservation>) -> i128 {
    let n = window.len();
    if n < 2 {
        return 0;
    }

    let mut peak: i128 = 0;
    let mut max_dd: i128 = 0;

    for i in 0..n {
        let amt = window.get(i).unwrap().amount;
        if amt > peak {
            peak = amt;
        }
        if peak > 0 {
            let dd = (peak - amt) * BPS_DENOM / peak;
            if dd > max_dd {
                max_dd = dd;
            }
        }
    }

    max_dd
}

/// Compute the rate of change between the oldest and newest observation
/// in the window, expressed in basis points.
///
/// Positive = price rose; negative = price fell.
/// Returns 0 if the window has fewer than 2 observations or oldest is 0.
pub fn rate_of_change_bps(window: &Vec<VolObservation>) -> i128 {
    let n = window.len();
    if n < 2 {
        return 0;
    }
    let oldest = window.get(0).unwrap().amount;
    let newest = window.get(n - 1).unwrap().amount;
    if oldest == 0 {
        return 0;
    }
    (newest - oldest) * BPS_DENOM / oldest
}
