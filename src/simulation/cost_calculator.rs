/// Gas cost calculator for transactions
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasCostEstimate {
    /// Base fee in stroops
    pub base_fee: u64,
    /// Resource fee in stroops
    pub resource_fee: u64,
    /// Total fee in stroops
    pub total_fee: u64,
    /// Estimated XLM cost
    pub xlm_cost: f64,
}

pub struct CostCalculator;

impl CostCalculator {
    /// Base fee per operation
    const BASE_FEE: u64 = 100;

    /// Resource fee per gas unit
    const RESOURCE_FEE_PER_GAS: u64 = 10;

    /// Stroops per XLM
    const STROOPS_PER_XLM: f64 = 10_000_000.0;

    /// Calculate gas cost for a tip operation
    pub fn calculate_tip_cost() -> GasCostEstimate {
        let base_fee = Self::BASE_FEE;
        let resource_fee = 100 * Self::RESOURCE_FEE_PER_GAS;
        let total_fee = base_fee + resource_fee;

        Self::create_estimate(base_fee, resource_fee, total_fee)
    }

    /// Calculate gas cost for a withdrawal operation
    pub fn calculate_withdrawal_cost() -> GasCostEstimate {
        let base_fee = Self::BASE_FEE;
        let resource_fee = 150 * Self::RESOURCE_FEE_PER_GAS;
        let total_fee = base_fee + resource_fee;

        Self::create_estimate(base_fee, resource_fee, total_fee)
    }

    /// Calculate gas cost for batch operation
    pub fn calculate_batch_cost(batch_size: u32) -> GasCostEstimate {
        let base_fee = Self::BASE_FEE;
        let resource_fee = (batch_size as u64 * 100) * Self::RESOURCE_FEE_PER_GAS;
        let total_fee = base_fee + resource_fee;

        Self::create_estimate(base_fee, resource_fee, total_fee)
    }

    /// Create cost estimate
    fn create_estimate(base_fee: u64, resource_fee: u64, total_fee: u64) -> GasCostEstimate {
        let xlm_cost = (total_fee as f64) / Self::STROOPS_PER_XLM;

        GasCostEstimate {
            base_fee,
            resource_fee,
            total_fee,
            xlm_cost,
        }
    }

    /// Estimate cost for custom gas units
    pub fn estimate_for_gas(gas_units: u64) -> GasCostEstimate {
        let base_fee = Self::BASE_FEE;
        let resource_fee = gas_units * Self::RESOURCE_FEE_PER_GAS;
        let total_fee = base_fee + resource_fee;

        Self::create_estimate(base_fee, resource_fee, total_fee)
    }

    /// Get cost breakdown as string
    pub fn format_cost(estimate: &GasCostEstimate) -> String {
        format!(
            "Base: {} stroops, Resource: {} stroops, Total: {} stroops ({:.6} XLM)",
            estimate.base_fee, estimate.resource_fee, estimate.total_fee, estimate.xlm_cost
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_tip_cost() {
        let cost = CostCalculator::calculate_tip_cost();
        assert_eq!(cost.base_fee, 100);
        assert!(cost.total_fee > 0);
    }

    #[test]
    fn test_calculate_withdrawal_cost() {
        let cost = CostCalculator::calculate_withdrawal_cost();
        assert!(cost.total_fee > CostCalculator::calculate_tip_cost().total_fee);
    }

    #[test]
    fn test_calculate_batch_cost() {
        let cost = CostCalculator::calculate_batch_cost(10);
        assert!(cost.total_fee > 0);
    }

    #[test]
    fn test_xlm_conversion() {
        let cost = CostCalculator::calculate_tip_cost();
        assert!(cost.xlm_cost > 0.0);
        assert!(cost.xlm_cost < 0.001); // Should be very small
    }

    #[test]
    fn test_format_cost() {
        let cost = CostCalculator::calculate_tip_cost();
        let formatted = CostCalculator::format_cost(&cost);
        assert!(formatted.contains("stroops"));
        assert!(formatted.contains("XLM"));
    }
}
