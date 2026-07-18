/// Example: Simulate a tip transaction
use tipjar_sdk::{PreviewGenerator, TransactionSimulator};

fn main() {
    println!("=== TipJar Transaction Simulation Example ===\n");

    // Simulate a tip
    println!("1. Simulating a tip transaction:");
    match TransactionSimulator::simulate_tip("creator_address", 1000, "tipper_address") {
        Ok(result) => {
            println!("   Success: {}", result.success);
            println!("   Gas Cost: {} stroops", result.gas_cost);
            println!("   State Changes: {}", result.state_changes.len());
            println!("   Events: {}", result.events.len());

            // Generate preview
            let preview = PreviewGenerator::generate_preview("tip", &result);
            println!("\n   Preview:");
            println!("   - Description: {}", preview.description);
            println!("   - Outcome: {}", preview.outcome);
            println!("   - Estimated Cost: {} stroops", preview.estimated_cost);
        }
        Err(e) => println!("   Error: {}", e),
    }

    // Simulate a withdrawal
    println!("\n2. Simulating a withdrawal transaction:");
    match TransactionSimulator::simulate_withdrawal("creator_address", 500, 1000) {
        Ok(result) => {
            println!("   Success: {}", result.success);
            println!("   Gas Cost: {} stroops", result.gas_cost);

            let preview = PreviewGenerator::preview_withdrawal("creator_address", 500);
            println!("\n   Preview:");
            println!("   - Description: {}", preview.description);
            println!("   - Estimated Cost: {} stroops", preview.estimated_cost);
        }
        Err(e) => println!("   Error: {}", e),
    }

    // Simulate batch tips
    println!("\n3. Simulating batch tips:");
    let tips = vec![
        ("creator1".to_string(), 100),
        ("creator2".to_string(), 200),
        ("creator3".to_string(), 150),
    ];

    match TransactionSimulator::simulate_batch_tips(tips) {
        Ok(result) => {
            println!("   Success: {}", result.success);
            println!("   Gas Cost: {} stroops", result.gas_cost);
            println!("   State Changes: {}", result.state_changes.len());
            println!("   Events: {}", result.events.len());
        }
        Err(e) => println!("   Error: {}", e),
    }

    println!("\n=== Simulation Complete ===");
}
