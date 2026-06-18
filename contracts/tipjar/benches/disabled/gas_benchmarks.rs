/// Contract Performance Benchmarking Suite for TipJar.
///
/// Uses the Soroban test environment's `env.budget()` API to capture exact
/// CPU instruction counts and memory byte counts per contract invocation.
/// Each benchmark resets the budget immediately before the measured call so
/// that setup overhead is excluded from the result.
///
/// Output format per benchmark:
///   BENCH <name> cpu=<n> mem=<n>
///
/// On threshold violation:
///   BENCH_FAIL <name>: cpu=<actual> exceeded threshold=<limit>
///
/// Run with:
///   cargo test --package tipjar --test gas_benchmarks -- --nocapture
extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token, Address, Env, Map, String,
};
use tipjar::{BatchTip, TipJarContract, TipJarContractClient};

// ── shared setup ─────────────────────────────────────────────────────────────

/// Registers the TipJar contract and a whitelisted mock token.
/// Returns (env, contract_id, token_id, admin).
///
/// The budget is reset to unlimited during setup so that registration and
/// initialisation costs do not pollute benchmark measurements.
fn setup() -> (Env, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
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

// ── benchmarks ───────────────────────────────────────────────────────────────

/// Measures the cost of the very first `tip` for a creator (cold storage).
///
/// Cold storage is more expensive because ledger entries must be allocated.
/// No threshold is asserted; the result serves as a baseline for warm comparisons.
#[test]
fn gas_bench_tip_cold() {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let token_admin = token::StellarAssetClient::new(&env, &token_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    token_admin.mint(&sender, &1_000_000);

    env.budget().reset_default();
    client.tip(&sender, &creator, &token_id, &1_000_000);
    let cpu = env.budget().cpu_instruction_count();
    let mem = env.budget().memory_bytes_count();

    println!("BENCH tip_cold cpu={cpu} mem={mem}");
}

/// Measures the cost of a subsequent `tip` for a creator (warm storage).
///
/// Warm storage is cheaper because ledger entries already exist.
/// Threshold: 5,000,000 CPU instructions.
#[test]
fn gas_bench_tip_warm() {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let token_admin = token::StellarAssetClient::new(&env, &token_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    token_admin.mint(&sender, &2_000_000);

    // Warm up storage entries — not measured.
    client.tip(&sender, &creator, &token_id, &1_000);

    env.budget().reset_default();
    client.tip(&sender, &creator, &token_id, &1_000);
    let cpu = env.budget().cpu_instruction_count();
    let mem = env.budget().memory_bytes_count();

    println!("BENCH tip_warm cpu={cpu} mem={mem}");

    const THRESHOLD: u64 = 5_000_000;
    assert!(
        cpu < THRESHOLD,
        "BENCH_FAIL tip_warm: cpu={cpu} exceeded threshold={THRESHOLD}"
    );
}

/// Measures the cost of `tip_with_message` (cold storage).
///
/// String serialisation adds overhead compared to a plain tip.
/// Threshold: 8,000,000 CPU instructions.
#[test]
fn gas_bench_tip_with_message() {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let token_admin = token::StellarAssetClient::new(&env, &token_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    token_admin.mint(&sender, &1_000_000);

    let message = String::from_str(&env, "Great content, keep it up!");
    let metadata = Map::new(&env);

    env.budget().reset_default();
    client.tip_with_message(
        &sender, &creator, &token_id, &1_000_000, &message, &metadata,
    );
    let cpu = env.budget().cpu_instruction_count();
    let mem = env.budget().memory_bytes_count();

    println!("BENCH tip_with_message cpu={cpu} mem={mem}");

    const THRESHOLD: u64 = 8_000_000;
    assert!(
        cpu < THRESHOLD,
        "BENCH_FAIL tip_with_message: cpu={cpu} exceeded threshold={THRESHOLD}"
    );
}

/// Measures the cost of `withdraw` after a prior tip has been made.
///
/// Threshold: 5,000,000 CPU instructions.
#[test]
fn gas_bench_withdraw() {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let token_admin = token::StellarAssetClient::new(&env, &token_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    token_admin.mint(&sender, &1_000_000);

    // Pre-state: creator has a balance to withdraw — not measured.
    client.tip(&sender, &creator, &token_id, &1_000_000);

    env.budget().reset_default();
    client.withdraw(&creator, &token_id);
    let cpu = env.budget().cpu_instruction_count();
    let mem = env.budget().memory_bytes_count();

    println!("BENCH withdraw cpu={cpu} mem={mem}");

    const THRESHOLD: u64 = 5_000_000;
    assert!(
        cpu < THRESHOLD,
        "BENCH_FAIL withdraw: cpu={cpu} exceeded threshold={THRESHOLD}"
    );
}

/// Measures the cost of `tip_batch` with 10 entries.
///
/// No threshold; result is compared against the batch-50 benchmark to
/// confirm linear scaling.
#[test]
fn gas_bench_tip_batch_10() {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let token_admin = token::StellarAssetClient::new(&env, &token_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    token_admin.mint(&sender, &100_000);

    let mut tips = soroban_sdk::Vec::new(&env);
    for _ in 0..10 {
        tips.push_back(BatchTip {
            creator: creator.clone(),
            token: token_id.clone(),
            amount: 1_000,
        });
    }

    env.budget().reset_default();
    client.tip_batch(&sender, &tips);
    let cpu = env.budget().cpu_instruction_count();
    let mem = env.budget().memory_bytes_count();

    println!("BENCH tip_batch_10 cpu={cpu} mem={mem}");
}

/// Measures the cost of `tip_batch` with 50 entries (maximum allowed batch size).
///
/// Threshold: 50,000,000 CPU instructions.
#[test]
fn gas_bench_tip_batch_50() {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let token_admin = token::StellarAssetClient::new(&env, &token_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    token_admin.mint(&sender, &500_000);

    let mut tips = soroban_sdk::Vec::new(&env);
    for _ in 0..50 {
        tips.push_back(BatchTip {
            creator: creator.clone(),
            token: token_id.clone(),
            amount: 1_000,
        });
    }

    env.budget().reset_default();
    client.tip_batch(&sender, &tips);
    let cpu = env.budget().cpu_instruction_count();
    let mem = env.budget().memory_bytes_count();

    println!("BENCH tip_batch_50 cpu={cpu} mem={mem}");

    const THRESHOLD: u64 = 50_000_000;
    assert!(
        cpu < THRESHOLD,
        "BENCH_FAIL tip_batch_50: cpu={cpu} exceeded threshold={THRESHOLD}"
    );
}

/// Measures the cost of `tip_locked` with a future unlock timestamp.
///
/// No threshold; result captures the overhead of locked-tip storage allocation.
#[test]
fn gas_bench_tip_locked() {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let token_admin = token::StellarAssetClient::new(&env, &token_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    token_admin.mint(&sender, &1_000_000);

    let unlock_ts = env.ledger().timestamp() + 1_000;

    env.budget().reset_default();
    client.tip_locked(&sender, &creator, &token_id, &1_000_000, &unlock_ts);
    let cpu = env.budget().cpu_instruction_count();
    let mem = env.budget().memory_bytes_count();

    println!("BENCH tip_locked cpu={cpu} mem={mem}");
}

/// Measures the cost of `get_total_tips` after a prior tip (warm storage read).
///
/// Threshold: 1,000,000 CPU instructions.
#[test]
fn gas_bench_get_total_tips() {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let token_admin = token::StellarAssetClient::new(&env, &token_id);
    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    token_admin.mint(&sender, &1_000);

    // Pre-state: ensure the storage entry exists — not measured.
    client.tip(&sender, &creator, &token_id, &1_000);

    env.budget().reset_default();
    client.get_total_tips(&creator, &token_id);
    let cpu = env.budget().cpu_instruction_count();
    let mem = env.budget().memory_bytes_count();

    println!("BENCH get_total_tips cpu={cpu} mem={mem}");

    const THRESHOLD: u64 = 1_000_000;
    assert!(
        cpu < THRESHOLD,
        "BENCH_FAIL get_total_tips: cpu={cpu} exceeded threshold={THRESHOLD}"
    );
}

/// Measures the cost of querying `get_total_tips` across 3 distinct tippers
/// to approximate leaderboard top-tippers read cost.
///
/// No threshold; result is informational.
#[test]
fn gas_bench_get_top_tippers() {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let token_admin = token::StellarAssetClient::new(&env, &token_id);
    let creator = Address::generate(&env);

    // Seed 3 distinct senders — not measured.
    for _ in 0..3 {
        let sender = Address::generate(&env);
        token_admin.mint(&sender, &1_000);
        client.tip(&sender, &creator, &token_id, &1_000);
    }

    env.budget().reset_default();
    // Query the creator's cumulative total (aggregates all tippers).
    client.get_total_tips(&creator, &token_id);
    let cpu = env.budget().cpu_instruction_count();
    let mem = env.budget().memory_bytes_count();

    println!("BENCH get_top_tippers cpu={cpu} mem={mem}");
}

/// Measures the cost of querying `get_total_tips` across 3 distinct creators
/// to approximate leaderboard top-creators read cost.
///
/// No threshold; result is informational.
#[test]
fn gas_bench_get_top_creators() {
    let (env, contract_id, token_id, _) = setup();
    let client = TipJarContractClient::new(&env, &contract_id);
    let token_admin = token::StellarAssetClient::new(&env, &token_id);
    let sender = Address::generate(&env);
    token_admin.mint(&sender, &10_000);

    // Seed 3 distinct creators — not measured.
    let mut creators = std::vec::Vec::new();
    for _ in 0..3 {
        let creator = Address::generate(&env);
        client.tip(&sender, &creator, &token_id, &1_000);
        creators.push(creator);
    }

    env.budget().reset_default();
    // Query each creator's total; the cumulative budget reflects leaderboard scan cost.
    for creator in &creators {
        client.get_total_tips(creator, &token_id);
    }
    let cpu = env.budget().cpu_instruction_count();
    let mem = env.budget().memory_bytes_count();

    println!("BENCH get_top_creators cpu={cpu} mem={mem}");
}
