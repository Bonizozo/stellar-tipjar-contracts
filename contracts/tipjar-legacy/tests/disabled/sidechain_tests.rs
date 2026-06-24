extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Env as _},
    Address, BytesN, Env,
};
use tipjar::{TipJarContract, TipJarContractClient};

// ── helpers ──────────────────────────────────────────────────────────────────

struct Ctx {
    env: Env,
    client: TipJarContractClient,
    admin: Address,
    operator: Address,
}

impl Ctx {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let operator = Address::generate(&env);
        let contract_id = env.register(TipJarContract, ());
        let client = TipJarContractClient::new(&env, &contract_id);

        client.init(&admin);
        client.init_sidechain(&admin, &operator);

        Self {
            env,
            client,
            admin,
            operator,
        }
    }

    fn state_root(&self, seed: u8) -> BytesN<32> {
        BytesN::from_array(&self.env, &[seed; 32])
    }

    fn token(&self) -> Address {
        Address::generate(&self.env)
    }
}

// ── init ─────────────────────────────────────────────────────────────────────

#[test]
fn test_init_sidechain_enables_feature() {
    let ctx = Ctx::new();
    let state = ctx.client.get_sidechain_state();
    assert!(state.enabled);
    assert_eq!(state.latest_checkpoint, 0);
    assert_eq!(state.total_checkpoints, 0);
    assert_eq!(state.total_finalized_volume, 0);
}

#[test]
fn test_init_sidechain_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let impostor = Address::generate(&env);
    let operator = Address::generate(&env);
    let contract_id = env.register(TipJarContract, ());
    let client = TipJarContractClient::new(&env, &contract_id);
    client.init(&admin);

    let result = client.try_init_sidechain(&impostor, &operator);
    assert!(result.is_err());
}

// ── checkpoint submission ─────────────────────────────────────────────────────

#[test]
fn test_submit_checkpoint_increments_seq() {
    let ctx = Ctx::new();

    let seq1 = ctx
        .client
        .submit_checkpoint(&ctx.operator, &ctx.state_root(1), &10, &1000);
    assert_eq!(seq1, 1);

    let seq2 = ctx
        .client
        .submit_checkpoint(&ctx.operator, &ctx.state_root(2), &5, &500);
    assert_eq!(seq2, 2);

    let state = ctx.client.get_sidechain_state();
    assert_eq!(state.latest_checkpoint, 2);
}

#[test]
fn test_submit_checkpoint_stores_data() {
    let ctx = Ctx::new();
    let root = ctx.state_root(42);

    let seq = ctx
        .client
        .submit_checkpoint(&ctx.operator, &root, &20, &2000);

    let cp = ctx.client.get_checkpoint(&seq).unwrap();
    assert_eq!(cp.seq, seq);
    assert_eq!(cp.state_root, root);
    assert_eq!(cp.tip_count, 20);
    assert_eq!(cp.total_volume, 2000);
    assert!(!cp.finalized);
}

#[test]
fn test_submit_checkpoint_unauthorized() {
    let ctx = Ctx::new();
    let impostor = Address::generate(&ctx.env);

    let result = ctx
        .client
        .try_submit_checkpoint(&impostor, &ctx.state_root(1), &10, &1000);
    assert!(result.is_err());
}

// ── checkpoint finalization ───────────────────────────────────────────────────

#[test]
fn test_finalize_checkpoint() {
    let ctx = Ctx::new();
    let seq = ctx
        .client
        .submit_checkpoint(&ctx.operator, &ctx.state_root(1), &10, &1000);

    ctx.client.finalize_checkpoint(&ctx.operator, &seq);

    let cp = ctx.client.get_checkpoint(&seq).unwrap();
    assert!(cp.finalized);
}

#[test]
fn test_finalize_checkpoint_updates_state() {
    let ctx = Ctx::new();
    let seq = ctx
        .client
        .submit_checkpoint(&ctx.operator, &ctx.state_root(1), &10, &1000);
    ctx.client.finalize_checkpoint(&ctx.operator, &seq);

    let state = ctx.client.get_sidechain_state();
    assert_eq!(state.total_checkpoints, 1);
    assert_eq!(state.total_finalized_volume, 1000);
}

