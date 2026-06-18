/// Criterion benchmark suite for the TipJar contract.
///
/// Measures wall-clock execution time for all public contract functions using
/// the Soroban test environment. Each benchmark group covers a distinct
/// function category: tipping, withdrawals, queries, and batch operations.
///
/// Run with:
///   cargo bench --package tipjar --bench contract_bench
///
/// Compare against a saved baseline:
///   cargo bench --package tipjar --bench contract_bench -- --baseline main
///
/// Save a new baseline:
///   cargo bench --package tipjar --bench contract_bench -- --save-baseline main
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use soroban_sdk::{testutils::Address as _, token, Address, Env, String, Vec};
use tipjar::{TipJarContract, TipJarContractClient, TipOperation};

// ── shared setup ─────────────────────────────────────────────────────────────

/// Initialises a fresh environment with the TipJar contract and a whitelisted
/// token. Returns `(env, contract_id, token_id, admin)`.
///
/// The budget is reset to unlimited so that setup overhead is excluded from
/// benchmark measurements.
fn setup_contract() -> (Env, Address, Address, Address) {
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

// ── tip benchmarks ────────────────────────────────────────────────────────────

/// Benchmarks a cold `tip` call (first tip for a creator — allocates new
/// ledger entries).
fn bench_tip(c: &mut Criterion) {
    c.bench_function("tip", |b| {
        b.iter(|| {
            let (env, contract_id, token_id, _) = setup_contract();
            let client = TipJarContractClient::new(&env, &contract_id);
            let token_client = token::StellarAssetClient::new(&env, &token_id);

            let sender = Address::generate(&env);
            let creator = Address::generate(&env);
            token_client.mint(&sender, &1_000);

            client.tip(&sender, &creator, &token_id, black_box(&100i128));
        });
    });
}

/// Benchmarks a warm `tip` call (subsequent tip for the same creator — ledger
/// entries already exist and are cheaper to update).
fn bench_tip_warm(c: &mut Criterion) {
    c.bench_function("tip_warm", |b| {
        b.iter(|| {
            let (env, contract_id, token_id, _) = setup_contract();
            let client = TipJarContractClient::new(&env, &contract_id);
            let token_client = token::StellarAssetClient::new(&env, &token_id);

            let sender = Address::generate(&env);
            let creator = Address::generate(&env);
            token_client.mint(&sender, &2_000);

            // Warm up storage — not measured.
            client.tip(&sender, &creator, &token_id, &100i128);

            // Measured call.
            client.tip(&sender, &creator, &token_id, black_box(&100i128));
        });
    });
}

/// Benchmarks `tip_with_message` with a short optional message.
fn bench_tip_with_message(c: &mut Criterion) {
    c.bench_function("tip_with_message", |b| {
        b.iter(|| {
            let (env, contract_id, token_id, _) = setup_contract();
            let client = TipJarContractClient::new(&env, &contract_id);
            let token_client = token::StellarAssetClient::new(&env, &token_id);

            let sender = Address::generate(&env);
            let creator = Address::generate(&env);
            token_client.mint(&sender, &1_000);

            let message = String::from_str(&env, "Great content, keep it up!");
            client.tip_with_message(
                &sender,
                &creator,
                &token_id,
                black_box(&100i128),
                &Some(message),
            );
        });
    });
}

/// Benchmarks `tip_with_message` with no message (None path).
fn bench_tip_no_message(c: &mut Criterion) {
    c.bench_function("tip_no_message", |b| {
        b.iter(|| {
            let (env, contract_id, token_id, _) = setup_contract();
            let client = TipJarContractClient::new(&env, &contract_id);
            let token_client = token::StellarAssetClient::new(&env, &token_id);

            let sender = Address::generate(&env);
            let creator = Address::generate(&env);
            token_client.mint(&sender, &1_000);

            client.tip_with_message(&sender, &creator, &token_id, black_box(&100i128), &None);
        });
    });
}

// ── withdraw benchmarks ───────────────────────────────────────────────────────

/// Benchmarks `withdraw` after a single prior tip.
fn bench_withdraw(c: &mut Criterion) {
    c.bench_function("withdraw", |b| {
        b.iter(|| {
            let (env, contract_id, token_id, _) = setup_contract();
            let client = TipJarContractClient::new(&env, &contract_id);
            let token_client = token::StellarAssetClient::new(&env, &token_id);

            let sender = Address::generate(&env);
            let creator = Address::generate(&env);
            token_client.mint(&sender, &1_000);

            // Pre-state: creator has a balance — not measured.
            client.tip(&sender, &creator, &token_id, &100i128);

            client.withdraw(&creator, black_box(&token_id));
        });
    });
}

// ── query benchmarks ──────────────────────────────────────────────────────────

/// Benchmarks `get_withdrawable_balance` on a warm storage entry.
fn bench_get_balance(c: &mut Criterion) {
    let (env, contract_id, token_id, _) = setup_contract();
    let client = TipJarContractClient::new(&env, &contract_id);
    let token_client = token::StellarAssetClient::new(&env, &token_id);

    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    token_client.mint(&sender, &1_000);
    client.tip(&sender, &creator, &token_id, &100i128);

    c.bench_function("get_withdrawable_balance", |b| {
        b.iter(|| {
            client.get_withdrawable_balance(black_box(&creator), black_box(&token_id));
        });
    });
}

/// Benchmarks `get_total_tips` on a warm storage entry.
fn bench_get_total_tips(c: &mut Criterion) {
    let (env, contract_id, token_id, _) = setup_contract();
    let client = TipJarContractClient::new(&env, &contract_id);
    let token_client = token::StellarAssetClient::new(&env, &token_id);

    let sender = Address::generate(&env);
    let creator = Address::generate(&env);
    token_client.mint(&sender, &1_000);
    client.tip(&sender, &creator, &token_id, &100i128);

    c.bench_function("get_total_tips", |b| {
        b.iter(|| {
            client.get_total_tips(black_box(&creator), black_box(&token_id));
        });
    });
}

/// Benchmarks `is_whitelisted` — a simple instance-storage read.
fn bench_is_whitelisted(c: &mut Criterion) {
    let (env, contract_id, token_id, _) = setup_contract();
    let client = TipJarContractClient::new(&env, &contract_id);

    c.bench_function("is_whitelisted", |b| {
        b.iter(|| {
            client.is_whitelisted(black_box(&token_id));
        });
    });
}

/// Benchmarks `is_paused` — a simple instance-storage read.
fn bench_is_paused(c: &mut Criterion) {
    let (env, contract_id, _, _) = setup_contract();
    let client = TipJarContractClient::new(&env, &contract_id);

    c.bench_function("is_paused", |b| {
        b.iter(|| {
            client.is_paused();
        });
    });
}

// ── batch tip benchmarks ──────────────────────────────────────────────────────

/// Benchmarks `batch_tip_v2` across several batch sizes to confirm linear
/// scaling and detect regressions.
fn bench_multiple_tips(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_tip_v2");

    for &batch_size in &[1usize, 10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |b, &size| {
                b.iter(|| {
                    let (env, contract_id, token_id, _) = setup_contract();
                    let client = TipJarContractClient::new(&env, &contract_id);
                    let token_client = token::StellarAssetClient::new(&env, &token_id);

                    let creator = Address::generate(&env);
                    let sender = Address::generate(&env);
                    // Mint enough for all tips in the batch.
                    token_client.mint(&sender, &(size as i128 * 100));

                    let mut ops: Vec<TipOperation> = Vec::new(&env);
                    for _ in 0..size {
                        ops.push_back(TipOperation {
                            creator: creator.clone(),
                            token: token_id.clone(),
                            amount: 10,
                        });
                    }

                    client.batch_tip_v2(&sender, black_box(&ops));
                });
            },
        );
    }

    group.finish();
}

