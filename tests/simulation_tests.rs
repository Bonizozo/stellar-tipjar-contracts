#![cfg(test)]

use tipjar_sdk::{CostCalculator, PreviewGenerator, TransactionSimulator};

#[test]
fn test_simulate_tip_transaction() {
    let result = TransactionSimulator::simulate_tip("creator1", 1000, "tipper1");
    assert!(result.is_ok());

    let sim = result.unwrap();
    assert!(sim.success);
    assert!(sim.gas_cost > 0);
    assert_eq!(sim.state_changes.len(), 2);
    assert_eq!(sim.events.len(), 1);
}

#[test]
fn test_simulate_withdrawal_transaction() {
    let result = TransactionSimulator::simulate_withdrawal("creator1", 500, 1000);
    assert!(result.is_ok());

    let sim = result.unwrap();
    assert!(sim.success);
    assert!(sim.gas_cost > 0);
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
fn test_preview_generation() {
    let result = TransactionSimulator::simulate_tip("creator1", 1000, "tipper1");
    assert!(result.is_ok());

    let sim = result.unwrap();
    let preview = PreviewGenerator::generate_preview("tip", &sim);

    assert!(!preview.description.is_empty());
    assert!(preview.outcome.contains("succeed"));
    assert!(preview.estimated_cost > 0);
}

#[test]
fn test_cost_calculation_tip() {
    let cost = CostCalculator::calculate_tip_cost();
    assert_eq!(cost.base_fee, 100);
    assert!(cost.total_fee > 0);
    assert!(cost.xlm_cost > 0.0);
}

#[test]
fn test_cost_calculation_withdrawal() {
    let tip_cost = CostCalculator::calculate_tip_cost();
    let withdrawal_cost = CostCalculator::calculate_withdrawal_cost();

    assert!(withdrawal_cost.total_fee > tip_cost.total_fee);
}

#[test]
fn test_cost_calculation_batch() {
    let batch_cost = CostCalculator::calculate_batch_cost(10);
    assert!(batch_cost.total_fee > 0);
}

#[test]
fn test_error_handling_invalid_amount() {
    let result = TransactionSimulator::simulate_tip("creator1", 0, "tipper1");
    assert!(result.is_err());
}

#[test]
fn test_error_handling_insufficient_balance() {
    let result = TransactionSimulator::simulate_withdrawal("creator1", 1500, 1000);
    assert!(result.is_err());
}

#[test]
fn test_error_handling_batch_too_large() {
    let tips: Vec<_> = (0..101)
        .map(|i| (format!("creator{}", i), 100))
        .collect();

    let result = TransactionSimulator::simulate_batch_tips(tips);
    assert!(result.is_err());
}

#[test]
fn test_state_changes_preview() {
    let preview = PreviewGenerator::preview_tip("creator1", 1000, "tipper1");
    assert!(preview.changes_summary.contains("creator1"));
    assert!(preview.changes_summary.contains("1000"));
}

#[test]
fn test_events_preview() {
    let preview = PreviewGenerator::preview_tip("creator1", 1000, "tipper1");
    assert!(!preview.events.is_empty());
    assert!(preview.events[0].contains("tipper1"));
}

#[test]
fn test_multi_step_simulation() {
    // Simulate multiple operations in sequence
    let tip_result = TransactionSimulator::simulate_tip("creator1", 1000, "tipper1");
    assert!(tip_result.is_ok());

    let withdrawal_result = TransactionSimulator::simulate_withdrawal("creator1", 500, 1000);
    assert!(withdrawal_result.is_ok());

    // Both should succeed
    assert!(tip_result.unwrap().success);
    assert!(withdrawal_result.unwrap().success);
}
