//! Gas estimation integration tests for TipJar contract operations.
//!
//! Each measurement function:
//!   1. Calls `setup()` with `env.budget().reset_unlimited()` so setup overhead
//!      is excluded from results.
//!   2. Resets the budget to default immediately before the measured call.
//!   3. Reads `cpu_instruction_cost()` and `memory_bytes_cost()` after the call.
//!
//! The single `run_all_estimates` test collects every measurement, builds batch
//! and comparison tables, generates optimisation suggestions, and writes the
//! full report to `gas-estimates.json`.  It also appends to `gas-history.ndjson`
//! so cost trends can be tracked across runs.
//!
//! Run with:
//!   cargo test -p gas-estimator --test estimate -- --nocapture

extern crate std;

use chrono::Utc;
use gas_estimator::{
    append_to_history, generate_comparisons, generate_suggestions, make_batch_estimate,
    make_estimate, EstimationReport, GasEstimate,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token, Address, Env, Vec as SorobanVec,
};
use tipjar_legacy::{TipJarContract, TipJarContractClient, TipRecipient};

// ── Shared setup ──────────────────────────────────────────────────────────────

/// Registers the TipJar contract and a whitelisted mock token.
/// Returns `(env, contract_id, token_id, admin)`.
///
/// Budget is reset to unlimited so setup costs do not pollute measurements.
fn setup() -> (Env, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();
    env.budget().reset_unlimited();

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let admin = Address::generate(&env);
    let contract_id = env.register(TipJarContract, ());
    let client = TipJarContractClient::new(&env, &contract_id);
    client.init(&admin);
    client.add_token(&admin, &token_id);

    (env, contract_id, token_id, admin)
}

fn mint(env: &Env, token_id: &Address, recipient: &Address, amount: i128) {
    token::StellarAssetClient::new(env, token_id).mint(recipient, &amount);
}

// ── Core tip operations ───────────────────────────────────────────────────────

/// First tip for a creator — allocates new ledger entries (cold storage).
fn measure_tip_cold() -> GasEstimate {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &token_id, &sender, 1_000_000);

    env.budget().reset_default();
    client.tip(&sender, &creator, &token_id, &1_000_000);
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] tip (cold)  cpu={cpu}  mem={mem}");
    make_estimate("tip", "cold", cpu, mem)
}

/// Subsequent tip for the same creator — ledger entries already exist (warm storage).
fn measure_tip_warm() -> GasEstimate {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &token_id, &sender, 2_000_000);
    client.tip(&sender, &creator, &token_id, &1_000); // warm-up, not measured

    env.budget().reset_default();
    client.tip(&sender, &creator, &token_id, &1_000);
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] tip (warm)  cpu={cpu}  mem={mem}");
    make_estimate("tip", "warm", cpu, mem)
}

// ── tip_with_fee — three congestion levels ────────────────────────────────────

fn measure_tip_with_fee_low() -> GasEstimate {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &token_id, &sender, 1_000_000);

    env.budget().reset_default();
    client.tip_with_fee(&sender, &creator, &token_id, &1_000_000, &0u32); // 0 = Low
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] tip_with_fee (low-congestion)  cpu={cpu}  mem={mem}");
    make_estimate("tip_with_fee", "low-congestion", cpu, mem)
}

fn measure_tip_with_fee_normal() -> GasEstimate {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &token_id, &sender, 1_000_000);

    env.budget().reset_default();
    client.tip_with_fee(&sender, &creator, &token_id, &1_000_000, &1u32); // 1 = Normal
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] tip_with_fee (normal-congestion)  cpu={cpu}  mem={mem}");
    make_estimate("tip_with_fee", "normal-congestion", cpu, mem)
}

fn measure_tip_with_fee_high() -> GasEstimate {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &token_id, &sender, 1_000_000);

    env.budget().reset_default();
    client.tip_with_fee(&sender, &creator, &token_id, &1_000_000, &2u32); // 2 = High
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] tip_with_fee (high-congestion)  cpu={cpu}  mem={mem}");
    make_estimate("tip_with_fee", "high-congestion", cpu, mem)
}

