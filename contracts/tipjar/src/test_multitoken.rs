//! Multi-token and migration tests for the TipJar contract.

use crate::{Error, TipJar, TipJarClient, DataKey, CURRENT_DATA_VERSION, V1_DATA_VERSION};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events as _},
    token, vec, Address, Env, IntoVal, Vec,
};

struct MultiTokenCtx {
    env: Env,
    contract_id: Address,
    token_a: Address,
    token_b: Address,
    admin: Address,
}

impl MultiTokenCtx {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let token_admin = Address::generate(&env);
        let token_a = env
            .register_stellar_asset_contract_v2(token_admin.clone())
            .address();
        let token_b = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();

        let admin = Address::generate(&env);
        let contract_id = env.register(TipJar, ());
        let client = TipJarClient::new(&env, &contract_id);
        
        // Initialize with token_a
        client.init(&token_a);

        MultiTokenCtx {
            env,
            contract_id,
            token_a,
            token_b,
            admin,
        }
    }

    fn client(&self) -> TipJarClient<'_> {
        TipJarClient::new(&self.env, &self.contract_id)
    }

    fn token_client_a(&self) -> token::TokenClient<'_> {
        token::TokenClient::new(&self.env, &self.token_a)
    }

    fn token_client_b(&self) -> token::TokenClient<'_> {
        token::TokenClient::new(&self.env, &self.token_b)
    }

    fn fund_a(&self, user: &Address, amount: i128) {
        self.token_client_a().mint(user, &amount);
    }

    fn fund_b(&self, user: &Address, amount: i128) {
        self.token_client_b().mint(user, &amount);
    }
}

struct V1MigrationCtx {
    env: Env,
    contract_id: Address,
    token: Address,
}

impl V1MigrationCtx {
    fn new_v1() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let token_admin = Address::generate(&env);
        let token = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();

        let contract_id = env.register(TipJar, ());
        
        // Manually set up v1 data without going through init
        env.storage()
            .instance()
            .set(&DataKey::Token, &token);

        V1MigrationCtx {
            env,
            contract_id,
            token,
        }
    }

    fn client(&self) -> TipJarClient<'_> {
        TipJarClient::new(&self.env, &self.contract_id)
    }

    fn token_client(&self) -> token::TokenClient<'_> {
        token::TokenClient::new(&self.env, &self.token)
    }

    fn fund(&self, user: &Address, amount: i128) {
        self.token_client().mint(user, &amount);
    }

    fn seed_v1_data(&self, creator: &Address, balance: i128, total: i128) {
        self.env.storage().persistent().set(
            &DataKey::CreatorBalance(creator.clone()),
            &balance,
        );
        self.env.storage().persistent().set(
            &DataKey::CreatorTotal(creator.clone()),
            &total,
        );
    }
}

#[test]
fn test_multi_token_initialization() {
    let ctx = MultiTokenCtx::new();
    let client = ctx.client();
    
    // Should have token_a in allowlist initially
    let tokens = client.get_tokens();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens.first().unwrap(), ctx.token_a);
}

#[test]
fn test_add_and_remove_tokens() {
    let ctx = MultiTokenCtx::new();
    let client = ctx.client();
    
    // Add token_b
    client.add_token(&ctx.admin, &ctx.token_b);
    
    let tokens = client.get_tokens();
    assert_eq!(tokens.len(), 2);
    assert!(tokens.contains(&ctx.token_a));
    assert!(tokens.contains(&ctx.token_b));
    
    // Remove token_b
    client.remove_token(&ctx.admin, &ctx.token_b);
    
    let tokens = client.get_tokens();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens.first().unwrap(), ctx.token_a);
}

#[test]
fn test_multi_token_tip_and_balance_isolation() {
    let ctx = MultiTokenCtx::new();
    let client = ctx.client();
    let creator = Address::generate(&ctx.env);
    let tipper = Address::generate(&ctx.env);
    
    // Add token_b to allowlist
    client.add_token(&ctx.admin, &ctx.token_b);
    
    // Fund tipper with both tokens
    ctx.fund_a(&tipper, 1000);
    ctx.fund_b(&tipper, 2000);
    
    // Tip different amounts in different tokens
    client.tip(&tipper, &creator, &ctx.token_a, &100);
    client.tip(&tipper, &creator, &ctx.token_b, &300);
    
    // Check balances are isolated per token
    assert_eq!(client.get_balance(&creator, &ctx.token_a), 100);
    assert_eq!(client.get_balance(&creator, &ctx.token_b), 300);
    assert_eq!(client.get_total_tips(&creator, &ctx.token_a), 100);
    assert_eq!(client.get_total_tips(&creator, &ctx.token_b), 300);
    
    // Tip more to same creator in same tokens
    client.tip(&tipper, &creator, &ctx.token_a, &50);
    client.tip(&tipper, &creator, &ctx.token_b, &200);
    
    // Check accumulation
    assert_eq!(client.get_balance(&creator, &ctx.token_a), 150);
    assert_eq!(client.get_balance(&creator, &ctx.token_b), 500);
    assert_eq!(client.get_total_tips(&creator, &ctx.token_a), 150);
    assert_eq!(client.get_total_tips(&creator, &ctx.token_b), 500);
}

