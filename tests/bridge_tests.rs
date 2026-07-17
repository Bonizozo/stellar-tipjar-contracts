use soroban_sdk::{
    testutils::Address as _,
    token, Address, BytesN, Env, String as SorobanString,
};
use tipjar::{
    bridge::{BridgeTip, SourceChain},
    TipJarContract, TipJarContractClient, TipJarError,
};

// ── helpers ──────────────────────────────────────────────────────────────────

struct BridgeCtx {
    env: Env,
    client: TipJarContractClient,
    admin: Address,
    relayer: Address,
    bridge_token: Address,
    token_admin: Address,
}

impl BridgeCtx {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let token_admin = Address::generate(&env);
        let bridge_token = env
            .register_stellar_asset_contract_v2(token_admin.clone())
            .address();

        let admin = Address::generate(&env);
        let relayer = Address::generate(&env);
        let contract_id = env.register(TipJarContract, ());
        let client = TipJarContractClient::new(&env, &contract_id);

        client.init(&admin);
        client.set_bridge_relayer(&admin, &relayer, &bridge_token);

        Self { env, client, admin, relayer, bridge_token, token_admin }
    }

    fn mint(&self, to: &Address, amount: i128) {
        token::StellarAssetClient::new(&self.env, &self.bridge_token)
            .mint(to, &amount);
    }

    fn tx_hash(&self, seed: u8) -> BytesN<32> {
        BytesN::from_array(&self.env, &[seed; 32])
    }

    fn make_tip(&self, creator: &Address, amount: i128, seed: u8) -> BridgeTip {
        BridgeTip {
            source_chain: SourceChain::Ethereum,
            source_tx_hash: self.tx_hash(seed),
            creator: creator.clone(),
            amount,
            message: SorobanString::from_str(&self.env, ""),
        }
    }
}

// ── basic bridge tip tests ──────────────────────────────────────────────────

#[test]
fn test_bridge_tip_credits_creator_balance() {
    let ctx = BridgeCtx::new();
    let creator = Address::generate(&ctx.env);
    ctx.mint(&ctx.relayer, 500);

    ctx.client.bridge_tip(&ctx.relayer, &ctx.make_tip(&creator, 500, 1)).unwrap();

    assert_eq!(ctx.client.get_balance(&creator, &ctx.bridge_token), 500);
    assert_eq!(ctx.client.get_total_tips(&creator, &ctx.bridge_token), 500);
}

#[test]
fn test_bridge_tip_replay_rejected() {
    let ctx = BridgeCtx::new();
    let creator = Address::generate(&ctx.env);
    ctx.mint(&ctx.relayer, 1000);

    let tip = ctx.make_tip(&creator, 500, 2);
    ctx.client.bridge_tip(&ctx.relayer, &tip).unwrap();

    // Same tx hash → replay
    let result = ctx.client.try_bridge_tip(&ctx.relayer, &tip);
    assert_eq!(result, Err(Ok(TipJarError::InvalidAmount)));
}

#[test]
fn test_bridge_tip_invalid_amount_rejected() {
    let ctx = BridgeCtx::new();
    let creator = Address::generate(&ctx.env);

    let tip = BridgeTip {
        source_chain: SourceChain::Polygon,
        source_tx_hash: ctx.tx_hash(3),
        creator,
        amount: 0,
        message: SorobanString::from_str(&ctx.env, ""),
    };
    let result = ctx.client.try_bridge_tip(&ctx.relayer, &tip);
    assert_eq!(result, Err(Ok(TipJarError::InvalidAmount)));
}

#[test]
fn test_bridge_tip_unauthorized_relayer_rejected() {
    let ctx = BridgeCtx::new();
    let creator = Address::generate(&ctx.env);
    let impostor = Address::generate(&ctx.env);
    ctx.mint(&impostor, 500);

    let tip = ctx.make_tip(&creator, 500, 4);
    let result = ctx.client.try_bridge_tip(&impostor, &tip);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}

#[test]
fn test_bridge_tip_multiple_chains() {
    let ctx = BridgeCtx::new();
    let creator = Address::generate(&ctx.env);
    ctx.mint(&ctx.relayer, 900);

    for (chain, seed, amount) in [
        (SourceChain::Ethereum, 10u8, 100i128),
        (SourceChain::Polygon, 11, 200),
        (SourceChain::BinanceSmartChain, 12, 300),
        (SourceChain::Avalanche, 13, 150),
        (SourceChain::Arbitrum, 14, 150),
    ] {
        let tip = BridgeTip {
            source_chain: chain,
            source_tx_hash: ctx.tx_hash(seed),
            creator: creator.clone(),
            amount,
            message: SorobanString::from_str(&ctx.env, ""),
        };
        ctx.client.bridge_tip(&ctx.relayer, &tip).unwrap();
    }

    assert_eq!(ctx.client.get_total_tips(&creator, &ctx.bridge_token), 900);
}

#[test]
fn test_bridge_tip_creator_can_withdraw() {
    let ctx = BridgeCtx::new();
    let creator = Address::generate(&ctx.env);
    ctx.mint(&ctx.relayer, 300);

    ctx.client.bridge_tip(&ctx.relayer, &ctx.make_tip(&creator, 300, 20)).unwrap();
    ctx.client.withdraw(&creator, &ctx.bridge_token, &None);

    assert_eq!(ctx.client.get_balance(&creator, &ctx.bridge_token), 0);
    // Token balance transferred to creator
    assert_eq!(
        token::Client::new(&ctx.env, &ctx.bridge_token).balance(&creator),
        300
    );
}