// ── Withdraw & balance queries ────────────────────────────────────────────────

fn measure_withdraw_warm() -> GasEstimate {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &token_id, &sender, 1_000_000);
    client.tip(&sender, &creator, &token_id, &1_000_000); // pre-state, not measured

    env.budget().reset_default();
    client.withdraw(&creator, &token_id, &None);
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] withdraw (warm)  cpu={cpu}  mem={mem}");
    make_estimate("withdraw", "warm", cpu, mem)
}

fn measure_get_withdrawable_balance_warm() -> GasEstimate {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &token_id, &sender, 1_000);
    client.tip(&sender, &creator, &token_id, &1_000);

    env.budget().reset_default();
    client.get_withdrawable_balance(&creator, &token_id);
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] get_withdrawable_balance (warm)  cpu={cpu}  mem={mem}");
    make_estimate("get_withdrawable_balance", "warm", cpu, mem)
}

fn measure_get_total_tips_warm() -> GasEstimate {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &token_id, &sender, 1_000);
    client.tip(&sender, &creator, &token_id, &1_000);

    env.budget().reset_default();
    client.get_total_tips(&creator, &token_id);
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] get_total_tips (warm)  cpu={cpu}  mem={mem}");
    make_estimate("get_total_tips", "warm", cpu, mem)
}

// ── tip_split — 3 and 10 recipients ──────────────────────────────────────────

fn build_recipients(env: &Env, count: u32) -> SorobanVec<TipRecipient> {
    assert!(
        count >= 2 && count <= 10,
        "tip_split requires 2–10 recipients"
    );
    let mut recipients = SorobanVec::new(env);
    let share_each = 10_000u32 / count;
    let remainder = 10_000u32 - share_each * count;
    for i in 0..count {
        let pct = if i == 0 {
            share_each + remainder
        } else {
            share_each
        };
        recipients.push_back(TipRecipient {
            creator: Address::generate(env),
            percentage: pct,
        });
    }
    recipients
}

fn measure_tip_split_3() -> GasEstimate {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let sender = Address::generate(&env);
    mint(&env, &token_id, &sender, 1_000_000);
    let recipients = build_recipients(&env, 3);

    env.budget().reset_default();
    client.tip_split(&sender, &token_id, &recipients, &1_000_000);
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] tip_split (3-recipients)  cpu={cpu}  mem={mem}");
    make_estimate("tip_split", "3-recipients", cpu, mem)
}

fn measure_tip_split_10() -> GasEstimate {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let sender = Address::generate(&env);
    mint(&env, &token_id, &sender, 1_000_000);
    let recipients = build_recipients(&env, 10);

    env.budget().reset_default();
    client.tip_split(&sender, &token_id, &recipients, &1_000_000);
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] tip_split (10-recipients)  cpu={cpu}  mem={mem}");
    make_estimate("tip_split", "10-recipients", cpu, mem)
}

// ── Leaderboard — 1 and 10 creators ──────────────────────────────────────────

fn measure_get_leaderboard_1() -> GasEstimate {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let sender = Address::generate(&env);
    mint(&env, &token_id, &sender, 1_000);
    let creator = Address::generate(&env);
    client.tip(&sender, &creator, &token_id, &1_000);

    env.budget().reset_default();
    client.get_leaderboard(
        &tipjar_legacy::TimePeriod::AllTime,
        &tipjar_legacy::ParticipantKind::Creator,
        &10u32,
    );
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] get_leaderboard (1-creator)  cpu={cpu}  mem={mem}");
    make_estimate("get_leaderboard", "1-creator", cpu, mem)
}

