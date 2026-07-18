//! Bonding Curve Mechanism for Dynamic Tip Token Pricing
//!
//! Implements three curve types for dynamic pricing of tip tokens:
//!
//! - **Linear**:      price = base_price + slope × supply
//! - **Exponential**: price = base_price × (1 + growth_rate)^supply  (approximated)
//! - **Sigmoid**:     price = max_price / (1 + e^(-k × (supply - midpoint)))  (approximated)
//!
//! All arithmetic uses integer math with a fixed-point PRECISION of 1e7 to
//! avoid floating-point while keeping acceptable accuracy.
//!
//! ## Reserve management
//! Each curve holds a **reserve** of the collateral token (e.g. XLM or a
//! stablecoin).  Buys add to the reserve; sells withdraw from it.  The
//! invariant `reserve ≥ integral(price, 0, supply)` is maintained so that
//! every holder can always sell back at the current curve price.

use soroban_sdk::{contracttype, panic_with_error, symbol_short, token, Address, Env};

use crate::{DataKey, TipJarError};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Fixed-point precision: 1e7.
pub const PRECISION: i128 = 10_000_000;

/// Maximum fee in basis points (10 % = 1 000 bps).
pub const MAX_FEE_BPS: u32 = 1_000;

/// Maximum number of bonding curves that can be created.
pub const MAX_CURVES: u64 = 1_000;

/// Minimum buy/sell amount (1 token unit).
pub const MIN_AMOUNT: i128 = 1;

// ── Curve type ────────────────────────────────────────────────────────────────

/// Selects the pricing formula used by a bonding curve.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CurveType {
    /// price = base_price + slope × supply
    Linear,
    /// price ≈ base_price × exp(growth_rate × supply / PRECISION)
    Exponential,
    /// price ≈ max_price × sigmoid(k × (supply − midpoint) / PRECISION)
    Sigmoid,
}

// ── Data types ────────────────────────────────────────────────────────────────

/// Configuration and live state for a single bonding curve.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BondingCurve {
    /// Unique curve identifier.
    pub id: u64,
    /// Creator / owner of this curve.
    pub creator: Address,
    /// Token minted/burned when users buy/sell (the "tip token").
    pub tip_token: Address,
    /// Collateral token deposited on buy / returned on sell.
    pub reserve_token: Address,
    /// Curve pricing formula.
    pub curve_type: CurveType,
    /// Base price in reserve-token units × PRECISION.
    pub base_price: i128,
    /// Slope (Linear) or growth-rate (Exponential) × PRECISION.
    pub slope: i128,
    /// Sigmoid steepness parameter k × PRECISION.
    pub k_param: i128,
    /// Sigmoid midpoint (supply at inflection) in token units.
    pub midpoint: i128,
    /// Maximum price cap for Sigmoid curves × PRECISION.
    pub max_price: i128,
    /// Current circulating supply of tip tokens.
    pub supply: i128,
    /// Collateral held in reserve (must cover all outstanding positions).
    pub reserve: i128,
    /// Buy fee in basis points (deducted from collateral paid).
    pub buy_fee_bps: u32,
    /// Sell fee in basis points (deducted from collateral returned).
    pub sell_fee_bps: u32,
    /// Accumulated fees collected (in reserve token).
    pub fees_collected: i128,
    /// Whether the curve is active.
    pub active: bool,
}

/// Parameters for creating a new bonding curve.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurveParams {
    pub curve_type: CurveType,
    /// Base price × PRECISION.
    pub base_price: i128,
    /// Slope / growth-rate × PRECISION (ignored for Sigmoid).
    pub slope: i128,
    /// Sigmoid k × PRECISION (ignored for Linear/Exponential).
    pub k_param: i128,
    /// Sigmoid midpoint in token units (ignored for Linear/Exponential).
    pub midpoint: i128,
    /// Sigmoid max price × PRECISION (ignored for Linear/Exponential).
    pub max_price: i128,
    /// Buy fee in basis points.
    pub buy_fee_bps: u32,
    /// Sell fee in basis points.
    pub sell_fee_bps: u32,
}