// ── bridge fee tests ────────────────────────────────────────────────────────

#[test]
fn test_bridge_tip_with_fee() {
    let ctx = BridgeCtx::new();
    let creator = Address::generate(&ctx.env);

    // Set bridge fee to 200 bps (2%)
    ctx.client.set_bridge_fee(&ctx.admin, &200);
    assert_eq!(ctx.client.get_bridge_fee(), 200);

    ctx.mint(&ctx.relayer, 1000);
    ctx.client.bridge_tip(&ctx.relayer, &ctx.make_tip(&creator, 1000, 30)).unwrap();

    // Fee = 1000 * 200 / 10000 = 20
    // Net = 1000 - 20 = 980
    assert_eq!(ctx.client.get_balance(&creator, &ctx.bridge_token), 980);
    assert_eq!(ctx.client.get_total_tips(&creator, &ctx.bridge_token), 1000);
}

#[test]
fn test_bridge_fee_zero_by_default() {
    let ctx = BridgeCtx::new();
    assert_eq!(ctx.client.get_bridge_fee(), 0);
}

#[test]
fn test_set_bridge_fee_unauthorized() {
    let ctx = BridgeCtx::new();
    let impostor = Address::generate(&ctx.env);
    let result = ctx.client.try_set_bridge_fee(&impostor, &100);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}

#[test]
fn test_set_bridge_fee_exceeds_maximum() {
    let ctx = BridgeCtx::new();
    let result = ctx.client.try_set_bridge_fee(&ctx.admin, &600);
    assert_eq!(result, Err(Ok(TipJarError::InvalidBridgeFee)));
}

// ── bridge enable/disable tests ─────────────────────────────────────────────

#[test]
fn test_bridge_disabled_rejects_tip() {
    let ctx = BridgeCtx::new();
    let creator = Address::generate(&ctx.env);
    ctx.mint(&ctx.relayer, 500);

    // Disable bridge
    ctx.client.enable_bridge(&ctx.admin, &false);
    assert!(!ctx.client.is_bridge_enabled());

    let result = ctx.client.try_bridge_tip(&ctx.relayer, &ctx.make_tip(&creator, 500, 40));
    assert_eq!(result, Err(Ok(TipJarError::BridgeDisabled)));
}

#[test]
fn test_bridge_enable_re_enables() {
    let ctx = BridgeCtx::new();
    let creator = Address::generate(&ctx.env);
    ctx.mint(&ctx.relayer, 500);

    // Disable then re-enable
    ctx.client.enable_bridge(&ctx.admin, &false);
    ctx.client.enable_bridge(&ctx.admin, &true);
    assert!(ctx.client.is_bridge_enabled());

    // Should succeed now
    ctx.client.bridge_tip(&ctx.relayer, &ctx.make_tip(&creator, 500, 41)).unwrap();
    assert_eq!(ctx.client.get_balance(&creator, &ctx.bridge_token), 500);
}

#[test]
fn test_enable_bridge_unauthorized() {
    let ctx = BridgeCtx::new();
    let impostor = Address::generate(&ctx.env);
    let result = ctx.client.try_enable_bridge(&impostor, &false);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}

#[test]
fn test_bridge_disabled_by_default() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(TipJarContract, ());
    let client = TipJarContractClient::new(&env, &contract_id);
    client.init(&admin);

    // Bridge should be disabled before set_bridge_relayer is called
    assert!(!client.is_bridge_enabled());
}

// ── bridge configuration tests ──────────────────────────────────────────────

#[test]
fn test_set_bridge_relayer_sets_enabled() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let bridge_token = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let contract_id = env.register(TipJarContract, ());
    let client = TipJarContractClient::new(&env, &contract_id);
    client.init(&admin);

    // Before setting relayer, bridge is disabled
    assert!(!client.is_bridge_enabled());

    client.set_bridge_relayer(&admin, &relayer, &bridge_token);
    assert!(client.is_bridge_enabled());
}

#[test]
fn test_set_bridge_relayer_unauthorized() {
    let ctx = BridgeCtx::new();
    let impostor = Address::generate(&ctx.env);
    let relayer = Address::generate(&ctx.env);

    let result = ctx.client.try_set_bridge_relayer(&impostor, &relayer, &ctx.bridge_token);
    assert_eq!(result, Err(Ok(TipJarError::Unauthorized)));
}

// ── bridge with message tests ───────────────────────────────────────────────

#[test]
fn test_bridge_tip_with_message() {
    let ctx = BridgeCtx::new();
    let creator = Address::generate(&ctx.env);
    ctx.mint(&ctx.relayer, 500);

    let tip = BridgeTip {
        source_chain: SourceChain::Ethereum,
        source_tx_hash: ctx.tx_hash(50),
        creator: creator.clone(),
        amount: 500,
        message: SorobanString::from_str(&ctx.env, "Thanks for the great content!"),
    };

    ctx.client.bridge_tip(&ctx.relayer, &tip).unwrap();
    assert_eq!(ctx.client.get_balance(&creator, &ctx.bridge_token), 500);
}