fn measure_get_leaderboard_10() -> GasEstimate {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let sender = Address::generate(&env);
    mint(&env, &token_id, &sender, 100_000);
    for _ in 0..10 {
        let creator = Address::generate(&env);
        client.tip(&sender, &creator, &token_id, &1_000);
    }

    env.budget().reset_default();
    client.get_leaderboard(
        &tipjar_legacy::TimePeriod::AllTime,
        &tipjar_legacy::ParticipantKind::Creator,
        &10u32,
    );
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] get_leaderboard (10-creators)  cpu={cpu}  mem={mem}");
    make_estimate("get_leaderboard", "10-creators", cpu, mem)
}

// ── Subscriptions ─────────────────────────────────────────────────────────────

fn measure_create_subscription_cold() -> GasEstimate {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);

    env.budget().reset_default();
    client.create_subscription(&subscriber, &creator, &token_id, &1_000, &86_400u64);
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] create_subscription (cold)  cpu={cpu}  mem={mem}");
    make_estimate("create_subscription", "cold", cpu, mem)
}

fn measure_execute_subscription_payment_warm() -> GasEstimate {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let subscriber = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &token_id, &subscriber, 10_000);
    client.create_subscription(&subscriber, &creator, &token_id, &1_000, &86_400u64);
    env.ledger().with_mut(|l| l.timestamp += 86_400);

    env.budget().reset_default();
    client.execute_subscription_payment(&subscriber, &creator);
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] execute_subscription_payment (warm)  cpu={cpu}  mem={mem}");
    make_estimate("execute_subscription_payment", "warm", cpu, mem)
}

// ── Conditional tip ───────────────────────────────────────────────────────────

fn measure_execute_conditional_tip_cold() -> GasEstimate {
    use tipjar_legacy::conditions::types::Condition;
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    mint(&env, &token_id, &sender, 1_000_000);

    // Condition::Always is always true — minimal overhead, isolates the
    // conditional dispatch cost rather than any specific condition logic.
    let mut conditions = SorobanVec::new(&env);
    conditions.push_back(Condition::Always);

    env.budget().reset_default();
    client.execute_conditional_tip(&sender, &creator, &token_id, &1_000_000, &conditions);
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] execute_conditional_tip (cold)  cpu={cpu}  mem={mem}");
    make_estimate("execute_conditional_tip", "cold", cpu, mem)
}

// ── Cheap read-only operations ────────────────────────────────────────────────

fn measure_is_paused() -> GasEstimate {
    let (env, contract_id, _, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);

    env.budget().reset_default();
    client.is_paused();
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] is_paused  cpu={cpu}  mem={mem}");
    make_estimate("is_paused", "warm", cpu, mem)
}

fn measure_get_current_fee_bps() -> GasEstimate {
    let (env, contract_id, _, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);

    env.budget().reset_default();
    client.get_current_fee_bps();
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] get_current_fee_bps  cpu={cpu}  mem={mem}");
    make_estimate("get_current_fee_bps", "warm", cpu, mem)
}

fn measure_get_version() -> GasEstimate {
    let (env, contract_id, _, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);

    env.budget().reset_default();
    client.get_version();
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    println!("[GAS] get_version  cpu={cpu}  mem={mem}");
    make_estimate("get_version", "warm", cpu, mem)
}

// ── Main test entry point ─────────────────────────────────────────────────────

