use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    token, Address, Env, String as SorobanString, Map, Vec as SorobanVec, BytesN,
};
use tipjar::{TipJarContract, TipJarContractClient, Role};

pub struct TestContext {
    pub env: Env,
    pub contract_id: Address,
    pub tipjar_client: TipJarContractClient,
    pub admin: Address,
    pub token_1: Address,
    pub token_2: Address,
    pub token_3: Address,
    pub token_admin: Address,
}

impl TestContext {
    pub fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let token_admin = Address::generate(&env);
        let token_1 = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
        let token_2 = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
        let token_3 = env.register_stellar_asset_contract_v2(token_admin.clone()).address();

        let admin = Address::generate(&env);
        let contract_id = env.register(TipJarContract, ());
        let tipjar_client = TipJarContractClient::new(&env, &contract_id);
        
        // Initialize contract
        tipjar_client.init(&admin);
        
        // Whitelist tokens
        tipjar_client.add_token(&admin, &token_1);
        tipjar_client.add_token(&admin, &token_2);
        // token_3 intentionally not whitelisted for testing

        Self {
            env,
            contract_id,
            tipjar_client,
            admin,
            token_1,
            token_2,
            token_3,
            token_admin,
        }
    }

    pub fn create_user(&self) -> Address {
        Address::generate(&self.env)
    }

    pub fn create_creator(&self) -> Address {
        let creator = Address::generate(&self.env);
        self.tipjar_client.grant_role(&self.admin, &creator, &Role::Creator);
        creator
    }

    pub fn create_moderator(&self) -> Address {
        let moderator = Address::generate(&self.env);
        self.tipjar_client.grant_role(&self.admin, &moderator, &Role::Moderator);
        moderator
    }

    pub fn mint_tokens(&self, user: &Address, token: &Address, amount: i128) {
        let token_admin_client = token::StellarAssetClient::new(&self.env, token);
        token_admin_client.mint(user, &amount);
    }

    pub fn get_token_balance(&self, user: &Address, token: &Address) -> i128 {
        let token_client = token::Client::new(&self.env, token);
        token_client.balance(user)
    }

    pub fn advance_time(&self, seconds: u64) {
        self.env.ledger().with_mut(|li| {
            li.timestamp += seconds;
        });
    }

    pub fn get_current_time(&self) -> u64 {
        self.env.ledger().timestamp()
    }

    pub fn get_events(&self) -> soroban_sdk::Vec<(Address, soroban_sdk::Vec<soroban_sdk::Val>, soroban_sdk::Val)> {
        self.env.events().all()
    }

    pub fn clear_events(&self) {
        // Events are automatically cleared between test runs in the test environment
    }
}

pub struct GasTracker {
    env: Env,
    initial_budget: u64,
}

impl GasTracker {
    pub fn new(env: &Env) -> Self {
        Self {
            env: env.clone(),
            initial_budget: env.budget().cpu_instruction_cost(),
        }
    }

    pub fn measure<F, R>(&self, operation: F) -> (R, u64)
    where
        F: FnOnce() -> R,
    {
        let start_budget = self.env.budget().cpu_instruction_cost();
        let result = operation();
        let end_budget = self.env.budget().cpu_instruction_cost();
        let gas_used = start_budget.saturating_sub(end_budget);
        (result, gas_used)
    }
}

// Helper functions for creating test data
pub fn create_batch_tips(env: &Env, creators: &[Address], token: &Address, amounts: &[i128]) -> SorobanVec<tipjar::BatchTip> {
    let mut tips = SorobanVec::new(env);
    for (creator, amount) in creators.iter().zip(amounts.iter()) {
        tips.push_back(tipjar::BatchTip {
            creator: creator.clone(),
            token: token.clone(),
            amount: *amount,
        });
    }
    tips
}

pub fn create_tip_message(env: &Env, message: &str) -> SorobanString {
    SorobanString::from_str(env, message)
}

pub fn create_metadata(env: &Env, key_values: &[(&str, &str)]) -> Map<SorobanString, SorobanString> {
    let mut metadata = Map::new(env);
    for (key, value) in key_values {
        metadata.set(
            SorobanString::from_str(env, key),
            SorobanString::from_str(env, value),
        );
    }
    metadata
}

// Assertion helpers
pub fn assert_balance_equals(ctx: &TestContext, user: &Address, token: &Address, expected: i128) {
    let actual = ctx.get_token_balance(user, token);
    assert_eq!(actual, expected, "Balance mismatch for user");
}

pub fn assert_withdrawable_balance_equals(ctx: &TestContext, creator: &Address, token: &Address, expected: i128) {
    let actual = ctx.tipjar_client.get_withdrawable_balance(creator.clone(), token.clone());
    assert_eq!(actual, expected, "Withdrawable balance mismatch for creator");
}

pub fn assert_total_tips_equals(ctx: &TestContext, creator: &Address, token: &Address, expected: i128) {
    let actual = ctx.tipjar_client.get_total_tips(creator.clone(), token.clone());
    assert_eq!(actual, expected, "Total tips mismatch for creator");
}

pub fn assert_error_contains<T>(result: Result<T, soroban_sdk::Error>, expected_error: tipjar::TipJarError) {
    match result {
        Err(error) => {
            let error_code: u32 = error.into();
            let expected_code: u32 = expected_error.into();
            assert_eq!(error_code, expected_code, "Error code mismatch");
        }
        Ok(_) => panic!("Expected error but operation succeeded"),
    }
}

// Property test helpers
pub fn generate_random_amount(env: &Env, max: i128) -> i128 {
    // Simple pseudo-random generation for testing
    let seed = env.ledger().timestamp() as i128;
    (seed % max).abs() + 1
}

pub fn generate_random_addresses(env: &Env, count: usize) -> Vec<Address> {
    (0..count).map(|_| Address::generate(env)).collect()
}