#[test]
fn test_multi_token_withdrawal() {
    let ctx = MultiTokenCtx::new();
    let client = ctx.client();
    let creator = Address::generate(&ctx.env);
    let tipper = Address::generate(&ctx.env);
    
    // Add token_b to allowlist
    client.add_token(&ctx.admin, &ctx.token_b);
    
    // Fund and tip
    ctx.fund_a(&tipper, 1000);
    ctx.fund_b(&tipper, 1000);
    client.tip(&tipper, &creator, &ctx.token_a, &100);
    client.tip(&tipper, &creator, &ctx.token_b, &200);
    
    // Withdraw token_a
    let creator_balance_a_before = ctx.token_client_a().balance(&creator);
    client.withdraw(&creator, &creator, &ctx.token_a, &creator, &Some(50));
    let creator_balance_a_after = ctx.token_client_a().balance(&creator);
    
    assert_eq!(creator_balance_a_after - creator_balance_a_before, 50);
    assert_eq!(client.get_balance(&creator, &ctx.token_a), 50);
    assert_eq!(client.get_balance(&creator, &ctx.token_b), 200); // Unchanged
    
    // Withdraw token_b (full amount)
    let creator_balance_b_before = ctx.token_client_b().balance(&creator);
    client.withdraw(&creator, &creator, &ctx.token_b, &creator, &None);
    let creator_balance_b_after = ctx.token_client_b().balance(&creator);
    
    assert_eq!(creator_balance_b_after - creator_balance_b_before, 200);
    assert_eq!(client.get_balance(&creator, &ctx.token_a), 50); // Unchanged
    assert_eq!(client.get_balance(&creator, &ctx.token_b), 0);
}

#[test]
fn test_token_not_allowed_error() {
    let ctx = MultiTokenCtx::new();
    let client = ctx.client();
    let creator = Address::generate(&ctx.env);
    let tipper = Address::generate(&ctx.env);
    
    // Try to tip with token_b which is not in allowlist
    ctx.fund_b(&tipper, 1000);
    
    let result = client.try_tip(&tipper, &creator, &ctx.token_b, &100);
    assert_eq!(result, Err(Ok(Error::TokenNotAllowed)));
}

#[test]
fn test_token_already_exists_error() {
    let ctx = MultiTokenCtx::new();
    let client = ctx.client();
    
    // Try to add token_a again (already in allowlist)
    let result = client.try_add_token(&ctx.admin, &ctx.token_a);
    assert_eq!(result, Err(Ok(Error::TokenAlreadyExists)));
}

#[test]
fn test_remove_nonexistent_token_error() {
    let ctx = MultiTokenCtx::new();
    let client = ctx.client();
    
    // Try to remove token_b which is not in allowlist
    let result = client.try_remove_token(&ctx.admin, &ctx.token_b);
    assert_eq!(result, Err(Ok(Error::TokenNotAllowed)));
}

#[test]
fn test_removed_token_withdrawal_still_works() {
    let ctx = MultiTokenCtx::new();
    let client = ctx.client();
    let creator = Address::generate(&ctx.env);
    let tipper = Address::generate(&ctx.env);
    
    // Add token_b, tip, then remove token_b
    client.add_token(&ctx.admin, &ctx.token_b);
    ctx.fund_b(&tipper, 1000);
    client.tip(&tipper, &creator, &ctx.token_b, &100);
    
    // Remove token_b from allowlist
    client.remove_token(&ctx.admin, &ctx.token_b);
    
    // Should still be able to withdraw existing balance
    let creator_balance_before = ctx.token_client_b().balance(&creator);
    client.withdraw(&creator, &creator, &ctx.token_b, &creator, &None);
    let creator_balance_after = ctx.token_client_b().balance(&creator);
    
    assert_eq!(creator_balance_after - creator_balance_before, 100);
    assert_eq!(client.get_balance(&creator, &ctx.token_b), 0);
    
    // But should not be able to tip with removed token
    let result = client.try_tip(&tipper, &creator, &ctx.token_b, &50);
    assert_eq!(result, Err(Ok(Error::TokenNotAllowed)));
}

