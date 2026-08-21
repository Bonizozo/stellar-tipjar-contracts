//! Interest rate calculations for lending pools.

const PRECISION: i128 = 1_000_000_000_000_000_000; // 1e18

/// Calculate interest rate based on pool utilization.
/// Returns rate as basis points (e.g., 500 = 5%).
/// Rate = 5% + (utilization * 45%) = 5% to 50%
pub fn calculate_rate(total_borrowed: i128, total_liquidity: i128) -> u32 {
    if total_borrowed == 0 && total_liquidity == 0 {
        return 5000; // 5% base rate for an empty pool
    }

    let utilization = (total_borrowed * PRECISION) / (total_borrowed + total_liquidity);
    let variable_rate = (utilization * 45000) / PRECISION;
    let base_rate = 5000u32;

    base_rate + (variable_rate as u32)
}

/// Calculate accrued interest over time period.
/// principal: loan amount
/// rate: interest rate in basis points
/// seconds: time elapsed
pub fn calculate_interest(principal: i128, rate: u32, seconds: u64) -> i128 {
    // Annual rate: rate / 10000 (basis points to decimal)
    // Per-second rate: annual_rate / 31536000 (seconds per year)
    // Interest = principal * rate / 10000 * seconds / 31536000
    let interest = (principal * (rate as i128) * (seconds as i128)) / (10000 * 31536000);

    interest.max(0)
}

/// Calculate liquidation price: collateral / (loan * 1.10)
/// Returns true if collateral falls below 110% of loan.
pub fn is_liquidatable(loan_amount: i128, collateral: i128) -> bool {
    // 110% threshold = loan * 1.10
    let threshold = (loan_amount * 110) / 100;
    collateral < threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_rate_no_borrowing() {
        let rate = calculate_rate(0, 100_000);
        assert_eq!(rate, 5000); // 5% base
    }

    #[test]
    fn test_calculate_rate_full_utilization() {
        let rate = calculate_rate(100_000, 0);
        assert_eq!(rate, 50000); // 50% max
    }

    #[test]
    fn test_calculate_rate_half_utilization() {
        let rate = calculate_rate(100_000, 100_000);
        assert!(rate > 5000 && rate < 50000);
    }

    #[test]
    fn test_calculate_interest() {
        // 1000 tokens at 10% annual (1000 bps) for 1 year
        let interest = calculate_interest(1000 * PRECISION, 1000, 31536000);
        assert_eq!(interest, 100 * PRECISION);
    }

    #[test]
    fn test_is_liquidatable_safe() {
        // 100 loan, 150 collateral = 150% (safe)
        assert!(!is_liquidatable(100, 150));
    }

    #[test]
    fn test_is_liquidatable_unsafe() {
        // 100 loan, 100 collateral = 100% (liquidatable)
        assert!(is_liquidatable(100, 100));
    }

    #[test]
    fn test_is_liquidatable_threshold() {
        // 100 loan, 110 collateral = exactly at threshold (not liquidatable)
        assert!(!is_liquidatable(100, 110));
    }
}
