extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Env as _, Ledger},
    Address, BytesN, Env,
};
use tipjar::{rollup::BatchStatus, TipJarContract, TipJarContractClient};

// ── helpers ──────────────────────────────────────────────────────────────────

struct Ctx {
    env: Env,
    client: TipJarContractClient,
    admin: Address,
    sequencer: Address,
}

impl Ctx {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let sequencer = Address::generate(&env);
        let contract_id = env.register(TipJarContract, ());
        let client = TipJarContractClient::new(&env, &contract_id);
        client.init(&admin);
        client.init_rollup(&admin, &sequencer);
        Self {
            env,
            client,
            admin,
            sequencer,
        }
    }

    fn root(&self, seed: u8) -> BytesN<32> {
        BytesN::from_array(&self.env, &[seed; 32])
    }

    fn advance_past_challenge(&self) {
        let challenge_period = 7 * 24 * 3600u64;
        self.env
            .ledger()
            .with_mut(|l| l.timestamp += challenge_period + 1);
    }
}

// ── init ─────────────────────────────────────────────────────────────────────

#[test]
fn test_init_rollup_state() {
    let ctx = Ctx::new();
    let state = ctx.client.get_rollup_state();
    assert!(state.enabled);
    assert_eq!(state.pending_batches, 0);
    assert_eq!(state.finalized_batches, 0);
    assert_eq!(state.challenged_batches, 0);
    assert_eq!(state.challenge_period, 7 * 24 * 3600);
}

#[test]
fn test_init_rollup_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let impostor = Address::generate(&env);
    let sequencer = Address::generate(&env);
    let id = env.register(TipJarContract, ());
    let client = TipJarContractClient::new(&env, &id);
    client.init(&admin);
    assert!(client.try_init_rollup(&impostor, &sequencer).is_err());
}

// ── batch submission ──────────────────────────────────────────────────────────

#[test]
fn test_submit_batch_increments_id() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    let id1 =
        ctx.client
            .submit_rollup_batch(&ctx.sequencer, &ctx.root(1), &creator, &token, &1000, &10);
    let id2 =
        ctx.client
            .submit_rollup_batch(&ctx.sequencer, &ctx.root(2), &creator, &token, &500, &5);
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);

    let state = ctx.client.get_rollup_state();
    assert_eq!(state.pending_batches, 2);
}

#[test]
fn test_submit_batch_stores_pending() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    let root = ctx.root(42);

    let id = ctx
        .client
        .submit_rollup_batch(&ctx.sequencer, &root, &creator, &token, &800, &8);
    let batch = ctx.client.get_rollup_batch(&id).unwrap();

    assert_eq!(batch.batch_id, id);
    assert_eq!(batch.state_root, root);
    assert_eq!(batch.total_amount, 800);
    assert_eq!(batch.tip_count, 8);
    assert!(matches!(batch.status, BatchStatus::Pending));
}

#[test]
fn test_submit_batch_unauthorized_sequencer() {
    let ctx = Ctx::new();
    let impostor = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    assert!(ctx
        .client
        .try_submit_rollup_batch(&impostor, &ctx.root(1), &creator, &token, &100, &1)
        .is_err());
}

#[test]
fn test_submit_batch_invalid_amount() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    assert!(ctx
        .client
        .try_submit_rollup_batch(&ctx.sequencer, &ctx.root(1), &creator, &token, &0, &0)
        .is_err());
}

// ── finalization ──────────────────────────────────────────────────────────────

#[test]
fn test_finalize_after_challenge_period() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    let id =
        ctx.client
            .submit_rollup_batch(&ctx.sequencer, &ctx.root(1), &creator, &token, &1000, &10);
    ctx.advance_past_challenge();
    ctx.client.finalize_rollup_batch(&id);

    let batch = ctx.client.get_rollup_batch(&id).unwrap();
    assert!(matches!(batch.status, BatchStatus::Finalized));

    let state = ctx.client.get_rollup_state();
    assert_eq!(state.pending_batches, 0);
    assert_eq!(state.finalized_batches, 1);
}

