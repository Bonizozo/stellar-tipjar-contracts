#![cfg(test)]

extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env};
use tipjar::{
    quadratic_funding::{FundingRound, RoundStatus},
    TipJarContract, TipJarContractClient,
};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Returns (env, client, admin, token, contract_id).
fn setup() -> (
    Env,
    TipJarContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TipJarContract);
    let client = TipJarContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin);
    client.add_token(&admin, &token);

    (env, client, admin, token, contract_id)
}

fn mint(env: &Env, token: &Address, to: &Address, amount: i128) {
    soroban_sdk::token::StellarAssetClient::new(env, token).mint(to, &amount);
}

fn advance_time(env: &Env, seconds: u64) {
    env.ledger().with_mut(|l| l.timestamp += seconds);
}

// ── create_round ──────────────────────────────────────────────────────────────

#[test]
fn test_create_round_basic() {
    let (env, client, admin, token, _) = setup();
    mint(&env, &token, &admin, 1_000);

    let round_id = client.qf_create_round(&admin, &token, &1_000, &3600);
    assert_eq!(round_id, 0);

    let round = client.qf_get_round(&round_id).unwrap();
    assert_eq!(round.matching_pool, 1_000);
    assert_eq!(round.status, RoundStatus::Active);
    assert_eq!(round.total_contributions, 0);
}

#[test]
fn test_create_multiple_rounds() {
    let (env, client, admin, token, _) = setup();
    mint(&env, &token, &admin, 3_000);

    let r0 = client.qf_create_round(&admin, &token, &1_000, &3600);
    let r1 = client.qf_create_round(&admin, &token, &1_000, &7200);
    let r2 = client.qf_create_round(&admin, &token, &1_000, &86400);

    assert_eq!(r0, 0);
    assert_eq!(r1, 1);
    assert_eq!(r2, 2);
}

// ── contribute ────────────────────────────────────────────────────────────────

#[test]
fn test_contribute_basic() {
    let (env, client, admin, token, _) = setup();
    mint(&env, &token, &admin, 1_000);

    let round_id = client.qf_create_round(&admin, &token, &1_000, &3600);

    let contributor = Address::generate(&env);
    let project = Address::generate(&env);
    mint(&env, &token, &contributor, 500);

    client.qf_contribute(&contributor, &round_id, &project, &100);

    let c = client
        .qf_get_contribution(&round_id, &project, &contributor)
        .unwrap();
    assert_eq!(c.amount, 100);
    assert_eq!(c.contributor, contributor);
    assert_eq!(c.project, project);

    let round = client.qf_get_round(&round_id).unwrap();
    assert_eq!(round.total_contributions, 100);
}

#[test]
fn test_sybil_resistance_one_contribution_per_address() {
    let (env, client, admin, token, _) = setup();
    mint(&env, &token, &admin, 1_000);

    let round_id = client.qf_create_round(&admin, &token, &1_000, &3600);

    let contributor = Address::generate(&env);
    let project = Address::generate(&env);
    mint(&env, &token, &contributor, 500);

    client.qf_contribute(&contributor, &round_id, &project, &100);

    // Second contribution from same address to same project must fail.
    let result = client.try_qf_contribute(&contributor, &round_id, &project, &50);
    assert!(result.is_err());
}

#[test]
fn test_multiple_contributors_same_project() {
    let (env, client, admin, token, _) = setup();
    mint(&env, &token, &admin, 1_000);

    let round_id = client.qf_create_round(&admin, &token, &1_000, &3600);
    let project = Address::generate(&env);

    for _ in 0..5u32 {
        let c = Address::generate(&env);
        mint(&env, &token, &c, 100);
        client.qf_contribute(&c, &round_id, &project, &100);
    }

    let round = client.qf_get_round(&round_id).unwrap();
    assert_eq!(round.total_contributions, 500);
}

#[test]
fn test_contribute_after_round_ends_fails() {
    let (env, client, admin, token, _) = setup();
    mint(&env, &token, &admin, 1_000);

    let round_id = client.qf_create_round(&admin, &token, &1_000, &3600);
    advance_time(&env, 3601);

    let contributor = Address::generate(&env);
    let project = Address::generate(&env);
    mint(&env, &token, &contributor, 100);

    let result = client.try_qf_contribute(&contributor, &round_id, &project, &100);
    assert!(result.is_err());
}

// ── finalize_round ────────────────────────────────────────────────────────────

#[test]
fn test_finalize_round() {
    let (env, client, admin, token, _) = setup();
    mint(&env, &token, &admin, 1_000);

    let round_id = client.qf_create_round(&admin, &token, &1_000, &3600);
    advance_time(&env, 3601);

    client.qf_finalize_round(&admin, &round_id);

    let round = client.qf_get_round(&round_id).unwrap();
    assert_eq!(round.status, RoundStatus::Finalized);
}

#[test]
fn test_finalize_before_end_fails() {
    let (env, client, admin, token, _) = setup();
    mint(&env, &token, &admin, 1_000);

    let round_id = client.qf_create_round(&admin, &token, &1_000, &3600);
    // Do NOT advance time.

    let result = client.try_qf_finalize_round(&admin, &round_id);
    assert!(result.is_err());
}

#[test]
fn test_finalize_unauthorized_fails() {
    let (env, client, admin, token, _) = setup();
    mint(&env, &token, &admin, 1_000);

    let round_id = client.qf_create_round(&admin, &token, &1_000, &3600);
    advance_time(&env, 3601);

    let other = Address::generate(&env);
    let result = client.try_qf_finalize_round(&other, &round_id);
    assert!(result.is_err());
}

