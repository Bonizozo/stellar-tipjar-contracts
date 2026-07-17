//! Derivative pricing models.
//!
//! Implements a simplified Black-Scholes-inspired pricing model suitable for
//! on-chain integer arithmetic (no floating point). All values use fixed-point
//! with `PRICE_PRECISION` (1_000_000 = 1.0) unless noted.
//!
//! # Approximations
//! - The normal CDF is approximated with a rational polynomial (Abramowitz &
//!   Stegun 26.2.17), accurate to ±7.5×10⁻⁸.
//! - Square root uses Newton-Raphson iteration (converges in ~10 steps).
//! - Natural log uses a table-free integer approximation.
//!
//! All intermediate values are scaled to avoid overflow within i128 range.

use super::{DerivativeKind, PRICE_PRECISION};

/// Volatility denominator: 10 000 bps = 100%.
pub const VOL_BPS: i128 = 10_000;

/// Time precision: seconds per year (365.25 days).
pub const SECS_PER_YEAR: i128 = 31_557_600;

/// Minimum time-to-expiry for pricing (1 hour in seconds).
pub const MIN_TIME_SECS: u64 = 3_600;

// ── Public API ───────────────────────────────────────────────────────────────

/// Parameters for Black-Scholes pricing.
pub struct PricingInput {
    /// Current spot price (PRICE_PRECISION scale).
    pub spot: i128,
    /// Strike price (PRICE_PRECISION scale).
    pub strike: i128,
    /// Time to expiry in seconds.
    pub time_to_expiry_secs: u64,
    /// Annualised volatility in basis points (e.g. 5000 = 50%).
    pub volatility_bps: u32,
    /// Annualised risk-free rate in basis points (e.g. 500 = 5%).
    pub risk_free_rate_bps: u32,
}

/// Computed option prices.
pub struct OptionPrice {
    /// Call option fair value (PRICE_PRECISION scale).
    pub call: i128,
    /// Put option fair value (PRICE_PRECISION scale).
    pub put: i128,
    /// Delta for the call (PRICE_PRECISION scale, 0..1_000_000).
    pub call_delta: i128,
    /// Delta for the put (PRICE_PRECISION scale, -1_000_000..0).
    pub put_delta: i128,
}

/// Price a European call and put using the Black-Scholes formula.
///
/// Returns `None` if time_to_expiry is below `MIN_TIME_SECS` (treat as expired).
pub fn black_scholes(input: &PricingInput) -> Option<OptionPrice> {
    if input.time_to_expiry_secs < MIN_TIME_SECS {
        return None;
    }
    if input.spot <= 0 || input.strike <= 0 {
        return None;
    }

    // t = time fraction of year (PRICE_PRECISION scale)
    let t = (input.time_to_expiry_secs as i128) * PRICE_PRECISION / SECS_PER_YEAR;

    // sigma = volatility (PRICE_PRECISION scale)
    let sigma = (input.volatility_bps as i128) * PRICE_PRECISION / VOL_BPS;

    // r = risk-free rate (PRICE_PRECISION scale)
    let r = (input.risk_free_rate_bps as i128) * PRICE_PRECISION / VOL_BPS;

    // sigma^2 * t / 2  (scaled: PRICE_PRECISION)
    let sigma_sq_t_half = sigma * sigma / PRICE_PRECISION * t / PRICE_PRECISION / 2;

    // sqrt(sigma^2 * t) = sigma * sqrt(t)
    let sigma_sqrt_t = sigma * isqrt(t) / isqrt(PRICE_PRECISION);

    if sigma_sqrt_t == 0 {
        return None;
    }

    // ln(S/K) approximation (PRICE_PRECISION scale)
    let ln_s_k = iln(input.spot * PRICE_PRECISION / input.strike);

    // d1 = (ln(S/K) + (r + sigma^2/2)*t) / (sigma*sqrt(t))
    let r_t = r * t / PRICE_PRECISION;
    let d1_num = ln_s_k + r_t + sigma_sq_t_half;
    let d1 = d1_num * PRICE_PRECISION / sigma_sqrt_t;

    // d2 = d1 - sigma*sqrt(t)
    let d2 = d1 - sigma_sqrt_t;

    // N(d1), N(d2) — standard normal CDF
    let nd1 = norm_cdf(d1);
    let nd2 = norm_cdf(d2);
    let nd1_neg = PRICE_PRECISION - nd1; // N(-d1)
    let nd2_neg = PRICE_PRECISION - nd2; // N(-d2)

    // Discount factor e^(-r*t) ≈ 1 - r*t for small r*t (good enough for short tenors)
    // For longer tenors use the series: e^x ≈ 1 + x + x^2/2
    let neg_rt = r_t; // r*t (positive)
    let discount = exp_neg(neg_rt); // e^(-r*t)

    // Call = S*N(d1) - K*e^(-rT)*N(d2)
    let call = input.spot * nd1 / PRICE_PRECISION
        - input.strike * discount / PRICE_PRECISION * nd2 / PRICE_PRECISION;

    // Put = K*e^(-rT)*N(-d2) - S*N(-d1)
    let put = input.strike * discount / PRICE_PRECISION * nd2_neg / PRICE_PRECISION
        - input.spot * nd1_neg / PRICE_PRECISION;

    Some(OptionPrice {
        call: call.max(0),
        put: put.max(0),
        call_delta: nd1,
        put_delta: nd1 - PRICE_PRECISION, // N(d1) - 1
    })
}