/// Result returned from a buy or sell operation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TradeResult {
    /// Tokens bought or sold.
    pub token_amount: i128,
    /// Collateral paid (buy) or received (sell).
    pub collateral_amount: i128,
    /// Fee charged in collateral units.
    pub fee_amount: i128,
    /// New spot price after the trade × PRECISION.
    pub new_price: i128,
    /// New total supply after the trade.
    pub new_supply: i128,
}

/// A price quote without executing a trade.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceQuote {
    /// Spot price at current supply × PRECISION.
    pub spot_price: i128,
    /// Collateral required to buy `amount` tokens (including fee).
    pub buy_cost: i128,
    /// Collateral returned for selling `amount` tokens (after fee).
    pub sell_return: i128,
    /// Fee on the buy side.
    pub buy_fee: i128,
    /// Fee on the sell side.
    pub sell_fee: i128,
}

// ── Pricing math ──────────────────────────────────────────────────────────────

/// Computes the spot price at a given supply for the curve.
/// Returns price × PRECISION.
pub fn spot_price(curve: &BondingCurve, supply: i128) -> i128 {
    match curve.curve_type {
        CurveType::Linear => {
            // price = base_price + slope × supply / PRECISION
            curve.base_price + curve.slope * supply / PRECISION
        }
        CurveType::Exponential => {
            // price ≈ base_price × e^(growth × supply / PRECISION)
            // Approximated with Taylor series: e^x ≈ 1 + x + x²/2 + x³/6
            // Accurate for small x; clamped for large x.
            let x = curve.slope * supply / PRECISION; // growth × supply (scaled)
            let exp_approx = exp_approx(x);
            curve.base_price * exp_approx / PRECISION
        }
        CurveType::Sigmoid => {
            // price ≈ max_price / (1 + e^(-k × (supply - midpoint) / PRECISION))
            let z = curve.k_param * (supply - curve.midpoint) / PRECISION;
            let sigmoid = sigmoid_approx(z);
            curve.max_price * sigmoid / PRECISION
        }
    }
}

/// Approximates e^x using a 5-term Taylor series, clamped to prevent overflow.
/// Input and output are both × PRECISION.
fn exp_approx(x: i128) -> i128 {
    // Clamp x to [-20 × PRECISION, 20 × PRECISION] to avoid overflow
    let x = x.max(-20 * PRECISION).min(20 * PRECISION);

    // e^x ≈ 1 + x + x²/2! + x³/3! + x⁴/4! + x⁵/5!
    // All terms scaled by PRECISION
    let term0 = PRECISION;
    let term1 = x;
    let term2 = x * x / (2 * PRECISION);
    let term3 = x * x / PRECISION * x / (6 * PRECISION);
    let term4 = x * x / PRECISION * x / PRECISION * x / (24 * PRECISION);
    let term5 = x * x / PRECISION * x / PRECISION * x / PRECISION * x / (120 * PRECISION);

    let result = term0 + term1 + term2 + term3 + term4 + term5;
    result.max(1) // e^x is always positive
}

/// Approximates sigmoid(x) = 1 / (1 + e^(-x)).
/// Input and output are both × PRECISION.
fn sigmoid_approx(x: i128) -> i128 {
    // sigmoid(x) = e^x / (1 + e^x)
    let exp_x = exp_approx(x);
    exp_x * PRECISION / (PRECISION + exp_x)
}