#[test]
fn test_v1_to_v2_lazy_migration() {
    let ctx = V1MigrationCtx::new_v1();
    let client = ctx.client();
    let creator = Address::generate(&ctx.env);
    let tipper = Address::generate(&ctx.env);
    
    // Seed v1 data
    ctx.seed_v1_data(&creator, 500, 1000);
    
    // Fund tipper
    ctx.fund(&tipper, 2000);
    
    // Access v1 data - this should trigger migration
    let balance = client.get_balance(&creator, &ctx.token);
    let total = client.get_total_tips(&creator, &ctx.token);
    
    assert_eq!(balance, 500);
    assert_eq!(total, 1000);
    
    // Check that data version was upgraded
    let version: u32 = ctx.env.storage().instance().get(&DataKey::DataVersion).unwrap();
    assert_eq!(version, CURRENT_DATA_VERSION);
    
    // Check that v1 data was removed and v2 data exists
    assert!(!ctx.env.storage().persistent().has(&DataKey::CreatorBalance(creator.clone())));
    assert!(!ctx.env.storage().persistent().has(&DataKey::CreatorTotal(creator.clone())));
    assert!(ctx.env.storage().persistent().has(&DataKey::Balance(creator.clone(), ctx.token.clone())));
    assert!(ctx.env.storage().persistent().has(&DataKey::Total(creator.clone(), ctx.token.clone())));
    
    // Check that token allowlist was initialized
    let tokens = client.get_tokens();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens.first().unwrap(), ctx.token);
    
    // Test that tipping still works after migration
    client.tip(&tipper, &creator, &ctx.token, &200);
    assert_eq!(client.get_balance(&creator, &ctx.token), 700);
    assert_eq!(client.get_total_tips(&creator, &ctx.token), 1200);
    
    // Test withdrawal works after migration
    let creator_balance_before = ctx.token_client().balance(&creator);
    client.withdraw(&creator, &creator, &ctx.token, &creator, &Some(300));
    let creator_balance_after = ctx.token_client().balance(&creator);
    
    assert_eq!(creator_balance_after - creator_balance_before, 300);
    assert_eq!(client.get_balance(&creator, &ctx.token), 400);
}

#[test]
fn test_migration_preserves_zero_balances() {
    let ctx = V1MigrationCtx::new_v1();
    let client = ctx.client();
    let creator = Address::generate(&ctx.env);
    
    // Don't seed any v1 data - should handle missing data gracefully
    let balance = client.get_balance(&creator, &ctx.token);
    let total = client.get_total_tips(&creator, &ctx.token);
    
    assert_eq!(balance, 0);
    assert_eq!(total, 0);
    
    // Check migration still happened
    let version: u32 = ctx.env.storage().instance().get(&DataKey::DataVersion).unwrap();
    assert_eq!(version, CURRENT_DATA_VERSION);
}

#[test]
fn test_events_include_token_address() {
    let ctx = MultiTokenCtx::new();
    let client = ctx.client();
    let creator = Address::generate(&ctx.env);
    let tipper = Address::generate(&ctx.env);
    
    ctx.fund_a(&tipper, 1000);
    
    // Clear any existing events
    ctx.env.events().all();
    
    // Tip and check event
    client.tip(&tipper, &creator, &ctx.token_a, &100);
    
    let events = ctx.env.events().all();
    let tip_event = events.last().unwrap();
    
    assert_eq!(
        tip_event.topics,
        (symbol_short!("tip"), creator.clone())
    );
    assert_eq!(
        tip_event.data,
        (ctx.token_a.clone(), tipper.clone(), 100i128).into_val(&ctx.env)
    );
    
    // Withdraw and check event
    client.withdraw(&creator, &creator, &ctx.token_a, &creator, &Some(50));
    
    let events = ctx.env.events().all();
    let withdraw_event = events.last().unwrap();
    
    assert_eq!(
        withdraw_event.topics,
        (symbol_short!("withdraw"), creator.clone())
    );
    assert_eq!(
        withdraw_event.data,
        (ctx.token_a.clone(), 50i128, creator.clone()).into_val(&ctx.env)
    );
}

#[test]
fn test_max_tokens_limit() {
    let ctx = MultiTokenCtx::new();
    let client = ctx.client();
    
    // Add tokens up to the limit (already have 1, so add 49 more)
    for i in 0..49 {
        let token = Address::generate(&ctx.env);
        client.add_token(&ctx.admin, &token);
    }
    
    // Try to add one more - should fail
    let extra_token = Address::generate(&ctx.env);
    let result = client.try_add_token(&ctx.admin, &extra_token);
    assert_eq!(result, Err(Ok(Error::MaxTokensReached)));
}

