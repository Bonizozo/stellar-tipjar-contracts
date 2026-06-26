use quickcheck::{quickcheck, TestResult};
use soroban_sdk::{testutils::Address as _, token, Address, Env};
use tipjar_legacy::{TipJarContract, TipJarContractClient};

// ── helpers ──────────────────────────────────────────────────────────────────

fn setup() -> (Env, TipJarContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let admin = Address::generate(&env);
    let contract_id = env.register(TipJarContract, ());
    let client = TipJarContractClient::new(&env, &contract_id);

    client.init(&admin);
    client.add_token(&admin, &token);

    (env, client, token, token_admin)
}

// ── quickcheck properties ────────────────────────────────────────────────────

#[test]
fn qc_tip_amount_conservation() {
    fn prop(amount: i64) -> TestResult {
        if amount <= 0 || amount > 1_000_000 {
            return TestResult::discard();
        }
        let amount = amount as i128;

        let (env, client, token, _token_admin) = setup();
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        token::StellarAssetClient::new(&env, &token).mint(&sender, &amount);

        let sender_before = token::Client::new(&env, &token).balance(&sender);
        client.tip(&sender, &creator, &token, &amount);
        let sender_after = token::Client::new(&env, &token).balance(&sender);

        TestResult::from_bool(sender_before - sender_after == amount)
    }
    quickcheck(prop as fn(i64) -> TestResult);
}

#[test]
fn qc_withdraw_clears_balance() {
    fn prop(amount: i64) -> TestResult {
        if amount <= 0 || amount > 1_000_000 {
            return TestResult::discard();
        }
        let amount = amount as i128;

        let (env, client, token, _token_admin) = setup();
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        token::StellarAssetClient::new(&env, &token).mint(&sender, &amount);
        client.tip(&sender, &creator, &token, &amount);

        let balance_before = client.get_withdrawable_balance(&creator, &token);
        client.withdraw(&creator, &token, &None);
        let balance_after = client.get_withdrawable_balance(&creator, &token);

        TestResult::from_bool(balance_before == amount && balance_after == 0)
    }
    quickcheck(prop as fn(i64) -> TestResult);
}

#[test]
fn qc_total_tips_monotonic() {
    fn prop(amounts: Vec<u16>) -> TestResult {
        if amounts.is_empty() || amounts.len() > 10 {
            return TestResult::discard();
        }

        let (env, client, token, _token_admin) = setup();
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        let total: i128 = amounts.iter().map(|&a| a as i128).sum();
        if total == 0 || total > 1_000_000 {
            return TestResult::discard();
        }

        token::StellarAssetClient::new(&env, &token).mint(&sender, &total);

        let mut expected_total = 0i128;
        for &amt in &amounts {
            let amt = amt as i128;
            if amt == 0 {
                continue;
            }
            client.tip(&sender, &creator, &token, &amt);
            expected_total += amt;
            let actual_total = client.get_total_tips(&creator, &token);
            if actual_total != expected_total {
                return TestResult::failed();
            }
        }

        TestResult::passed()
    }
    quickcheck(prop as fn(Vec<u16>) -> TestResult);
}
