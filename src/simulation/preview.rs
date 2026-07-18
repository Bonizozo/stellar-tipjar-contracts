/// Preview generator for transaction outcomes
use serde::{Deserialize, Serialize};

use super::simulator::{ContractEvent, SimulationResult, StateChange};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionPreview {
    /// Human-readable description
    pub description: String,
    /// Expected outcome
    pub outcome: String,
    /// Estimated cost
    pub estimated_cost: i128,
    /// State changes summary
    pub changes_summary: String,
    /// Events that will be emitted
    pub events: Vec<String>,
    /// Warnings or notes
    pub warnings: Vec<String>,
}

pub struct PreviewGenerator;

impl PreviewGenerator {
    /// Generate preview from simulation result
    pub fn generate_preview(
        operation: &str,
        simulation: &SimulationResult,
    ) -> TransactionPreview {
        let description = Self::describe_operation(operation);
        let outcome = if simulation.success {
            "Transaction will succeed".to_string()
        } else {
            format!("Transaction will fail: {}", simulation.error.as_ref().unwrap_or(&"Unknown error".to_string()))
        };

        let estimated_cost = Self::calculate_total_cost(simulation.gas_cost);
        let changes_summary = Self::summarize_changes(&simulation.state_changes);
        let events = simulation
            .events
            .iter()
            .map(|e| format!("Event: {} with data {:?}", e.topic, e.data))
            .collect();

        let warnings = Self::generate_warnings(operation, simulation);

        TransactionPreview {
            description,
            outcome,
            estimated_cost,
            changes_summary,
            events,
            warnings,
        }
    }

    /// Describe the operation in human-readable format
    fn describe_operation(operation: &str) -> String {
        match operation {
            "tip" => "Send a tip to a creator".to_string(),
            "withdraw" => "Withdraw escrowed tips".to_string(),
            "batch_tip" => "Send multiple tips in one transaction".to_string(),
            _ => format!("Execute operation: {}", operation),
        }
    }

    /// Summarize state changes
    fn summarize_changes(changes: &[StateChange]) -> String {
        if changes.is_empty() {
            return "No state changes".to_string();
        }

        let mut summary = String::new();
        for change in changes {
            summary.push_str(&format!(
                "- {} will change to {}\n",
                change.key, change.new_value
            ));
        }
        summary
    }

    /// Generate warnings based on simulation
    fn generate_warnings(operation: &str, simulation: &SimulationResult) -> Vec<String> {
        let mut warnings = Vec::new();

        if simulation.gas_cost > 10000 {
            warnings.push("High gas cost detected".to_string());
        }

        if operation == "batch_tip" && simulation.state_changes.len() > 50 {
            warnings.push("Large batch operation may take longer to process".to_string());
        }

        warnings
    }

    /// Calculate total cost including network fees
    fn calculate_total_cost(gas_units: u64) -> i128 {
        let base_fee: i128 = 100;
        let resource_fee: i128 = (gas_units as i128) * 10;
        base_fee + resource_fee
    }

    /// Generate detailed preview for tip operation
    pub fn preview_tip(creator: &str, amount: i128, tipper: &str) -> TransactionPreview {
        TransactionPreview {
            description: format!("Send {} XLM tip to {}", amount, creator),
            outcome: "Tip will be recorded and escrowed".to_string(),
            estimated_cost: 1100, // Base cost
            changes_summary: format!(
                "- Creator {} balance increases by {}\n- Creator {} total tips increases by {}",
                creator, amount, creator, amount
            ),
            events: vec![format!("Tip event: {} sends {} to {}", tipper, amount, creator)],
            warnings: vec![],
        }
    }

    /// Generate detailed preview for withdrawal operation
    pub fn preview_withdrawal(creator: &str, amount: i128) -> TransactionPreview {
        TransactionPreview {
            description: format!("Withdraw {} XLM from escrowed balance", amount),
            outcome: "Funds will be transferred to creator account".to_string(),
            estimated_cost: 1500, // Higher cost for withdrawal
            changes_summary: format!(
                "- Creator {} balance decreases by {}\n- Funds transferred to creator account",
                creator, amount
            ),
            events: vec![format!("Withdrawal event: {} withdraws {}", creator, amount)],
            warnings: vec!["Ensure sufficient balance before withdrawal".to_string()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_preview() {
        let simulation = SimulationResult {
            success: true,
            gas_cost: 100,
            state_changes: vec![],
            events: vec![],
            error: None,
        };

        let preview = PreviewGenerator::generate_preview("tip", &simulation);
        assert!(preview.outcome.contains("succeed"));
    }

    #[test]
    fn test_preview_tip() {
        let preview = PreviewGenerator::preview_tip("creator1", 1000, "tipper1");
        assert!(preview.description.contains("1000"));
        assert!(preview.description.contains("creator1"));
    }

    #[test]
    fn test_preview_withdrawal() {
        let preview = PreviewGenerator::preview_withdrawal("creator1", 500);
        assert!(preview.description.contains("500"));
        assert!(preview.outcome.contains("transferred"));
    }

    #[test]
    fn test_high_gas_warning() {
        let simulation = SimulationResult {
            success: true,
            gas_cost: 20000,
            state_changes: vec![],
            events: vec![],
            error: None,
        };

        let preview = PreviewGenerator::generate_preview("batch_tip", &simulation);
        assert!(preview.warnings.iter().any(|w| w.contains("gas")));
    }
}
