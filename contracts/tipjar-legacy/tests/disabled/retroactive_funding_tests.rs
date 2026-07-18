#![cfg(test)]

extern crate std;

use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env};
use tipjar::{
    retroactive_funding::{EvalCriteria, RetroRoundStatus},
    TipJarContract, TipJarContractClient,
};

// ── helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, TipJarContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TipJarContract);
    let client = TipJarContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin);
    client.add_token(&admin, &token);

    (env, client, admin, token)
}

fn mint(env: &Env, token: &Address, to: &Address, amount: i128) {
    soroban_sdk::token::StellarAssetClient::new(env, token).mint(to, &amount);
}

fn advance_time(env: &Env, seconds: u64) {
    env.ledger().with_mut(|l| l.timestamp += seconds);
}

fn equal_criteria() -> EvalCriteria {
    EvalCriteria {
        tips_weight_bps: 3_334,
        tip_count_weight_bps: 3_333,
        vote_weight_bps: 3_333,
    }
}

fn vote_only_criteria() -> EvalCriteria {
    EvalCriteria {
        tips_weight_bps: 0,
        tip_count_weight_bps: 0,
        vote_weight_bps: 10_000,
    }
}

// ── create_round ──────────────────────────────────────────────────────────────

#[test]
fn test_create_round_basic() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 1_000);

    let now = env.ledger().timestamp();
    let round_id = client.retro_create_round(
        &admin, &token, &1_000, &now, &(now + 3600), &equal_criteria(),
    );
    assert_eq!(round_id, 0);

    let round = client.retro_get_round(&round_id);
    assert_eq!(round.reward_pool, 1_000);
    assert_eq!(round.status, RetroRoundStatus::Active);
    assert_eq!(round.total_votes, 0);
}

#[test]
fn test_create_multiple_rounds() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 3_000);

    let now = env.ledger().timestamp();
    let r0 = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 3600), &equal_criteria());
    let r1 = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 7200), &equal_criteria());
    let r2 = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 86400), &equal_criteria());

    assert_eq!(r0, 0);
    assert_eq!(r1, 1);
    assert_eq!(r2, 2);
}

#[test]
#[should_panic]
fn test_create_round_invalid_criteria() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 1_000);

    let now = env.ledger().timestamp();
    // weights sum to 9_000, not 10_000 — should panic
    client.retro_create_round(
        &admin,
        &token,
        &1_000,
        &now,
        &(now + 3600),
        &EvalCriteria {
            tips_weight_bps: 3_000,
            tip_count_weight_bps: 3_000,
            vote_weight_bps: 3_000,
        },
    );
}

// ── nominate_creator ──────────────────────────────────────────────────────────

#[test]
fn test_nominate_creator() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 1_000);

    let now = env.ledger().timestamp();
    let round_id = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 3600), &equal_criteria());

    let creator = Address::generate(&env);
    client.retro_nominate_creator(&admin, &round_id, &creator);

    let creators = client.retro_get_round_creators(&round_id);
    assert_eq!(creators.len(), 1);
    assert_eq!(creators.get(0).unwrap(), creator);
}

#[test]
fn test_nominate_creator_idempotent() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 1_000);

    let now = env.ledger().timestamp();
    let round_id = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 3600), &equal_criteria());

    let creator = Address::generate(&env);
    client.retro_nominate_creator(&admin, &round_id, &creator);
    client.retro_nominate_creator(&admin, &round_id, &creator); // duplicate — no-op

    let creators = client.retro_get_round_creators(&round_id);
    assert_eq!(creators.len(), 1);
}

// ── cast_vote ─────────────────────────────────────────────────────────────────

#[test]
fn test_cast_vote_basic() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 1_000);

    let now = env.ledger().timestamp();
    let round_id = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 3600), &vote_only_criteria());

    let creator = Address::generate(&env);
    client.retro_nominate_creator(&admin, &round_id, &creator);

    let voter = Address::generate(&env);
    client.retro_cast_vote(&voter, &round_id, &creator);

    // voter with no tip history gets weight = 1
    let votes = client.retro_get_creator_votes(&round_id, &creator);
    assert_eq!(votes, 1);
    assert!(client.retro_has_voted(&round_id, &voter));
}

#[test]
#[should_panic]
fn test_cast_vote_twice_panics() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 1_000);

    let now = env.ledger().timestamp();
    let round_id = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 3600), &vote_only_criteria());

    let creator = Address::generate(&env);
    client.retro_nominate_creator(&admin, &round_id, &creator);

    let voter = Address::generate(&env);
    client.retro_cast_vote(&voter, &round_id, &creator);
    client.retro_cast_vote(&voter, &round_id, &creator); // should panic
}

#[test]
#[should_panic]
fn test_cast_vote_unnominated_creator_panics() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 1_000);

    let now = env.ledger().timestamp();
    let round_id = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 3600), &vote_only_criteria());

    let creator = Address::generate(&env);
    let voter = Address::generate(&env);
    // creator not nominated — should panic
    client.retro_cast_vote(&voter, &round_id, &creator);
}

