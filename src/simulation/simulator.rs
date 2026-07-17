/// Transaction simulator for previewing contract calls
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    /// Whether the transaction would succeed
    pub success: bool,
    /// Gas cost in stroops
    pub gas_cost: u64,
    /// State changes that would occur
    pub state_changes: Vec<StateChange>,
    /// Events that would be emitted
    pub events: Vec<ContractEvent>,
    /// Error message if simulation failed
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    /// Storage key that would be modified
    pub key: String,
    /// Old value (if exists)
    pub old_value: Option<String>,
    /// New value
    pub new_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractEvent {
    /// Event topic
    pub topic: String,
    /// Event data
    pub data: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TipSimulation {
    pub creator: String,
    pub amount: i128,
    pub tipper: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalSimulation {
    pub creator: String,
    pub amount: i128,
}

pub struct TransactionSimulator;

impl TransactionSimulator {
    /// Simulate a tip transaction
    pub fn simulate_tip(
        creator: &str,
        amount: i128,
        tipper: &str,
    ) -> Result<SimulationResult, String> {
        if amount <= 0 {
            return Err("Invalid amount: must be positive".to_string());
        }

        let gas_cost = Self::calculate_gas_cost(100); // Base gas for tip

        let state_changes = vec![
            StateChange {
                key: format!("creator_balance:{}", creator),
                old_value: None,
                new_value: amount.to_string(),
            },
            StateChange {
                key: format!("creator_total:{}", creator),
                old_value: None,
                new_value: amount.to_string(),
            },
        ];

        let events = vec![ContractEvent {
            topic: "tip".to_string(),
            data: vec![tipper.to_string(), amount.to_string()],
        }];

        Ok(SimulationResult {
            success: true,
            gas_cost,
            state_changes,
            events,
            error: None,
        })
    }

    /// Simulate a withdrawal transaction
    pub fn simulate_withdrawal(
        creator: &str,
        amount: i128,
        balance: i128,
    ) -> Result<SimulationResult, String> {
        if amount <= 0 {
            return Err("Invalid amount: must be positive".to_string());
        }

        if amount > balance {
            return Err("Insufficient balance".to_string());
        }

        let gas_cost = Self::calculate_gas_cost(150); // Higher gas for withdrawal

        let state_changes = vec![StateChange {
            key: format!("creator_balance:{}", creator),
            old_value: Some(balance.to_string()),
            new_value: (balance - amount).to_string(),
        }];

        let events = vec![ContractEvent {
            topic: "withdraw".to_string(),
            data: vec![amount.to_string()],
        }];

        Ok(SimulationResult {
            success: true,
            gas_cost,
            state_changes,
            events,
            error: None,
        })
    }

    /// Calculate total cost including network fees
    pub fn calculate_total_cost(gas_units: u64) -> i128 {
        let base_fee: i128 = 100;
        let resource_fee: i128 = (gas_units as i128) * 10;
        base_fee + resource_fee
    }

    /// Calculate gas cost for an operation
    fn calculate_gas_cost(base_gas: u64) -> u64 {
        base_gas + 50 // Add overhead
    }

    /// Simulate batch tip operation
    pub fn simulate_batch_tips(
        tips: Vec<(String, i128)>,
    ) -> Result<SimulationResult, String> {
        if tips.is_empty() {
            return Err("Empty batch".to_string());
        }

        if tips.len() > 100 {
            return Err("Batch too large".to_string());
        }

        let total_amount: i128 = tips.iter().map(|(_, amount)| amount).sum();
        let gas_cost = Self::calculate_gas_cost((tips.len() as u64) * 100);

        let state_changes: Vec<StateChange> = tips
            .iter()
            .map(|(creator, amount)| StateChange {
                key: format!("creator_balance:{}", creator),
                old_value: None,
                new_value: amount.to_string(),
            })
            .collect();

        let events: Vec<ContractEvent> = tips
            .iter()
            .map(|(creator, amount)| ContractEvent {
                topic: "batch_tip".to_string(),
                data: vec![creator.clone(), amount.to_string()],
            })
            .collect();

        Ok(SimulationResult {
            success: true,
            gas_cost,
            state_changes,
            events,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulate_tip() {
        let result = TransactionSimulator::simulate_tip("creator1", 1000, "tipper1");
        assert!(result.is_ok());
        let sim = result.unwrap();
        assert!(sim.success);
        assert!(sim.gas_cost > 0);
        assert_eq!(sim.state_changes.len(), 2);
        assert_eq!(sim.events.len(), 1);
    }

    #[test]
    fn test_simulate_tip_invalid_amount() {
        let result = TransactionSimulator::simulate_tip("creator1", 0, "tipper1");
        assert!(result.is_err());
    }

    #[test]
    fn test_simulate_withdrawal() {
        let result = TransactionSimulator::simulate_withdrawal("creator1", 500, 1000);
        assert!(result.is_ok());
        let sim = result.unwrap();
        assert!(sim.success);
    }

    #[test]
    fn test_simulate_withdrawal_insufficient_balance() {
        let result = TransactionSimulator::simulate_withdrawal("creator1", 1500, 1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_total_cost() {
        let cost = TransactionSimulator::calculate_total_cost(100);
        assert_eq!(cost, 100 + (100 * 10));
    }

    #[test]
    fn test_simulate_batch_tips() {
        let tips = vec![
            ("creator1".to_string(), 100),
            ("creator2".to_string(), 200),
        ];
        let result = TransactionSimulator::simulate_batch_tips(tips);
        assert!(result.is_ok());
        let sim = result.unwrap();
        assert!(sim.success);
        assert_eq!(sim.state_changes.len(), 2);
    }

    #[test]
    fn test_simulate_batch_too_large() {
        let tips: Vec<_> = (0..101)
            .map(|i| (format!("creator{}", i), 100))
            .collect();
        let result = TransactionSimulator::simulate_batch_tips(tips);
        assert!(result.is_err());
    }
}