// ── tip batch recording and settlement ───────────────────────────────────────

#[test]
fn test_record_and_settle_batch() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = ctx.token();

    let seq = ctx
        .client
        .submit_checkpoint(&ctx.operator, &ctx.state_root(1), &5, &500);
    ctx.client.finalize_checkpoint(&ctx.operator, &seq);

    let batch_id = ctx
        .client
        .record_tip_batch(&ctx.operator, &creator, &token, &500, &5, &seq);
    assert_eq!(batch_id, 1);

    ctx.client.finalize_tips(&batch_id);

    let finalized = ctx.client.get_sidechain_finalized_total(&creator, &token);
    assert_eq!(finalized, 500);
}

#[test]
fn test_settle_batch_credits_creator_balance() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = ctx.token();

    let seq = ctx
        .client
        .submit_checkpoint(&ctx.operator, &ctx.state_root(1), &3, &300);
    ctx.client.finalize_checkpoint(&ctx.operator, &seq);

    let batch_id = ctx
        .client
        .record_tip_batch(&ctx.operator, &creator, &token, &300, &3, &seq);
    ctx.client.finalize_tips(&batch_id);

    // Creator balance should be credited
    let finalized = ctx.client.get_sidechain_finalized_total(&creator, &token);
    assert_eq!(finalized, 300);
}

#[test]
fn test_settle_batch_requires_finalized_checkpoint() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = ctx.token();

    // Submit but do NOT finalize the checkpoint
    let seq = ctx
        .client
        .submit_checkpoint(&ctx.operator, &ctx.state_root(1), &5, &500);

    let batch_id = ctx
        .client
        .record_tip_batch(&ctx.operator, &creator, &token, &500, &5, &seq);

    // Should panic because checkpoint is not finalized
    let result = ctx.client.try_finalize_tips(&batch_id);
    assert!(result.is_err());
}

#[test]
fn test_record_batch_invalid_amount() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = ctx.token();

    let seq = ctx
        .client
        .submit_checkpoint(&ctx.operator, &ctx.state_root(1), &0, &0);
    ctx.client.finalize_checkpoint(&ctx.operator, &seq);

    let result = ctx
        .client
        .try_record_tip_batch(&ctx.operator, &creator, &token, &0, &0, &seq);
    assert!(result.is_err());
}

// ── multiple batches ──────────────────────────────────────────────────────────

#[test]
fn test_multiple_batches_accumulate() {
    let ctx = Ctx::new();
    let creator = Address::generate(&ctx.env);
    let token = ctx.token();

    let seq = ctx
        .client
        .submit_checkpoint(&ctx.operator, &ctx.state_root(1), &10, &1000);
    ctx.client.finalize_checkpoint(&ctx.operator, &seq);

    let b1 = ctx
        .client
        .record_tip_batch(&ctx.operator, &creator, &token, &400, &4, &seq);
    let b2 = ctx
        .client
        .record_tip_batch(&ctx.operator, &creator, &token, &600, &6, &seq);

    ctx.client.finalize_tips(&b1);
    ctx.client.finalize_tips(&b2);

    let total = ctx.client.get_sidechain_finalized_total(&creator, &token);
    assert_eq!(total, 1000);
}

#[test]
fn test_multiple_checkpoints_state() {
    let ctx = Ctx::new();

    let s1 = ctx
        .client
        .submit_checkpoint(&ctx.operator, &ctx.state_root(1), &5, &500);
    ctx.client.finalize_checkpoint(&ctx.operator, &s1);

    let s2 = ctx
        .client
        .submit_checkpoint(&ctx.operator, &ctx.state_root(2), &10, &1000);
    ctx.client.finalize_checkpoint(&ctx.operator, &s2);

    let state = ctx.client.get_sidechain_state();
    assert_eq!(state.total_checkpoints, 2);
    assert_eq!(state.total_finalized_volume, 1500);
    assert_eq!(state.latest_checkpoint, 2);
}
