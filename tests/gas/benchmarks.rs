/// Gas benchmarks for TipJar contract functions.
///
/// Soroban's test environment tracks CPU instructions and memory bytes consumed
/// per invocation via `env.cost_estimate().budget()`. These benchmarks capture
/// those metrics for each major entry point so regressions are visible in CI
/// output.
///
/// Coverage note: this file benchmarks not just the original v1 tip/withdraw
/// path but every entrypoint added or materially changed by the multi-token
/// allowlist, timelocked upgrade, admin two-step transfer, guardian/pause
/// circuit breaker, operator delegation, and protocol fee features — see
/// `bench_tip_against_full_allowlist`, `bench_propose_upgrade` /
/// `bench_execute_upgrade`, `bench_propose_admin` / `bench_accept_admin`,
/// `bench_guardian_pause_all`, `bench_authorize_operator` /
/// `bench_operator_withdraw`, and `bench_tip_with_fee` below.
///
/// Run with:
///   cargo test -p tipjar --test gas_benchmarks -- --nocapture
#[cfg(test)]
mod bench {
    use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, token, Address, Env};
    use tipjar::{TipJar, TipJarClient};

    // The timelocked-upgrade benchmarks swap this contract's WASM onto a
    // second, genuinely distinct compiled binary, the same way
    // `contracts/tipjar/src/test_upgrade.rs` does — see that file's module
    // docs for why a real WASM swap is needed rather than unit-testing the
    // storage writes in isolation. `file` is relative to this crate's
    // manifest dir (`contracts/tipjar`), regardless of where this test file
    // physically lives.
    //
    // Requires `cargo build -p tipjar-v2-fixture --target wasm32v1-none
    // --release` to have run first — `.github/workflows/gas-check.yml` does
    // this in a dedicated step before profiling.
    mod v2 {
        soroban_sdk::contractimport!(
            file = "../../target/wasm32v1-none/release/tipjar_v2_fixture.wasm"
        );
    }

    /// Mirrors `contracts/tipjar/src/lib.rs`'s private `MAX_ALLOWED_TOKENS`.
    /// Kept in sync manually since the constant isn't part of the public API.
    const MAX_ALLOWED_TOKENS: u32 = 50;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn setup() -> (Env, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        let token_admin = Address::generate(&env);
        let token_id = env
            .register_stellar_asset_contract_v2(token_admin.clone())
            .address();

        let admin = Address::generate(&env);
        let contract_id = env.register(TipJar, ());
        let client = TipJarClient::new(&env, &contract_id);
        client.init(&token_id, &admin, &100);

        (env, contract_id, token_id, admin)
    }

    fn print_budget(env: &Env, label: &str) {
        let budget = env.cost_estimate().budget();
        let cpu = budget.cpu_instruction_cost();
        let mem = budget.memory_bytes_cost();
        println!("[BENCH] {label}: cpu={cpu} instructions, mem={mem} bytes");
    }

    // ── benchmarks ───────────────────────────────────────────────────────────

    #[test]
    fn bench_tip_single() {
        let (env, contract_id, token_id, _) = setup();
        let client = TipJarClient::new(&env, &contract_id);
        let token_admin = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        token_admin.mint(&sender, &1_000_000);

        env.cost_estimate().budget().reset_default();
        client.tip(&sender, &creator, &token_id, &1_000_000);
        print_budget(&env, "tip (first, cold storage)");
    }

    #[test]
    fn bench_tip_warm_storage() {
        let (env, contract_id, token_id, _) = setup();
        let client = TipJarClient::new(&env, &contract_id);
        let token_admin = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        token_admin.mint(&sender, &2_000_000);

        // Warm up storage entries for this creator.
        client.tip(&sender, &creator, &token_id, &1_000);

        env.cost_estimate().budget().reset_default();
        client.tip(&sender, &creator, &token_id, &1_000);
        print_budget(&env, "tip (second, warm storage)");
    }

    #[test]
    fn bench_withdraw() {
        let (env, contract_id, token_id, _) = setup();
        let client = TipJarClient::new(&env, &contract_id);
        let token_admin = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        token_admin.mint(&sender, &1_000_000);

        client.tip(&sender, &creator, &token_id, &1_000_000);

        env.cost_estimate().budget().reset_default();
        client.withdraw(&creator, &creator, &token_id, &creator, &None);
        print_budget(&env, "withdraw");
    }

    #[test]
    fn bench_get_total_tips() {
        let (env, contract_id, token_id, _) = setup();
        let client = TipJarClient::new(&env, &contract_id);
        let token_admin = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        token_admin.mint(&sender, &1_000);
        client.tip(&sender, &creator, &token_id, &1_000);

        env.cost_estimate().budget().reset_default();
        client.get_total_tips(&creator, &token_id);
        print_budget(&env, "get_total_tips");
    }

    #[test]
    fn bench_get_balance() {
        let (env, contract_id, token_id, _) = setup();
        let client = TipJarClient::new(&env, &contract_id);
        let token_admin = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        token_admin.mint(&sender, &1_000);
        client.tip(&sender, &creator, &token_id, &1_000);

        env.cost_estimate().budget().reset_default();
        client.get_balance(&creator, &token_id);
        print_budget(&env, "get_balance");
    }

    #[test]
    fn bench_add_token() {
        let (env, contract_id, _token_id, admin) = setup();
        let client = TipJarClient::new(&env, &contract_id);

        let second_token_admin = Address::generate(&env);
        let second_token_id = env
            .register_stellar_asset_contract_v2(second_token_admin)
            .address();

        env.cost_estimate().budget().reset_default();
        client.add_token(&admin, &second_token_id);
        print_budget(&env, "add_token");
    }

    /// `tip`'s `ensure_token_allowed` linearly scans `AllowedTokens`, so its
    /// cost grows with allowlist size. `bench_tip_single`/`bench_tip_warm_storage`
    /// only ever exercise a 1-token allowlist (the token passed to `init`) —
    /// this benchmarks the worst case: the allowlist filled to its
    /// `MAX_ALLOWED_TOKENS` cap, tipping with the *last* token added so the
    /// scan runs to completion.
    #[test]
    fn bench_tip_against_full_allowlist() {
        let (env, contract_id, token_id, admin) = setup();
        let client = TipJarClient::new(&env, &contract_id);

        // `init` already registered `token_id`, so add MAX_ALLOWED_TOKENS - 1
        // more to reach the cap.
        let mut last_token = token_id.clone();
        for _ in 0..(MAX_ALLOWED_TOKENS - 1) {
            let extra_admin = Address::generate(&env);
            let extra_token = env
                .register_stellar_asset_contract_v2(extra_admin)
                .address();
            client.add_token(&admin, &extra_token);
            last_token = extra_token;
        }

        let token_admin = token::StellarAssetClient::new(&env, &last_token);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        token_admin.mint(&sender, &1_000_000);

        env.cost_estimate().budget().reset_default();
        client.tip(&sender, &creator, &last_token, &1_000_000);
        print_budget(&env, "tip (full allowlist, worst-case scan)");
    }

    /// Fee-bearing tip: exercises the `FeeBps`/`FeeBalanceToken` bookkeeping
    /// and `FeeCharged` event path that a zero-fee tip (the case benched by
    /// `bench_tip_single`) skips entirely.
    #[test]
    fn bench_tip_with_fee() {
        let (env, contract_id, token_id, admin) = setup();
        let client = TipJarClient::new(&env, &contract_id);
        let collector = Address::generate(&env);
        client.set_fee(&admin, &250, &collector); // 2.5%

        let token_admin = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        token_admin.mint(&sender, &1_000_000);

        env.cost_estimate().budget().reset_default();
        client.tip(&sender, &creator, &token_id, &1_000_000);
        print_budget(&env, "tip (with protocol fee)");
    }

    #[test]
    fn bench_authorize_operator() {
        let (env, contract_id, _token_id, _admin) = setup();
        let client = TipJarClient::new(&env, &contract_id);
        let creator = Address::generate(&env);
        let operator = Address::generate(&env);

        env.cost_estimate().budget().reset_default();
        client.authorize_operator(
            &creator,
            &operator,
            &1_000_000,
            &(env.ledger().sequence() + 1000),
        );
        print_budget(&env, "authorize_operator");
    }

    /// The delegated-withdrawal path: an operator (distinct from the creator)
    /// spends down an allowance authorized via `authorize_operator`, which
    /// `bench_withdraw`'s self-withdrawal never exercises.
    #[test]
    fn bench_operator_withdraw() {
        let (env, contract_id, token_id, _admin) = setup();
        let client = TipJarClient::new(&env, &contract_id);
        let token_admin = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        let operator = Address::generate(&env);
        token_admin.mint(&sender, &1_000_000);

        client.tip(&sender, &creator, &token_id, &1_000_000);
        client.authorize_operator(
            &creator,
            &operator,
            &1_000_000,
            &(env.ledger().sequence() + 1000),
        );

        env.cost_estimate().budget().reset_default();
        client.withdraw(&operator, &creator, &token_id, &creator, &None);
        print_budget(&env, "withdraw (via delegated operator)");
    }

    #[test]
    fn bench_propose_admin() {
        let (env, contract_id, _token_id, admin) = setup();
        let client = TipJarClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        env.cost_estimate().budget().reset_default();
        client.propose_admin(&admin, &new_admin);
        print_budget(&env, "propose_admin");
    }

    #[test]
    fn bench_accept_admin() {
        let (env, contract_id, _token_id, admin) = setup();
        let client = TipJarClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        client.propose_admin(&admin, &new_admin);

        env.cost_estimate().budget().reset_default();
        client.accept_admin(&new_admin);
        print_budget(&env, "accept_admin (two-step transfer completes)");
    }

    /// The timelocked-upgrade proposal path. `propose_upgrade` itself does no
    /// WASM swap — see `bench_execute_upgrade` for the full lifecycle.
    #[test]
    fn bench_propose_upgrade() {
        let (env, contract_id, _token_id, admin) = setup();
        let client = TipJarClient::new(&env, &contract_id);
        let hash = env.deployer().upload_contract_wasm(v2::WASM);

        env.cost_estimate().budget().reset_default();
        client.propose_upgrade(&admin, &hash);
        print_budget(&env, "propose_upgrade");
    }

    /// Full timelocked-upgrade lifecycle: propose, wait out the timelock, then
    /// swap this contract's WASM onto a genuinely distinct compiled binary via
    /// `execute_upgrade`. See `contracts/tipjar/src/test_upgrade.rs` for why a
    /// real WASM swap (rather than unit-testing the storage writes) is needed.
    #[test]
    fn bench_execute_upgrade() {
        let (env, contract_id, _token_id, admin) = setup();
        let client = TipJarClient::new(&env, &contract_id);
        let hash = env.deployer().upload_contract_wasm(v2::WASM);
        client.propose_upgrade(&admin, &hash);

        // `setup()` initializes with a 100-ledger timelock.
        env.ledger().with_mut(|li| li.sequence_number += 100);

        env.cost_estimate().budget().reset_default();
        client.execute_upgrade();
        print_budget(&env, "execute_upgrade (WASM swap after timelock)");
    }

    /// Guardian-triggered circuit breaker: distinct code path from an
    /// admin-triggered pause (persistent, no expiry bookkeeping) since it also
    /// records `guardian_expiry`.
    #[test]
    fn bench_guardian_pause_all() {
        let (env, contract_id, _token_id, admin) = setup();
        let client = TipJarClient::new(&env, &contract_id);
        let guardian = Address::generate(&env);
        client.set_guardian(&admin, &guardian);

        env.cost_estimate().budget().reset_default();
        client.pause_all(&guardian);
        print_budget(&env, "pause_all (guardian-triggered)");
    }
}
