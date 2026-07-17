use soroban_sdk::{
    testutils::Address as _,
    token, Address, BytesN, Env,
};
use tipjar::{
    privacy::{commitment, CommitmentOpening, PrivateTip},
    TipJarContract, TipJarContractClient, TipJarError,
};

// ── helpers ──────────────────────────────────────────────────────────────────

struct PrivacyCtx {
    env: Env,
    client: TipJarContractClient,
    token: Address,
    token_admin: Address,
}

impl PrivacyCtx {
    fn new() -> Self {
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

        Self { env, client, token, token_admin }
    }

    fn mint(&self, to: &Address, amount: i128) {
        token::StellarAssetClient::new(&self.env, &self.token).mint(to, &amount);
    }

    fn bytes32(&self, seed: u8) -> BytesN<32> {
        BytesN::from_array(&self.env, &[seed; 32])
    }

    fn make_tip(&self, creator: &Address, amount: i128, bf_seed: u8, null_seed: u8) -> (PrivateTip, CommitmentOpening) {
        let blinding_factor = self.bytes32(bf_seed);
        let commitment = commitment::compute_commitment(&self.env, creator, amount, &blinding_factor);
        let tip = PrivateTip {
            commitment,
            nullifier: self.bytes32(null_seed),
        };
        let opening = CommitmentOpening {
            creator: creator.clone(),
            amount,
            blinding_factor,
        };
        (tip, opening)
    }
}

// ── commitment tests ──────────────────────────────────────────────────────────

#[test]
fn test_commitment_verify_correct_opening() {
    let ctx = PrivacyCtx::new();
    let creator = Address::generate(&ctx.env);
    let bf = ctx.bytes32(1);
    let c = commitment::compute_commitment(&ctx.env, &creator, 500, &bf);
    let opening = CommitmentOpening { creator, amount: 500, blinding_factor: bf };
    assert!(commitment::verify_opening(&ctx.env, &c, &opening));
}

#[test]
fn test_commitment_wrong_amount_fails() {
    let ctx = PrivacyCtx::new();
    let creator = Address::generate(&ctx.env);
    let bf = ctx.bytes32(2);
    let c = commitment::compute_commitment(&ctx.env, &creator, 500, &bf);
    let opening = CommitmentOpening { creator, amount: 999, blinding_factor: bf };
    assert!(!commitment::verify_opening(&ctx.env, &c, &opening));
}

#[test]
fn test_commitment_wrong_blinding_factor_fails() {
    let ctx = PrivacyCtx::new();
    let creator = Address::generate(&ctx.env);
    let c = commitment::compute_commitment(&ctx.env, &creator, 100, &ctx.bytes32(3));
    let opening = CommitmentOpening { creator, amount: 100, blinding_factor: ctx.bytes32(4) };
    assert!(!commitment::verify_opening(&ctx.env, &c, &opening));
}

// ── private_tip / private_withdraw flow ──────────────────────────────────────

#[test]
fn test_private_tip_and_withdraw_happy_path() {
    let ctx = PrivacyCtx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 300);

    let (tip, opening) = ctx.make_tip(&creator, 300, 10, 20);
    ctx.client.private_tip(&sender, &ctx.token, &tip, &300).unwrap();

    // Creator withdraws by revealing the opening.
    ctx.client.private_withdraw(&tip, &opening).unwrap();

    assert_eq!(
        token::Client::new(&ctx.env, &ctx.token).balance(&creator),
        300
    );
}

#[test]
fn test_private_withdraw_double_spend_rejected() {
    let ctx = PrivacyCtx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 300);

    let (tip, opening) = ctx.make_tip(&creator, 300, 11, 21);
    ctx.client.private_tip(&sender, &ctx.token, &tip, &300).unwrap();
    ctx.client.private_withdraw(&tip, &opening).unwrap();

    // Second withdrawal with same nullifier must fail.
    let result = ctx.client.try_private_withdraw(&tip, &opening);
    assert!(result.is_err());
}

#[test]
fn test_private_withdraw_wrong_opening_rejected() {
    let ctx = PrivacyCtx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 200);

    let (tip, _correct_opening) = ctx.make_tip(&creator, 200, 12, 22);
    ctx.client.private_tip(&sender, &ctx.token, &tip, &200).unwrap();

    // Wrong blinding factor → commitment mismatch.
    let bad_opening = CommitmentOpening {
        creator: creator.clone(),
        amount: 200,
        blinding_factor: ctx.bytes32(99),
    };
    let result = ctx.client.try_private_withdraw(&tip, &bad_opening);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}

#[test]
fn test_private_tip_invalid_amount_rejected() {
    let ctx = PrivacyCtx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);

    let (tip, _) = ctx.make_tip(&creator, 0, 13, 23);
    let result = ctx.client.try_private_tip(&sender, &ctx.token, &tip, &0);
    assert_eq!(result, Err(Ok(TipJarError::InvalidAmount)));
}

#[test]
fn test_private_tip_duplicate_commitment_rejected() {
    let ctx = PrivacyCtx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 600);

    let (tip, _) = ctx.make_tip(&creator, 300, 14, 24);
    ctx.client.private_tip(&sender, &ctx.token, &tip, &300).unwrap();

    // Same commitment again must fail.
    let result = ctx.client.try_private_tip(&sender, &ctx.token, &tip, &300);
    assert_eq!(result, Err(Ok(TipJarError::InvalidAmount)));
}

#[test]
fn test_private_tip_anonymity_no_creator_link() {
    // Verify that no CreatorBalance is updated by private_tip — the link is
    // only established at withdrawal time via the opening.
    let ctx = PrivacyCtx::new();
    let sender = Address::generate(&ctx.env);
    let creator = Address::generate(&ctx.env);
    ctx.mint(&sender, 500);

    let (tip, _) = ctx.make_tip(&creator, 500, 15, 25);
    ctx.client.private_tip(&sender, &ctx.token, &tip, &500).unwrap();

    // Creator's on-chain balance should be zero — no link established yet.
    assert_eq!(ctx.client.get_balance(&creator, &ctx.token), 0);
}
