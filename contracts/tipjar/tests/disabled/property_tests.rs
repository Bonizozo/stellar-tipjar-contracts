/// Property-based tests for the TipJar contract using proptest.
///
/// Each `proptest!` block states an invariant that must hold for all generated
/// inputs.  The test runner shrinks failing cases automatically.
use proptest::prelude::*;
use soroban_sdk::{testutils::Address as _, token, Address, Env};
use tipjar::{TipJarContract, TipJarContractClient};

// ── test helpers ─────────────────────────────────────────────────────────────

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

fn mint(env: &Env, token: &Address, to: &Address, amount: i128) {
    token::StellarAssetClient::new(env, token).mint(to, &amount);
}

// ── properties ────────────────────────────────────────────────────────────────

proptest! {
    /// Property: after a single tip, the creator's withdrawable balance equals
    /// the tipped amount.
    #[test]
    fn prop_tip_balance_invariant(amount in 1i128..1_000_000_000i128) {
        let (env, client, token, _) = setup();
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        mint(&env, &token, &sender, amount);
        client.tip(&sender, &creator, &token, &amount);

        prop_assert_eq!(client.get_withdrawable_balance(&creator, &token), amount);
        prop_assert_eq!(client.get_total_tips(&creator, &token), amount);
    }

    /// Property: multiple tips accumulate correctly — total equals the sum of
    /// all individual amounts.
    #[test]
    fn prop_multiple_tips_accumulate(
        amounts in prop::collection::vec(1i128..1_000_000i128, 1..10usize)
    ) {
        let (env, client, token, _) = setup();
        let creator = Address::generate(&env);
        let expected_total: i128 = amounts.iter().sum();

        for amount in &amounts {
            let sender = Address::generate(&env);
            mint(&env, &token, &sender, *amount);
            client.tip(&sender, &creator, &token, amount);
        }

        prop_assert_eq!(client.get_total_tips(&creator, &token), expected_total);
        prop_assert_eq!(client.get_withdrawable_balance(&creator, &token), expected_total);
    }

    /// Property: after withdrawal the balance is zero but the historical total
    /// is unchanged.
    #[test]
    fn prop_withdraw_clears_balance(amount in 1i128..1_000_000_000i128) {
        let (env, client, token, _) = setup();
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        mint(&env, &token, &sender, amount);
        client.tip(&sender, &creator, &token, &amount);
        client.withdraw(&creator, &token);

        prop_assert_eq!(client.get_withdrawable_balance(&creator, &token), 0);
        prop_assert_eq!(client.get_total_tips(&creator, &token), amount);
    }

    /// Property: tipping with a non-positive amount must panic.
    #[test]
    fn prop_no_negative_or_zero_amounts(amount in i128::MIN..=0i128) {
        let (env, client, token, _) = setup();
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        // Soroban panics are caught as Err by catch_unwind in std tests.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.tip(&sender, &creator, &token, &amount);
        }));
        prop_assert!(result.is_err(), "expected panic for amount={}", amount);
    }

    /// Property: tip_with_memo stores exactly one record per call and the
    /// returned list length equals the number of tips sent (up to the query
    /// limit of 50).
    #[test]
    fn prop_memo_tip_count(
        amounts in prop::collection::vec(1i128..100_000i128, 1..10usize)
    ) {
        let (env, client, token, _) = setup();
        let creator = Address::generate(&env);
        let n = amounts.len() as u32;

        for amount in &amounts {
            let sender = Address::generate(&env);
            mint(&env, &token, &sender, *amount);
            client.tip_with_memo(&sender, &creator, &token, amount, &None);
        }

        let tips = client.get_tips_with_memos(&creator, &n);
        prop_assert_eq!(tips.len(), n);
    }

    /// Property: memo length exactly at the limit (200 chars) is accepted;
    /// one character over must panic.
    #[test]
    fn prop_memo_length_boundary(extra in 0u32..=1u32) {
        let (env, client, token, _) = setup();
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        mint(&env, &token, &sender, 1_000);

        // Build a memo of length 200 + extra.
        let memo_str: String = "a".repeat((200 + extra) as usize);
        let memo = soroban_sdk::String::from_str(&env, &memo_str);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.tip_with_memo(&sender, &creator, &token, &500, &Some(memo));
        }));

        if extra == 0 {
            prop_assert!(result.is_ok(), "200-char memo should be accepted");
        } else {
            prop_assert!(result.is_err(), "201-char memo should be rejected");
        }
    }

    /// Property: the sum of all per-creator balances across independent
    /// creators equals the total amount transferred into the contract.
    #[test]
    fn prop_total_conservation(
        amounts in prop::collection::vec(1i128..100_000i128, 2..6usize)
    ) {
        let (env, client, token, _) = setup();
        let expected_total: i128 = amounts.iter().sum();

        let mut creators = Vec::new();
        for amount in &amounts {
            let sender = Address::generate(&env);
            let creator = Address::generate(&env);
            mint(&env, &token, &sender, *amount);
            client.tip(&sender, &creator, &token, amount);
            creators.push((creator, *amount));
        }

        let actual_total: i128 = creators
            .iter()
            .map(|(c, _)| client.get_withdrawable_balance(c, &token))
            .sum();

        prop_assert_eq!(actual_total, expected_total);
    }
}