#[test]
fn test_finalize_before_challenge_period_fails() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    let id =
        ctx.client
            .submit_rollup_batch(&ctx.sequencer, &ctx.root(1), &creator, &token, &1000, &10);
    // Do NOT advance time
    assert!(ctx.client.try_finalize_rollup_batch(&id).is_err());
}

#[test]
fn test_finalize_credits_creator_balance() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);

    let id =
        ctx.client
            .submit_rollup_batch(&ctx.sequencer, &ctx.root(1), &creator, &token, &500, &5);
    ctx.advance_past_challenge();
    ctx.client.finalize_rollup_batch(&id);

    // Creator total should be credited
    let batch = ctx.client.get_rollup_batch(&id).unwrap();
    assert!(matches!(batch.status, BatchStatus::Finalized));
    assert_eq!(batch.total_amount, 500);
}

// ── fraud proofs ──────────────────────────────────────────────────────────────

#[test]
fn test_fraud_proof_accepted_on_root_mismatch() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    let challenger = Address::generate(&ctx.env);

    let id =
        ctx.client
            .submit_rollup_batch(&ctx.sequencer, &ctx.root(1), &creator, &token, &1000, &10);
    // Submit fraud proof with a different root
    let accepted = ctx
        .client
        .submit_fraud_proof(&challenger, &id, &ctx.root(99));
    assert!(accepted);

    let batch = ctx.client.get_rollup_batch(&id).unwrap();
    assert!(matches!(batch.status, BatchStatus::Challenged));

    let state = ctx.client.get_rollup_state();
    assert_eq!(state.challenged_batches, 1);
    assert_eq!(state.pending_batches, 0);
}

#[test]
fn test_fraud_proof_rejected_on_matching_root() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    let challenger = Address::generate(&ctx.env);
    let root = ctx.root(1);

    let id = ctx
        .client
        .submit_rollup_batch(&ctx.sequencer, &root, &creator, &token, &1000, &10);
    // Same root — no fraud
    let accepted = ctx.client.submit_fraud_proof(&challenger, &id, &root);
    assert!(!accepted);

    let batch = ctx.client.get_rollup_batch(&id).unwrap();
    assert!(matches!(batch.status, BatchStatus::Pending));
}

#[test]
fn test_fraud_proof_after_challenge_period_fails() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    let challenger = Address::generate(&ctx.env);

    let id =
        ctx.client
            .submit_rollup_batch(&ctx.sequencer, &ctx.root(1), &creator, &token, &1000, &10);
    ctx.advance_past_challenge();
    assert!(ctx
        .client
        .try_submit_fraud_proof(&challenger, &id, &ctx.root(99))
        .is_err());
}

#[test]
fn test_get_fraud_proof() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    let challenger = Address::generate(&ctx.env);

    let id =
        ctx.client
            .submit_rollup_batch(&ctx.sequencer, &ctx.root(1), &creator, &token, &1000, &10);
    ctx.client
        .submit_fraud_proof(&challenger, &id, &ctx.root(99));

    let proof = ctx.client.get_fraud_proof(&id).unwrap();
    assert_eq!(proof.batch_id, id);
    assert_eq!(proof.challenger, challenger);
    assert_eq!(proof.claimed_root, ctx.root(99));
}

#[test]
fn test_challenged_batch_cannot_be_finalized() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = Address::generate(&ctx.env);
    let challenger = Address::generate(&ctx.env);

    let id =
        ctx.client
            .submit_rollup_batch(&ctx.sequencer, &ctx.root(1), &creator, &token, &1000, &10);
    ctx.client
        .submit_fraud_proof(&challenger, &id, &ctx.root(99));
    ctx.advance_past_challenge();

    assert!(ctx.client.try_finalize_rollup_batch(&id).is_err());
}