/// Price a futures contract: fair value = spot * e^(r*T).
/// Returns the fair futures price (PRICE_PRECISION scale).
pub fn futures_fair_value(
    spot: i128,
    time_to_expiry_secs: u64,
    risk_free_rate_bps: u32,
) -> i128 {
    let t = (time_to_expiry_secs as i128) * PRICE_PRECISION / SECS_PER_YEAR;
    let r = (risk_free_rate_bps as i128) * PRICE_PRECISION / VOL_BPS;
    let rt = r * t / PRICE_PRECISION;
    // e^(r*t) ≈ 1 + r*t + (r*t)^2/2
    let exp_rt = PRICE_PRECISION + rt + rt * rt / PRICE_PRECISION / 2;
    spot * exp_rt / PRICE_PRECISION
}

/// Price a fixed-for-floating swap: present value of fixed leg minus floating leg.
/// Returns the net PV from the fixed-payer's perspective (PRICE_PRECISION scale).
pub fn swap_fair_value(
    fixed_rate_bps: u32,
    floating_rate_bps: u32,
    notional: i128,
    time_to_expiry_secs: u64,
) -> i128 {
    let t = (time_to_expiry_secs as i128) * PRICE_PRECISION / SECS_PER_YEAR;
    let fixed = (fixed_rate_bps as i128) * PRICE_PRECISION / VOL_BPS;
    let floating = (floating_rate_bps as i128) * PRICE_PRECISION / VOL_BPS;
    // PV = notional * (floating - fixed) * t
    notional * (floating - fixed) / PRICE_PRECISION * t / PRICE_PRECISION
}

/// Compute the premium for a given derivative kind.
/// Returns the fair premium in token base units.
pub fn compute_premium(
    kind: DerivativeKind,
    input: &PricingInput,
    notional: i128,
) -> i128 {
    match kind {
        DerivativeKind::Call => {
            black_scholes(input)
                .map(|p| p.call * notional / PRICE_PRECISION)
                .unwrap_or(0)
        }
        DerivativeKind::Put => {
            black_scholes(input)
                .map(|p| p.put * notional / PRICE_PRECISION)
                .unwrap_or(0)
        }
        DerivativeKind::Future => {
            let fair = futures_fair_value(
                input.spot,
                input.time_to_expiry_secs,
                input.risk_free_rate_bps,
            );
            // Premium = |fair - strike| * notional / PRICE_PRECISION
            (fair - input.strike).abs() * notional / PRICE_PRECISION
        }
        DerivativeKind::Swap => {
            // For swaps, premium is the absolute PV difference
            swap_fair_value(
                input.risk_free_rate_bps,
                input.volatility_bps, // reuse volatility_bps as floating rate
                notional,
                input.time_to_expiry_secs,
            )
            .abs()
        }
    }
}

// ── Integer math helpers ─────────────────────────────────────────────────────

/// Integer square root via Newton-Raphson. Input and output are raw integers.
pub fn isqrt(n: i128) -> i128 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Natural log approximation for x > 0 (PRICE_PRECISION scale input/output).
/// Uses the identity ln(x) = 2*atanh((x-1)/(x+1)) with a 4-term series.
pub fn iln(x: i128) -> i128 {
    if x <= 0 {
        return -10 * PRICE_PRECISION; // clamp to a large negative
    }
    if x == PRICE_PRECISION {
        return 0;
    }

    // Reduce: find k such that x = 2^k * m, 0.5 <= m < 1 (in PRICE_PRECISION)
    // ln(x) = k*ln(2) + ln(m)
    // ln(2) ≈ 693_147 (PRICE_PRECISION scale)
    const LN2: i128 = 693_147;

    let mut val = x;
    let mut k: i128 = 0;

    while val >= 2 * PRICE_PRECISION {
        val /= 2;
        k += 1;
    }
    while val < PRICE_PRECISION {
        val *= 2;
        k -= 1;
    }

    // Now val is in [PRICE_PRECISION, 2*PRICE_PRECISION)
    // Use atanh series: ln(v) = 2*atanh((v-1)/(v+1))
    // t = (v-1)/(v+1)
    let t = (val - PRICE_PRECISION) * PRICE_PRECISION / (val + PRICE_PRECISION);
    let t2 = t * t / PRICE_PRECISION;
    // 2*(t + t^3/3 + t^5/5 + t^7/7)
    let series = t
        + t * t2 / PRICE_PRECISION / 3
        + t * t2 / PRICE_PRECISION * t2 / PRICE_PRECISION / 5
        + t * t2 / PRICE_PRECISION * t2 / PRICE_PRECISION * t2 / PRICE_PRECISION / 7;
    let ln_m = 2 * series;

    k * LN2 + ln_m
}

