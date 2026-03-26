/// Gas benchmarks for TipJar contract functions.
///
/// Soroban's test environment tracks CPU instructions and memory bytes consumed
/// per invocation via `env.budget()`. These benchmarks capture those metrics
/// for each major entry point so regressions are visible in CI output.
///
/// Run with:
///   cargo test -p tipjar -- bench --nocapture
#[cfg(test)]
mod bench {
    use soroban_sdk::{
        testutils::{Address as _, Ledger as _},
        token, Address, Env, Map, String,
    };
    use tipjar::{BatchTip, Role, TipJarContract, TipJarContractClient};

    // ── helpers ──────────────────────────────────────────────────────────────

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

    fn print_budget(env: &Env, label: &str) {
        let cpu = env.budget().cpu_instruction_count();
        let mem = env.budget().memory_bytes_count();
        println!("[BENCH] {label}: cpu={cpu} instructions, mem={mem} bytes");
    }

    // ── benchmarks ───────────────────────────────────────────────────────────

    #[test]
    fn bench_tip_single() {
        let (env, contract_id, token_id, _) = setup();
        let client = TipJarContractClient::new(&env, &contract_id);
        let token_admin = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        token_admin.mint(&sender, &1_000_000);

        env.budget().reset_default();
        client.tip(&sender, &creator, &token_id, &1_000_000);
        print_budget(&env, "tip (first, cold storage)");
    }

    #[test]
    fn bench_tip_warm_storage() {
        let (env, contract_id, token_id, _) = setup();
        let client = TipJarContractClient::new(&env, &contract_id);
        let token_admin = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        token_admin.mint(&sender, &2_000_000);

        // Warm up storage entries for this creator.
        client.tip(&sender, &creator, &token_id, &1_000);

        env.budget().reset_default();
        client.tip(&sender, &creator, &token_id, &1_000);
        print_budget(&env, "tip (second, warm storage)");
    }

    #[test]
    fn bench_tip_with_message() {
        let (env, contract_id, token_id, _) = setup();
        let client = TipJarContractClient::new(&env, &contract_id);
        let token_admin = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        token_admin.mint(&sender, &1_000_000);

        let message = String::from_str(&env, "Great content, keep it up!");
        let metadata = Map::new(&env);

        env.budget().reset_default();
        client.tip_with_message(&sender, &creator, &token_id, &1_000_000, &message, &metadata);
        print_budget(&env, "tip_with_message (cold storage)");
    }

    #[test]
    fn bench_withdraw() {
        let (env, contract_id, token_id, admin) = setup();
        let client = TipJarContractClient::new(&env, &contract_id);
        let token_admin = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        token_admin.mint(&sender, &1_000_000);

        client.tip(&sender, &creator, &token_id, &1_000_000);
        client.grant_role(&admin, &creator, &Role::Creator);

        env.budget().reset_default();
        client.withdraw(&creator, &token_id);
        print_budget(&env, "withdraw");
    }

    #[test]
    fn bench_tip_batch_10() {
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
        print_budget(&env, "tip_batch (10 entries)");
    }

    #[test]
    fn bench_tip_batch_50() {
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
        print_budget(&env, "tip_batch (50 entries, max batch)");
    }

    #[test]
    fn bench_get_total_tips() {
        let (env, contract_id, token_id, _) = setup();
        let client = TipJarContractClient::new(&env, &contract_id);
        let token_admin = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        token_admin.mint(&sender, &1_000);
        client.tip(&sender, &creator, &token_id, &1_000);

        env.budget().reset_default();
        client.get_total_tips(&creator, &token_id);
        print_budget(&env, "get_total_tips");
    }

    #[test]
    fn bench_tip_locked() {
        let (env, contract_id, token_id, _) = setup();
        let client = TipJarContractClient::new(&env, &contract_id);
        let token_admin = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        token_admin.mint(&sender, &1_000_000);

        let unlock_ts = env.ledger().timestamp() + 1_000;

        env.budget().reset_default();
        client.tip_locked(&sender, &creator, &token_id, &1_000_000, &unlock_ts);
        print_budget(&env, "tip_locked");
    }
}
