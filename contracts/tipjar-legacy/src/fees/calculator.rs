use super::adjustment::CongestionLevel;
use soroban_sdk::Env;

/// Base fee in basis points (1% = 100 bps).
pub const BASE_FEE_BPS: u32 = 100;
/// Minimum fee in basis points (0.1%).
pub const MIN_FEE_BPS: u32 = 10;
/// Maximum fee in basis points (5%).
pub const MAX_FEE_BPS: u32 = 500;

/// Computes the fee amount for a given tip `amount` and `congestion_level`.
///
/// Returns `(fee_amount, fee_bps)` where `fee_bps` is the effective rate used.
pub fn compute_fee(env: &Env, amount: i128, congestion: CongestionLevel) -> (i128, u32) {
    let fee_bps = super::adjustment::adjusted_fee_bps(env, congestion);
    let fee = amount * fee_bps as i128 / 10_000;
    (fee, fee_bps)
}