/// Integrates the price curve from `supply_start` to `supply_end` using
/// the trapezoidal rule with `steps` intervals.
/// Returns the total collateral cost × PRECISION (divide by PRECISION for actual cost).
fn integrate_price(curve: &BondingCurve, supply_start: i128, supply_end: i128, steps: u32) -> i128 {
    if supply_start >= supply_end || steps == 0 {
        return 0;
    }

    let n = steps as i128;
    let delta = (supply_end - supply_start) / n;
    if delta == 0 {
        // Very small range — use single trapezoid
        let p0 = spot_price(curve, supply_start);
        let p1 = spot_price(curve, supply_end);
        return (p0 + p1) * (supply_end - supply_start) / 2;
    }

    let mut total: i128 = 0;
    let mut s = supply_start;

    // Trapezoidal rule: sum of (f(a) + f(b)) / 2 × Δx
    let p_first = spot_price(curve, s);
    total += p_first * delta / 2;

    for _ in 1..steps {
        s += delta;
        total += spot_price(curve, s) * delta;
    }

    let p_last = spot_price(curve, supply_end);
    total += p_last * delta / 2;

    total
}

/// Number of integration steps — higher = more accurate but more gas.
const INTEGRATION_STEPS: u32 = 20;

/// Computes the collateral cost to buy `amount` tokens starting from `current_supply`.
/// Returns raw collateral (not scaled by PRECISION).
fn cost_to_buy(curve: &BondingCurve, amount: i128) -> i128 {
    integrate_price(
        curve,
        curve.supply,
        curve.supply + amount,
        INTEGRATION_STEPS,
    ) / PRECISION
}

/// Computes the collateral returned for selling `amount` tokens.
/// Returns raw collateral (not scaled by PRECISION).
fn return_on_sell(curve: &BondingCurve, amount: i128) -> i128 {
    integrate_price(
        curve,
        curve.supply - amount,
        curve.supply,
        INTEGRATION_STEPS,
    ) / PRECISION
}

// ── Storage helpers ───────────────────────────────────────────────────────────

fn get_curve(env: &Env, curve_id: u64) -> Option<BondingCurve> {
    env.storage()
        .persistent()
        .get(&DataKey::BondingCurve(curve_id))
}

fn get_curve_or_panic(env: &Env, curve_id: u64) -> BondingCurve {
    get_curve(env, curve_id).unwrap_or_else(|| panic_with_error!(env, TipJarError::BcNotFound))
}

fn set_curve(env: &Env, curve: &BondingCurve) {
    env.storage()
        .persistent()
        .set(&DataKey::BondingCurve(curve.id), curve);
}

fn next_curve_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::BondingCurveCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .persistent()
        .set(&DataKey::BondingCurveCounter, &next);
    next
}

// ── Validation ────────────────────────────────────────────────────────────────