/// Benchmarks 100 individual `tip` calls to a single creator, measuring the
/// cumulative cost of repeated tipping (warm-path scaling).
fn bench_100_tips(c: &mut Criterion) {
    c.bench_function("100_tips", |b| {
        b.iter(|| {
            let (env, contract_id, token_id, _) = setup_contract();
            let client = TipJarContractClient::new(&env, &contract_id);
            let token_client = token::StellarAssetClient::new(&env, &token_id);

            let creator = Address::generate(&env);

            for _ in 0..100 {
                let sender = Address::generate(&env);
                token_client.mint(&sender, &100);
                client.tip(&sender, &creator, &token_id, black_box(&10i128));
            }
        });
    });
}

// ── admin / config benchmarks ─────────────────────────────────────────────────

/// Benchmarks `add_token` — an admin write to instance storage.
fn bench_add_token(c: &mut Criterion) {
    c.bench_function("add_token", |b| {
        b.iter(|| {
            let (env, contract_id, _, admin) = setup_contract();
            let client = TipJarContractClient::new(&env, &contract_id);

            let new_token_admin = Address::generate(&env);
            let new_token = env
                .register_stellar_asset_contract_v2(new_token_admin)
                .address();

            client.add_token(&admin, black_box(&new_token));
        });
    });
}

/// Benchmarks `pause` followed by `unpause` — two admin writes.
fn bench_pause_unpause(c: &mut Criterion) {
    c.bench_function("pause_unpause", |b| {
        b.iter(|| {
            let (env, contract_id, _, admin) = setup_contract();
            let client = TipJarContractClient::new(&env, &contract_id);

            let reason = String::from_str(&env, "maintenance");
            client.pause(&admin, &reason, &None);
            client.unpause(&admin);
        });
    });
}

// ── criterion groups ──────────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_tip,
    bench_tip_warm,
    bench_tip_with_message,
    bench_tip_no_message,
    bench_withdraw,
    bench_get_balance,
    bench_get_total_tips,
    bench_is_whitelisted,
    bench_is_paused,
    bench_multiple_tips,
    bench_100_tips,
    bench_add_token,
    bench_pause_unpause,
);
criterion_main!(benches);