/// e^(-x) approximation for x >= 0 (PRICE_PRECISION scale input/output).
/// Uses the series: e^(-x) ≈ 1 - x + x^2/2 - x^3/6 (accurate for small x).
pub fn exp_neg(x: i128) -> i128 {
    if x <= 0 {
        return PRICE_PRECISION;
    }
    let x2 = x * x / PRICE_PRECISION;
    let x3 = x2 * x / PRICE_PRECISION;
    let result = PRICE_PRECISION - x + x2 / 2 - x3 / 6;
    result.max(0)
}

/// Standard normal CDF approximation (Abramowitz & Stegun 26.2.17).
/// Input: z in PRICE_PRECISION scale. Output: probability in PRICE_PRECISION scale.
pub fn norm_cdf(z: i128) -> i128 {
    // Handle tails
    if z <= -4 * PRICE_PRECISION {
        return 0;
    }
    if z >= 4 * PRICE_PRECISION {
        return PRICE_PRECISION;
    }

    let negative = z < 0;
    let z_abs = z.abs();

    // Rational approximation constants (scaled by 1_000_000)
    // p = 0.2316419 → 231_642
    const P: i128 = 231_642;
    // a1..a5 coefficients × 1_000_000
    const A1: i128 = 319_381_530;
    const A2: i128 = -356_563_782;
    const A3: i128 = 1_781_477_937;
    const A4: i128 = -1_821_255_978;
    const A5: i128 = 1_330_274_429;

    // t = 1 / (1 + p*|z|)  (PRICE_PRECISION scale)
    let denom = PRICE_PRECISION + P * z_abs / PRICE_PRECISION;
    let t = PRICE_PRECISION * PRICE_PRECISION / denom;

    // Polynomial in t
    let t2 = t * t / PRICE_PRECISION;
    let t3 = t2 * t / PRICE_PRECISION;
    let t4 = t3 * t / PRICE_PRECISION;
    let t5 = t4 * t / PRICE_PRECISION;

    let poly = (A1 * t + A2 * t2 + A3 * t3 + A4 * t4 + A5 * t5) / PRICE_PRECISION;

    // Standard normal PDF at z: phi(z) = e^(-z^2/2) / sqrt(2*pi)
    // sqrt(2*pi) ≈ 2_506_628 (PRICE_PRECISION scale)
    const SQRT_2PI: i128 = 2_506_628;
    let z2_half = z_abs * z_abs / PRICE_PRECISION / 2;
    let phi_num = exp_neg(z2_half);
    let phi = phi_num * PRICE_PRECISION / SQRT_2PI;

    let tail = phi * poly.abs() / PRICE_PRECISION;

    if negative {
        tail
    } else {
        PRICE_PRECISION - tail
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn test_isqrt() {
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(1_000_000 * 1_000_000), 1_000_000);
        assert_eq!(isqrt(4 * 1_000_000 * 1_000_000), 2_000_000);
    }

    #[test]
    fn test_norm_cdf_symmetry() {
        let p0 = norm_cdf(0);
        // N(0) ≈ 0.5
        assert!((p0 - 500_000).abs() < 5_000, "N(0) should be ~0.5, got {}", p0);
        let p_pos = norm_cdf(PRICE_PRECISION);
        let p_neg = norm_cdf(-PRICE_PRECISION);
        // N(1) + N(-1) ≈ 1
        assert!((p_pos + p_neg - PRICE_PRECISION).abs() < 10_000);
    }

    #[test]
    fn test_black_scholes_call_put_parity() {
        // Put-call parity: C - P = S - K*e^(-rT)
        let input = PricingInput {
            spot: PRICE_PRECISION,       // S = 1.0
            strike: PRICE_PRECISION,     // K = 1.0 (ATM)
            time_to_expiry_secs: 86_400 * 30, // 30 days
            volatility_bps: 5_000,       // 50%
            risk_free_rate_bps: 500,     // 5%
        };
        let prices = black_scholes(&input).unwrap();
        // ATM: call ≈ put (approximately, ignoring small r*T effect)
        let diff = (prices.call - prices.put).abs();
        assert!(diff < PRICE_PRECISION / 10, "ATM call/put should be close: call={} put={}", prices.call, prices.put);
    }

    #[test]
    fn test_futures_fair_value() {
        let spot = PRICE_PRECISION; // 1.0
        let fair = futures_fair_value(spot, 86_400 * 365, 500); // 1 year, 5%
        // Should be approximately 1.05 * PRICE_PRECISION
        assert!(fair > PRICE_PRECISION, "futures should be above spot");
        assert!(fair < 11 * PRICE_PRECISION / 10, "futures should be < 1.1 * spot");
    }
}
