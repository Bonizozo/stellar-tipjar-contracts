use super::calculator::{BASE_FEE_BPS, MAX_FEE_BPS, MIN_FEE_BPS};
use crate::DataKey;
use soroban_sdk::Env;

/// Network congestion level supplied by the caller or an oracle.
///
/// - `Low`    → fee discount applied  (×0.5)
/// - `Normal` → base fee unchanged    (×1.0)
/// - `High`   → fee surcharge applied (×1.5)
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CongestionLevel {
    Low,
    Normal,
    High,
}

/// Returns the dynamically adjusted fee in basis points, clamped to
/// `[MIN_FEE_BPS, MAX_FEE_BPS]`.
///
/// The result is also persisted in contract storage so it can be queried
/// transparently via `get_current_fee_bps`.
pub fn adjusted_fee_bps(env: &Env, congestion: CongestionLevel) -> u32 {
    let raw = match congestion {
        CongestionLevel::Low => BASE_FEE_BPS / 2,
        CongestionLevel::Normal => BASE_FEE_BPS,
        CongestionLevel::High => BASE_FEE_BPS * 3 / 2,
    };
    let clamped = raw.clamp(MIN_FEE_BPS, MAX_FEE_BPS);
    env.storage()
        .instance()
        .set(&DataKey::CurrentFeeBps, &clamped);
    clamped
}