#[test]
fn test_multiple_voters() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 1_000);

    let now = env.ledger().timestamp();
    let round_id = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 3600), &vote_only_criteria());

    let creator = Address::generate(&env);
    client.retro_nominate_creator(&admin, &round_id, &creator);

    for _ in 0..5 {
        let voter = Address::generate(&env);
        client.retro_cast_vote(&voter, &round_id, &creator);
    }

    let votes = client.retro_get_creator_votes(&round_id, &creator);
    assert_eq!(votes, 5); // 5 voters × weight 1 each
}

// ── finalize_round ────────────────────────────────────────────────────────────

#[test]
fn test_finalize_round() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 1_000);

    let now = env.ledger().timestamp();
    let round_id = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 3600), &vote_only_criteria());

    advance_time(&env, 3601);
    client.retro_finalize_round(&admin, &round_id);

    let round = client.retro_get_round(&round_id);
    assert_eq!(round.status, RetroRoundStatus::Finalized);
}

#[test]
#[should_panic]
fn test_finalize_before_end_panics() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 1_000);

    let now = env.ledger().timestamp();
    let round_id = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 3600), &vote_only_criteria());

    // voting window still open — should panic
    client.retro_finalize_round(&admin, &round_id);
}

// ── compute_impact_score ──────────────────────────────────────────────────────

#[test]
fn test_compute_impact_score_vote_only() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 1_000);

    let now = env.ledger().timestamp();
    let round_id = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 3600), &vote_only_criteria());

    let creator = Address::generate(&env);
    client.retro_nominate_creator(&admin, &round_id, &creator);

    // 3 voters, each weight 1
    for _ in 0..3 {
        let voter = Address::generate(&env);
        client.retro_cast_vote(&voter, &round_id, &creator);
    }

    let score = client.retro_compute_impact_score(&round_id, &creator);
    // score = (0 * 0 + 0 * 0 + 3 * 1_000_000 * 10_000) / 10_000 = 3_000_000
    assert_eq!(score, 3_000_000);
}

// ── claim_reward ──────────────────────────────────────────────────────────────

#[test]
fn test_claim_reward_single_creator() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 1_000);

    let now = env.ledger().timestamp();
    let round_id = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 3600), &vote_only_criteria());

    let creator = Address::generate(&env);
    client.retro_nominate_creator(&admin, &round_id, &creator);

    let voter = Address::generate(&env);
    client.retro_cast_vote(&voter, &round_id, &creator);

    advance_time(&env, 3601);
    client.retro_finalize_round(&admin, &round_id);

    let reward = client.retro_claim_reward(&creator, &round_id);
    // Only creator → gets 100% of pool
    assert_eq!(reward, 1_000);
}

#[test]
fn test_claim_reward_two_creators_equal_votes() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 1_000);

    let now = env.ledger().timestamp();
    let round_id = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 3600), &vote_only_criteria());

    let creator_a = Address::generate(&env);
    let creator_b = Address::generate(&env);
    client.retro_nominate_creator(&admin, &round_id, &creator_a);
    client.retro_nominate_creator(&admin, &round_id, &creator_b);

    // 1 vote each
    client.retro_cast_vote(&Address::generate(&env), &round_id, &creator_a);
    client.retro_cast_vote(&Address::generate(&env), &round_id, &creator_b);

    advance_time(&env, 3601);
    client.retro_finalize_round(&admin, &round_id);

    let reward_a = client.retro_claim_reward(&creator_a, &round_id);
    let reward_b = client.retro_claim_reward(&creator_b, &round_id);

    // Equal scores → equal split (integer division may leave 1 stroop in contract)
    assert_eq!(reward_a, 500);
    assert_eq!(reward_b, 500);
}

#[test]
#[should_panic]
fn test_claim_reward_twice_panics() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 1_000);

    let now = env.ledger().timestamp();
    let round_id = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 3600), &vote_only_criteria());

    let creator = Address::generate(&env);
    client.retro_nominate_creator(&admin, &round_id, &creator);
    client.retro_cast_vote(&Address::generate(&env), &round_id, &creator);

    advance_time(&env, 3601);
    client.retro_finalize_round(&admin, &round_id);

    client.retro_claim_reward(&creator, &round_id);
    client.retro_claim_reward(&creator, &round_id); // should panic
}

#[test]
#[should_panic]
fn test_claim_reward_before_finalize_panics() {
    let (env, client, admin, token) = setup();
    mint(&env, &token, &admin, 1_000);

    let now = env.ledger().timestamp();
    let round_id = client.retro_create_round(&admin, &token, &1_000, &now, &(now + 3600), &vote_only_criteria());

    let creator = Address::generate(&env);
    client.retro_nominate_creator(&admin, &round_id, &creator);
    client.retro_cast_vote(&Address::generate(&env), &round_id, &creator);

    // Not finalized yet — should panic
    client.retro_claim_reward(&creator, &round_id);
}
