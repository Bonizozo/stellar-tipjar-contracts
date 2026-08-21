/// Gas benchmarks for TipJar contract functions.
///
/// Soroban's test environment tracks CPU instructions and memory bytes consumed
/// per invocation via `env.cost_estimate().budget()`. These benchmarks capture
/// those metrics for each major entry point so regressions are visible in CI
/// output.
///
/// Run with:
///   cargo test -p tipjar --test gas_benchmarks -- --nocapture
#[cfg(test)]
mod bench {
    use soroban_sdk::{testutils::Address as _, token, Address, Env};
    use tipjar::{TipJar, TipJarClient};

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
}