#[test]
fn test_overflow_protection_per_token() {
    let ctx = MultiTokenCtx::new();
    let client = ctx.client();
    let creator = Address::generate(&ctx.env);
    let tipper = Address::generate(&ctx.env);
    
    ctx.fund_a(&tipper, i128::MAX);
    
    // Tip maximum amount
    client.tip(&tipper, &creator, &ctx.token_a, &i128::MAX);
    assert_eq!(client.get_balance(&creator, &ctx.token_a), i128::MAX);
    
    // Try to tip more - should fail with overflow
    ctx.fund_a(&tipper, 1);
    let result = client.try_tip(&tipper, &creator, &ctx.token_a, &1);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
}

#[test]
fn test_different_creators_independent_balances() {
    let ctx = MultiTokenCtx::new();
    let client = ctx.client();
    let creator1 = Address::generate(&ctx.env);
    let creator2 = Address::generate(&ctx.env);
    let tipper = Address::generate(&ctx.env);
    
    ctx.fund_a(&tipper, 1000);
    
    // Tip different amounts to different creators
    client.tip(&tipper, &creator1, &ctx.token_a, &100);
    client.tip(&tipper, &creator2, &ctx.token_a, &200);
    
    // Check balances are independent
    assert_eq!(client.get_balance(&creator1, &ctx.token_a), 100);
    assert_eq!(client.get_balance(&creator2, &ctx.token_a), 200);
    assert_eq!(client.get_total_tips(&creator1, &ctx.token_a), 100);
    assert_eq!(client.get_total_tips(&creator2, &ctx.token_a), 200);
    
    // Withdraw from creator1 shouldn't affect creator2
    client.withdraw(&creator1, &creator1, &ctx.token_a, &creator1, &Some(50));
    
    assert_eq!(client.get_balance(&creator1, &ctx.token_a), 50);
    assert_eq!(client.get_balance(&creator2, &ctx.token_a), 200); // Unchanged
    assert_eq!(client.get_total_tips(&creator1, &ctx.token_a), 100); // Total unchanged
    assert_eq!(client.get_total_tips(&creator2, &ctx.token_a), 200); // Unchanged
}

#[test]
fn test_complete_migration_workflow() {
    // Test the complete workflow: v1 setup → migration → v2 operations
    let ctx = V1MigrationCtx::new_v1();
    let client = ctx.client();
    let creator = Address::generate(&ctx.env);
    let tipper = Address::generate(&ctx.env);
    
    // Seed substantial v1 data
    ctx.seed_v1_data(&creator, 5000, 10000);
    ctx.fund(&tipper, 20000);
    
    // Check initial state is v1
    let version: Option<u32> = ctx.env.storage().instance().get(&DataKey::DataVersion);
    assert!(version.is_none() || version == Some(V1_DATA_VERSION));
    
    // Access data to trigger migration
    let pre_migration_balance = client.get_balance(&creator, &ctx.token);
    let pre_migration_total = client.get_total_tips(&creator, &ctx.token);
    
    // Verify migration preserved data
    assert_eq!(pre_migration_balance, 5000);
    assert_eq!(pre_migration_total, 10000);
    
    // Check v1 data was cleaned up
    assert!(!ctx.env.storage().persistent().has(&DataKey::CreatorBalance(creator.clone())));
    assert!(!ctx.env.storage().persistent().has(&DataKey::CreatorTotal(creator.clone())));
    
    // Check v2 data exists
    assert!(ctx.env.storage().persistent().has(&DataKey::Balance(creator.clone(), ctx.token.clone())));
    assert!(ctx.env.storage().persistent().has(&DataKey::Total(creator.clone(), ctx.token.clone())));
    
    // Verify new functionality works post-migration
    client.tip(&tipper, &creator, &ctx.token, &3000);
    assert_eq!(client.get_balance(&creator, &ctx.token), 8000);
    assert_eq!(client.get_total_tips(&creator, &ctx.token), 13000);
    
    // Test partial withdrawal
    client.withdraw(&creator, &creator, &ctx.token, &creator, &Some(2000));
    assert_eq!(client.get_balance(&creator, &ctx.token), 6000);
    assert_eq!(client.get_total_tips(&creator, &ctx.token), 13000); // Total unchanged
    
    // Add a new token to demonstrate multi-token capability post-migration
    let new_token = ctx.env.register_stellar_asset_contract_v2(Address::generate(&ctx.env)).address();
    client.add_token(&creator, &new_token); // Creator acts as admin in this test
    
    let new_tipper = Address::generate(&ctx.env);
    token::StellarAssetClient::new(&ctx.env, &new_token).mint(&new_tipper, &5000);
    
    client.tip(&new_tipper, &creator, &new_token, &1000);
    
    // Verify balances are isolated between tokens
    assert_eq!(client.get_balance(&creator, &ctx.token), 6000);
    assert_eq!(client.get_balance(&creator, &new_token), 1000);
    assert_eq!(client.get_total_tips(&creator, &ctx.token), 13000);
    assert_eq!(client.get_total_tips(&creator, &new_token), 1000);
}