fn validate_params(env: &Env, params: &CurveParams) {
    if params.base_price <= 0 {
        panic_with_error!(env, TipJarError::BcInvalidParams);
    }
    if params.buy_fee_bps > MAX_FEE_BPS || params.sell_fee_bps > MAX_FEE_BPS {
        panic_with_error!(env, TipJarError::BcFeeTooHigh);
    }
    match params.curve_type {
        CurveType::Linear => {
            if params.slope < 0 {
                panic_with_error!(env, TipJarError::BcInvalidParams);
            }
        }
        CurveType::Exponential => {
            if params.slope <= 0 {
                panic_with_error!(env, TipJarError::BcInvalidParams);
            }
        }
        CurveType::Sigmoid => {
            if params.k_param <= 0 || params.max_price <= 0 || params.midpoint < 0 {
                panic_with_error!(env, TipJarError::BcInvalidParams);
            }
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Creates a new bonding curve. Returns the curve ID.
///
/// The creator must supply an initial reserve to seed the curve.
/// Pass `initial_reserve = 0` to start with an empty reserve.
pub fn create_curve(
    env: &Env,
    creator: &Address,
    tip_token: &Address,
    reserve_token: &Address,
    params: CurveParams,
    initial_reserve: i128,
) -> u64 {
    creator.require_auth();
    validate_params(env, &params);

    if initial_reserve < 0 {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    // Transfer initial reserve from creator if provided
    if initial_reserve > 0 {
        token::Client::new(env, reserve_token).transfer(
            creator,
            &env.current_contract_address(),
            &initial_reserve,
        );
    }

    let curve_id = next_curve_id(env);

    let curve = BondingCurve {
        id: curve_id,
        creator: creator.clone(),
        tip_token: tip_token.clone(),
        reserve_token: reserve_token.clone(),
        curve_type: params.curve_type,
        base_price: params.base_price,
        slope: params.slope,
        k_param: params.k_param,
        midpoint: params.midpoint,
        max_price: params.max_price,
        supply: 0,
        reserve: initial_reserve,
        buy_fee_bps: params.buy_fee_bps,
        sell_fee_bps: params.sell_fee_bps,
        fees_collected: 0,
        active: true,
    };

    set_curve(env, &curve);

    env.events().publish(
        (symbol_short!("bc_create"),),
        (
            curve_id,
            creator.clone(),
            tip_token.clone(),
            reserve_token.clone(),
        ),
    );

    curve_id
}

/// Buys `token_amount` tip tokens from the curve.
///
/// The buyer pays collateral calculated by integrating the price curve from
/// `supply` to `supply + token_amount`, plus the buy fee.
/// Returns a `TradeResult` with the actual amounts.
pub fn buy(
    env: &Env,
    buyer: &Address,
    curve_id: u64,
    token_amount: i128,
    max_collateral: i128,
) -> TradeResult {
    buyer.require_auth();

    if token_amount < MIN_AMOUNT {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    let mut curve = get_curve_or_panic(env, curve_id);

    if !curve.active {
        panic_with_error!(env, TipJarError::BcInactive);
    }

    // Calculate base cost via integration
    let base_cost = cost_to_buy(&curve, token_amount);
    if base_cost <= 0 {
        panic_with_error!(env, TipJarError::BcPriceCalculationFailed);
    }

    // Apply buy fee
    let fee = base_cost * curve.buy_fee_bps as i128 / 10_000;
    let total_cost = base_cost + fee;

    if total_cost > max_collateral {
        panic_with_error!(env, TipJarError::BcSlippageExceeded);
    }

    // Transfer collateral from buyer to contract
    token::Client::new(env, &curve.reserve_token).transfer(
        buyer,
        &env.current_contract_address(),
        &total_cost,
    );

    // Mint tip tokens to buyer
    token::Client::new(env, &curve.tip_token).transfer(
        &env.current_contract_address(),
        buyer,
        &token_amount,
    );

    // Update curve state
    curve.supply += token_amount;
    curve.reserve += base_cost; // fee stays in fees_collected, not reserve
    curve.fees_collected += fee;

    let new_price = spot_price(&curve, curve.supply);
    set_curve(env, &curve);

    env.events().publish(
        (symbol_short!("bc_buy"),),
        (buyer.clone(), curve_id, token_amount, total_cost, new_price),
    );

    TradeResult {
        token_amount,
        collateral_amount: total_cost,
        fee_amount: fee,
        new_price,
        new_supply: curve.supply,
    }
}

/// Sells `token_amount` tip tokens back to the curve.
///
/// The seller receives collateral calculated by integrating the price curve
/// from `supply - token_amount` to `supply`, minus the sell fee.
/// Returns a `TradeResult` with the actual amounts.
pub fn sell(
    env: &Env,
    seller: &Address,
    curve_id: u64,
    token_amount: i128,
    min_collateral: i128,
) -> TradeResult {
    seller.require_auth();

    if token_amount < MIN_AMOUNT {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    let mut curve = get_curve_or_panic(env, curve_id);

    if !curve.active {
        panic_with_error!(env, TipJarError::BcInactive);
    }

    if token_amount > curve.supply {
        panic_with_error!(env, TipJarError::BcInsufficientSupply);
    }

    // Calculate return via integration
    let base_return = return_on_sell(&curve, token_amount);
    if base_return <= 0 {
        panic_with_error!(env, TipJarError::BcPriceCalculationFailed);
    }

    // Apply sell fee
    let fee = base_return * curve.sell_fee_bps as i128 / 10_000;
    let net_return = base_return - fee;

    if net_return < min_collateral {
        panic_with_error!(env, TipJarError::BcSlippageExceeded);
    }

    // Guard: reserve must cover the return
    if base_return > curve.reserve {
        panic_with_error!(env, TipJarError::BcInsufficientReserve);
    }

    // Transfer tip tokens from seller to contract (burn)
    token::Client::new(env, &curve.tip_token).transfer(
        seller,
        &env.current_contract_address(),
        &token_amount,
    );

    // Transfer collateral back to seller
    token::Client::new(env, &curve.reserve_token).transfer(
        &env.current_contract_address(),
        seller,
        &net_return,
    );

    // Update curve state
    curve.supply -= token_amount;
    curve.reserve -= base_return;
    curve.fees_collected += fee;

    let new_price = spot_price(&curve, curve.supply);
    set_curve(env, &curve);

    env.events().publish(
        (symbol_short!("bc_sell"),),
        (
            seller.clone(),
            curve_id,
            token_amount,
            net_return,
            new_price,
        ),
    );

    TradeResult {
        token_amount,
        collateral_amount: net_return,
        fee_amount: fee,
        new_price,
        new_supply: curve.supply,
    }
}

/// Updates the fee parameters of a curve. Only the creator can call this.
pub fn update_fees(
    env: &Env,
    creator: &Address,
    curve_id: u64,
    buy_fee_bps: u32,
    sell_fee_bps: u32,
) {
    creator.require_auth();

    if buy_fee_bps > MAX_FEE_BPS || sell_fee_bps > MAX_FEE_BPS {
        panic_with_error!(env, TipJarError::BcFeeTooHigh);
    }

    let mut curve = get_curve_or_panic(env, curve_id);

    if curve.creator != *creator {
        panic_with_error!(env, TipJarError::Unauthorized);
    }

    curve.buy_fee_bps = buy_fee_bps;
    curve.sell_fee_bps = sell_fee_bps;
    set_curve(env, &curve);

    env.events().publish(
        (symbol_short!("bc_fee"),),
        (curve_id, buy_fee_bps, sell_fee_bps),
    );
}

/// Withdraws accumulated fees to the curve creator.
pub fn withdraw_fees(env: &Env, creator: &Address, curve_id: u64) -> i128 {
    creator.require_auth();

    let mut curve = get_curve_or_panic(env, curve_id);

    if curve.creator != *creator {
        panic_with_error!(env, TipJarError::Unauthorized);
    }

    let fees = curve.fees_collected;
    if fees <= 0 {
        panic_with_error!(env, TipJarError::BcNoFeesToWithdraw);
    }

    token::Client::new(env, &curve.reserve_token).transfer(
        &env.current_contract_address(),
        creator,
        &fees,
    );

    curve.fees_collected = 0;
    set_curve(env, &curve);

    env.events().publish(
        (symbol_short!("bc_wfee"),),
        (curve_id, creator.clone(), fees),
    );

    fees
}

/// Deactivates a curve. Only the creator can call this.
/// Deactivated curves cannot accept new buys/sells.
pub fn deactivate_curve(env: &Env, creator: &Address, curve_id: u64) {
    creator.require_auth();

    let mut curve = get_curve_or_panic(env, curve_id);

    if curve.creator != *creator {
        panic_with_error!(env, TipJarError::Unauthorized);
    }

    if !curve.active {
        panic_with_error!(env, TipJarError::BcInactive);
    }

    curve.active = false;
    set_curve(env, &curve);

    env.events()
        .publish((symbol_short!("bc_deact"),), (curve_id,));
}

/// Returns a price quote for buying or selling `amount` tokens without
/// executing any trade.
pub fn get_quote(env: &Env, curve_id: u64, amount: i128) -> PriceQuote {
    if amount <= 0 {
        panic_with_error!(env, TipJarError::InvalidAmount);
    }

    let curve = get_curve_or_panic(env, curve_id);
    let current_spot = spot_price(&curve, curve.supply);

    let buy_base = cost_to_buy(&curve, amount);
    let buy_fee = buy_base * curve.buy_fee_bps as i128 / 10_000;
    let buy_cost = buy_base + buy_fee;

    let sell_base = if amount <= curve.supply {
        return_on_sell(&curve, amount)
    } else {
        0
    };
    let sell_fee = sell_base * curve.sell_fee_bps as i128 / 10_000;
    let sell_return = (sell_base - sell_fee).max(0);

    PriceQuote {
        spot_price: current_spot,
        buy_cost,
        sell_return,
        buy_fee,
        sell_fee,
    }
}

/// Returns the current spot price for a curve.
pub fn get_spot_price(env: &Env, curve_id: u64) -> i128 {
    let curve = get_curve_or_panic(env, curve_id);
    spot_price(&curve, curve.supply)
}

/// Returns a bonding curve by ID.
pub fn get_curve_info(env: &Env, curve_id: u64) -> BondingCurve {
    get_curve_or_panic(env, curve_id)
}

// ── Unit tests for pure math ──────────────────────────────────────────────────

#[cfg(test)]
mod math_tests {
    use super::*;

    fn linear_curve() -> BondingCurve {
        use soroban_sdk::testutils::Address as _;
        let env = soroban_sdk::Env::default();
        let addr = soroban_sdk::Address::generate(&env);
        BondingCurve {
            id: 1,
            creator: addr.clone(),
            tip_token: addr.clone(),
            reserve_token: addr.clone(),
            curve_type: CurveType::Linear,
            base_price: PRECISION, // 1.0
            slope: PRECISION / 10, // 0.1 per token
            k_param: 0,
            midpoint: 0,
            max_price: 0,
            supply: 0,
            reserve: 0,
            buy_fee_bps: 0,
            sell_fee_bps: 0,
            fees_collected: 0,
            active: true,
        }
    }

    #[test]
    fn linear_spot_price_at_zero_supply() {
        let curve = linear_curve();
        let price = spot_price(&curve, 0);
        assert_eq!(price, PRECISION); // base_price = 1.0
    }

    #[test]
    fn linear_spot_price_increases_with_supply() {
        let curve = linear_curve();
        let p0 = spot_price(&curve, 0);
        let p10 = spot_price(&curve, 10 * PRECISION);
        assert!(p10 > p0);
    }

    #[test]
    fn exp_approx_at_zero_is_one() {
        let result = exp_approx(0);
        assert_eq!(result, PRECISION);
    }

    #[test]
    fn exp_approx_positive_x_greater_than_one() {
        let result = exp_approx(PRECISION); // e^1 ≈ 2.718
        assert!(result > PRECISION);
        assert!(result < 3 * PRECISION);
    }

    #[test]
    fn sigmoid_at_zero_is_half() {
        let result = sigmoid_approx(0);
        // sigmoid(0) = 0.5 → result ≈ PRECISION / 2
        let half = PRECISION / 2;
        let tolerance = PRECISION / 20; // 5% tolerance
        assert!((result - half).abs() < tolerance);
    }

    #[test]
    fn integration_cost_positive_for_positive_amount() {
        let curve = linear_curve();
        let cost = cost_to_buy(&curve, 10 * PRECISION);
        assert!(cost > 0);
    }

    #[test]
    fn sell_return_less_than_buy_cost_due_to_curve_shape() {
        // For a linear curve, buying then selling at the same supply
        // should return approximately the same amount (no fee case).
        let mut curve = linear_curve();
        let amount = 5 * PRECISION;
        let buy_cost = cost_to_buy(&curve, amount);
        curve.supply = amount; // simulate after buy
        let sell_ret = return_on_sell(&curve, amount);
        // Should be approximately equal (within integration error)
        let diff = (buy_cost - sell_ret).abs();
        let tolerance = buy_cost / 20; // 5% tolerance for numerical integration
        assert!(
            diff < tolerance,
            "buy={buy_cost} sell={sell_ret} diff={diff}"
        );
    }
}