// ── distribute_matching ───────────────────────────────────────────────────────

#[test]
fn test_distribute_matching_two_projects() {
    let (env, client, admin, token, contract_id) = setup();
    let matching_pool: i128 = 10_000;
    mint(&env, &token, &admin, matching_pool);

    let round_id = client.qf_create_round(&admin, &token, &matching_pool, &3600);

    let project_a = Address::generate(&env);
    let project_b = Address::generate(&env);

    // 4 contributors to A, 1 to B — QF should favour A heavily.
    for _ in 0..4u32 {
        let c = Address::generate(&env);
        mint(&env, &token, &c, 100);
        client.qf_contribute(&c, &round_id, &project_a, &100);
    }
    let lone = Address::generate(&env);
    mint(&env, &token, &lone, 100);
    client.qf_contribute(&lone, &round_id, &project_b, &100);

    advance_time(&env, 3601);
    client.qf_finalize_round(&admin, &round_id);
    client.qf_distribute_matching(&admin, &round_id);

    let round = client.qf_get_round(&round_id).unwrap();
    assert_eq!(round.status, RoundStatus::Distributed);

    // Project A should have received more matching than B.
    let token_client = soroban_sdk::token::TokenClient::new(&env, &token);
    let bal_a = token_client.balance(&project_a);
    let bal_b = token_client.balance(&project_b);
    assert!(
        bal_a > bal_b,
        "project_a should receive more matching: {bal_a} vs {bal_b}"
    );

    // Total distributed = matching_pool (contributions are returned to contributors).
    assert_eq!(bal_a + bal_b, matching_pool);
}

#[test]
fn test_distribute_returns_contributions_to_donors() {
    let (env, client, admin, token, _) = setup();
    mint(&env, &token, &admin, 1_000);

    let round_id = client.qf_create_round(&admin, &token, &1_000, &3600);

    let contributor = Address::generate(&env);
    let project = Address::generate(&env);
    mint(&env, &token, &contributor, 200);

    client.qf_contribute(&contributor, &round_id, &project, &200);

    advance_time(&env, 3601);
    client.qf_finalize_round(&admin, &round_id);
    client.qf_distribute_matching(&admin, &round_id);

    // Contributor should get their 200 back.
    let token_client = soroban_sdk::token::TokenClient::new(&env, &token);
    assert_eq!(token_client.balance(&contributor), 200);
}

#[test]
fn test_distribute_twice_fails() {
    let (env, client, admin, token, _) = setup();
    mint(&env, &token, &admin, 1_000);

    let round_id = client.qf_create_round(&admin, &token, &1_000, &3600);
    advance_time(&env, 3601);
    client.qf_finalize_round(&admin, &round_id);
    client.qf_distribute_matching(&admin, &round_id);

    let result = client.try_qf_distribute_matching(&admin, &round_id);
    assert!(result.is_err());
}

// ── match estimate ────────────────────────────────────────────────────────────

#[test]
fn test_match_estimate_nonzero_for_active_project() {
    let (env, client, admin, token, _) = setup();
    mint(&env, &token, &admin, 10_000);

    let round_id = client.qf_create_round(&admin, &token, &10_000, &3600);

    let project = Address::generate(&env);
    let contributor = Address::generate(&env);
    mint(&env, &token, &contributor, 500);
    client.qf_contribute(&contributor, &round_id, &project, &500);

    let estimate = client.qf_get_match_estimate(&round_id, &project);
    assert_eq!(estimate, 10_000); // Only project → gets 100% of pool.
}

#[test]
fn test_match_estimate_zero_for_unknown_project() {
    let (env, client, admin, token, _) = setup();
    mint(&env, &token, &admin, 1_000);

    let round_id = client.qf_create_round(&admin, &token, &1_000, &3600);
    let unknown = Address::generate(&env);

    assert_eq!(client.qf_get_match_estimate(&round_id, &unknown), 0);
}

// ── quadratic weighting ───────────────────────────────────────────────────────

#[test]
fn test_quadratic_weighting_many_small_beats_one_large() {
    // 9 contributors of 1 each vs 1 contributor of 9.
    // QF weight for 9×1: (9 × √1)² = 81
    // QF weight for 1×9: (1 × √9)² = 9
    // So the many-small project should get ~9× more matching.
    let (env, client, admin, token, _) = setup();
    let matching_pool: i128 = 9_000;
    mint(&env, &token, &admin, matching_pool);

    let round_id = client.qf_create_round(&admin, &token, &matching_pool, &3600);

    let project_many = Address::generate(&env);
    let project_one = Address::generate(&env);

    for _ in 0..9u32 {
        let c = Address::generate(&env);
        mint(&env, &token, &c, 1);
        client.qf_contribute(&c, &round_id, &project_many, &1);
    }

    let big = Address::generate(&env);
    mint(&env, &token, &big, 9);
    client.qf_contribute(&big, &round_id, &project_one, &9);

    advance_time(&env, 3601);
    client.qf_finalize_round(&admin, &round_id);
    client.qf_distribute_matching(&admin, &round_id);

    let token_client = soroban_sdk::token::TokenClient::new(&env, &token);
    let bal_many = token_client.balance(&project_many);
    let bal_one = token_client.balance(&project_one);

    // Many-small should receive significantly more.
    assert!(
        bal_many > bal_one * 5,
        "many-small ({bal_many}) should dominate one-large ({bal_one})"
    );
}