#[test]
fn run_all_estimates() {
    println!("\n╔══════════════════════════════════════════════════════╗");
    println!("║         TipJar Gas Estimation Suite                 ║");
    println!("╚══════════════════════════════════════════════════════╝\n");

    // ── Collect all per-function estimates ────────────────────────────────────
    let mut estimates: std::vec::Vec<GasEstimate> = std::vec![
        // Core tip
        measure_tip_cold(),
        measure_tip_warm(),
        // tip_with_fee — all three congestion levels
        measure_tip_with_fee_low(),
        measure_tip_with_fee_normal(),
        measure_tip_with_fee_high(),
        // Withdraw & queries
        measure_withdraw_warm(),
        measure_get_withdrawable_balance_warm(),
        measure_get_total_tips_warm(),
        // tip_split — min and max recipient counts
        measure_tip_split_3(),
        measure_tip_split_10(),
        // Leaderboard — 1 and 10 creators
        measure_get_leaderboard_1(),
        measure_get_leaderboard_10(),
        // Subscriptions
        measure_create_subscription_cold(),
        measure_execute_subscription_payment_warm(),
        // Read-only floor
        measure_is_paused(),
        measure_get_current_fee_bps(),
        measure_get_version(),
    ];

    // execute_conditional_tip
    estimates.push(measure_execute_conditional_tip_cold());

    // ── Batch estimates ───────────────────────────────────────────────────────
    // tip_batch is not yet implemented in the contract; extrapolate from
    // cold (first item) + warm (remaining N-1 items) single-tip measurements.
    let tip_cold = estimates
        .iter()
        .find(|e| e.function_name == "tip" && e.storage_variant == "cold")
        .unwrap();
    let tip_warm = estimates
        .iter()
        .find(|e| e.function_name == "tip" && e.storage_variant == "warm")
        .unwrap();

    let batch_estimates: std::vec::Vec<_> = [10u32, 25, 50, 100]
        .iter()
        .map(|&n| {
            let cpu = tip_cold.cpu_instructions + (n as u64 - 1) * tip_warm.cpu_instructions;
            let mem = tip_cold.memory_bytes + (n as u64 - 1) * tip_warm.memory_bytes;
            make_batch_estimate(
                "tip (extrapolated)",
                n,
                true,
                &make_estimate("tip_batch_extrapolated", &format!("batch-{n}"), cpu, mem),
            )
        })
        .collect();

    // ── Comparisons & suggestions ─────────────────────────────────────────────
    let comparisons = generate_comparisons(&estimates);
    let suggestions = generate_suggestions(&estimates);

    let report = EstimationReport {
        timestamp: Utc::now(),
        network: "Stellar Testnet / Mainnet (Soroban)".to_string(),
        estimates,
        batch_estimates,
        comparisons,
        suggestions,
    };

    // ── Print summary ─────────────────────────────────────────────────────────
    println!(
        "\n  {:<45} {:>18} {:>14} {:>16}",
        "Function (variant)", "CPU Instructions", "Memory Bytes", "Est. Cost (XLM)"
    );
    println!("  {}", "─".repeat(97));
    for e in &report.estimates {
        println!(
            "  {:<45} {:>18} {:>14} {:>16.8}",
            format!("{} ({})", e.function_name, e.storage_variant),
            e.cpu_instructions,
            e.memory_bytes,
            e.estimated_cost_xlm,
        );
    }

    println!("\n  Batch Estimates (extrapolated from cold+warm tip):");
    println!(
        "  {:<30} {:>5} {:>18} {:>16} {:>16}",
        "Operation", "N", "Total CPU", "Total XLM", "Per-item XLM"
    );
    println!("  {}", "─".repeat(90));
    for b in &report.batch_estimates {
        println!(
            "  {:<30} {:>5} {:>18} {:>16.8} {:>16.8}",
            b.operation,
            b.batch_size,
            b.total_cpu_instructions,
            b.total_cost_xlm,
            b.cost_per_item_xlm,
        );
    }

    if !report.comparisons.is_empty() {
        println!("\n  Comparisons:");
        for c in &report.comparisons {
            let sign = if c.delta_cpu >= 0 { "+" } else { "" };
            println!(
                "  {:<55}  {}{:.1}%  ({}{} CPU)",
                c.label, sign, c.delta_pct, sign, c.delta_cpu
            );
        }
    }

    if !report.suggestions.is_empty() {
        println!("\n  Suggestions:");
        for s in &report.suggestions {
            println!("  [{:?}] {}: {}", s.severity, s.function, s.message);
        }
    }

    // ── Write report & history ────────────────────────────────────────────────
    let json = serde_json::to_string_pretty(&report).expect("serialise report");
    std::fs::write("gas-estimates.json", &json).expect("write gas-estimates.json");
    println!("\n✅ Report written to gas-estimates.json");

    append_to_history("gas-history.ndjson", &report).expect("append to gas-history.ndjson");
    println!("📝 Appended to gas-history.ndjson");
}
