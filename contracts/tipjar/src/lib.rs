#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, token,
    Address, Env, Map, String, Vec,
};

#[cfg(test)]
extern crate std;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipWithMessage {
    pub sender: Address,
    pub creator: Address,
    pub amount: i128,
    pub message: String,
    pub metadata: Map<String, String>,
    pub timestamp: u64,
}

/// Storage layout for persistent contract data.
#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    /// Token contract address used for all tips.
    Token,
    /// Creator's currently withdrawable balance held by this contract.
    CreatorBalance(Address),
    /// Historical total tips ever received by creator.
    CreatorTotal(Address),
    /// Emergency pause state (bool).
    Paused,
    /// Contract administrator (Address).
    Admin,
    /// Messages appended for a creator.
    CreatorMessages(Address),
}

/// Public view of the contract's current configuration.
///
/// Returned by [`TipJarContract::get_config`] so that frontends and SDKs
/// can read all relevant settings in a single, stable call without relying
/// on internal storage assumptions.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractConfig {
    /// The token contract address used for all tips.
    pub token: Address,
    /// The contract administrator address.
    pub admin: Address,
    /// Whether the contract is currently paused.
    pub paused: bool,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TipJarError {
    AlreadyInitialized = 1,
    TokenNotInitialized = 2,
    InvalidAmount = 3,
    NothingToWithdraw = 4,
    MessageTooLong = 5,
    /// Returned when a read is attempted before the contract is initialized.
    NotInitialized = 6,
}

#[contract]
pub struct TipJarContract;

#[contractimpl]
impl TipJarContract {
    /// One-time setup to choose the token contract and administrator for the TipJar.
    pub fn init(env: Env, token: Address, admin: Address) {
        if env.storage().instance().has(&DataKey::Token) {
            panic_with_error!(&env, TipJarError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    /// Moves `amount` tokens from `sender` into contract escrow for `creator`.
    ///
    /// The sender must authorize this call and have enough token balance.
    pub fn tip(env: Env, sender: Address, creator: Address, amount: i128) {
        Self::require_not_paused(&env);
        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }

        sender.require_auth();

        let token_id = Self::read_token(&env);
        let token_client = token::Client::new(&env, &token_id);
        let contract_address = env.current_contract_address();

        // Transfer tokens into contract escrow first so creators can withdraw later.
        token_client.transfer(&sender, &contract_address, &amount);

        let creator_balance_key = DataKey::CreatorBalance(creator.clone());
        let creator_total_key = DataKey::CreatorTotal(creator.clone());

        let current_balance: i128 = env
            .storage()
            .persistent()
            .get(&creator_balance_key)
            .unwrap_or(0);
        let current_total: i128 = env
            .storage()
            .persistent()
            .get(&creator_total_key)
            .unwrap_or(0);

        let next_balance = current_balance + amount;
        let next_total = current_total + amount;

        env.storage()
            .persistent()
            .set(&creator_balance_key, &next_balance);
        env.storage()
            .persistent()
            .set(&creator_total_key, &next_total);

        // Event topics: ("tip", creator). Event data: (sender, amount).
        env.events()
            .publish((symbol_short!("tip"), creator), (sender, amount));
    }

    /// Allows supporters to attach a note and metadata to a tip.
    pub fn tip_with_message(
        env: Env,
        sender: Address,
        creator: Address,
        amount: i128,
        message: String,
        metadata: Map<String, String>,
    ) {
        Self::require_not_paused(&env);
        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        if message.len() > 280 {
            panic_with_error!(&env, TipJarError::MessageTooLong);
        }

        sender.require_auth();

        let token_id = Self::read_token(&env);
        let token_client = token::Client::new(&env, &token_id);
        let contract_address = env.current_contract_address();

        // Transfer tokens into contract escrow first so creators can withdraw later.
        token_client.transfer(&sender, &contract_address, &amount);

        let creator_balance_key = DataKey::CreatorBalance(creator.clone());
        let creator_total_key = DataKey::CreatorTotal(creator.clone());
        let creator_msgs_key = DataKey::CreatorMessages(creator.clone());

        let current_balance: i128 = env
            .storage()
            .persistent()
            .get(&creator_balance_key)
            .unwrap_or(0);
        let current_total: i128 = env
            .storage()
            .persistent()
            .get(&creator_total_key)
            .unwrap_or(0);

        let next_balance = current_balance + amount;
        let next_total = current_total + amount;

        env.storage()
            .persistent()
            .set(&creator_balance_key, &next_balance);
        env.storage()
            .persistent()
            .set(&creator_total_key, &next_total);

        // Store message
        let timestamp = env.ledger().timestamp();
        let payload = TipWithMessage {
            sender: sender.clone(),
            creator: creator.clone(),
            amount,
            message: message.clone(),
            metadata: metadata.clone(),
            timestamp,
        };
        let mut messages: Vec<TipWithMessage> = env
            .storage()
            .persistent()
            .get(&creator_msgs_key)
            .unwrap_or_else(|| Vec::new(&env));
        messages.push_back(payload);
        env.storage().persistent().set(&creator_msgs_key, &messages);

        // Emit message payload
        env.events().publish(
            (symbol_short!("tip_msg"), creator),
            (sender, amount, message, metadata),
        );
    }

    /// Returns total historical tips for a creator.
    pub fn get_total_tips(env: Env, creator: Address) -> i128 {
        let key = DataKey::CreatorTotal(creator);
        env.storage().persistent().get(&key).unwrap_or(0)
    }

    /// Returns stored messages for a creator.
    pub fn get_messages(env: Env, creator: Address) -> Vec<TipWithMessage> {
        let key = DataKey::CreatorMessages(creator);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Returns currently withdrawable escrowed tips for a creator.
    pub fn get_withdrawable_balance(env: Env, creator: Address) -> i128 {
        let key = DataKey::CreatorBalance(creator);
        env.storage().persistent().get(&key).unwrap_or(0)
    }

    /// Returns the full current contract configuration in a single stable call.
    ///
    /// Safe and read-only — no state is mutated. Returns an error if the
    /// contract has not been initialized yet, so callers can distinguish
    /// between a missing config and a zero-value config.
    pub fn get_config(env: Env) -> Result<ContractConfig, TipJarError> {
        let token: Option<Address> = env.storage().instance().get(&DataKey::Token);
        let admin: Option<Address> = env.storage().instance().get(&DataKey::Admin);

        match (token, admin) {
            (Some(token), Some(admin)) => {
                let paused: bool = env
                    .storage()
                    .instance()
                    .get(&DataKey::Paused)
                    .unwrap_or(false);
                Ok(ContractConfig { token, admin, paused })
            }
            _ => Err(TipJarError::NotInitialized),
        }
    }

    /// Allows creator to withdraw their accumulated escrowed tips.
    pub fn withdraw(env: Env, creator: Address) {
        Self::require_not_paused(&env);
        creator.require_auth();

        let key = DataKey::CreatorBalance(creator.clone());
        let amount: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if amount <= 0 {
            panic_with_error!(&env, TipJarError::NothingToWithdraw);
        }

        let token_id = Self::read_token(&env);
        let token_client = token::Client::new(&env, &token_id);
        let contract_address = env.current_contract_address();

        token_client.transfer(&contract_address, &creator, &amount);
        env.storage().persistent().set(&key, &0i128);

        env.events()
            .publish((symbol_short!("withdraw"), creator), amount);
    }

    fn read_token(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Token)
            .unwrap_or_else(|| panic_with_error!(env, TipJarError::TokenNotInitialized))
    }

    /// Emergency pause to stop all state-changing activities (Admin only).
    pub fn pause(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic!("Unauthorized");
        }
        env.storage().instance().set(&DataKey::Paused, &true);
    }

    /// Resume contract activities after an emergency pause (Admin only).
    pub fn unpause(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic!("Unauthorized");
        }
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    /// Internal helper to check if the contract is paused.
    fn require_not_paused(env: &Env) {
        let is_paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if is_paused {
            panic!("Contract is paused");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, token, Address, Env};

    fn setup() -> (Env, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let token_admin = Address::generate(&env);
        let token_id = env
            .register_stellar_asset_contract_v2(token_admin.clone())
            .address();

        let admin = Address::generate(&env);
        let contract_id = env.register(TipJarContract, ());
        let tipjar_client = TipJarContractClient::new(&env, &contract_id);
        tipjar_client.init(&token_id, &admin);

        (env, contract_id, token_id, admin)
    }

    #[test]
    fn test_tipping_functionality() {
        let (env, contract_id, token_id, _) = setup();
        let tipjar_client = TipJarContractClient::new(&env, &contract_id);
        let token_client = token::Client::new(&env, &token_id);
        let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        token_admin_client.mint(&sender, &1_000);
        tipjar_client.tip(&sender, &creator, &250);

        assert_eq!(token_client.balance(&sender), 750);
        assert_eq!(token_client.balance(&contract_id), 250);
        assert_eq!(tipjar_client.get_total_tips(&creator), 250);
    }

    #[test]
    fn test_tipping_with_message_functionality() {
        let (env, contract_id, token_id, _) = setup();
        let tipjar_client = TipJarContractClient::new(&env, &contract_id);
        let token_client = token::Client::new(&env, &token_id);
        let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        let message = soroban_sdk::String::from_str(&env, "Great job!");
        let metadata = soroban_sdk::Map::new(&env);

        token_admin_client.mint(&sender, &1_000);
        tipjar_client.tip_with_message(&sender, &creator, &250, &message, &metadata);

        assert_eq!(token_client.balance(&sender), 750);
        assert_eq!(token_client.balance(&contract_id), 250);
        assert_eq!(tipjar_client.get_total_tips(&creator), 250);

        let msgs = tipjar_client.get_messages(&creator);
        assert_eq!(msgs.len(), 1);
        let msg = msgs.get(0).unwrap();
        assert_eq!(msg.message, message);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #5)")]
    fn test_tipping_message_too_long() {
        let (env, contract_id, token_id, _) = setup();
        let tipjar_client = TipJarContractClient::new(&env, &contract_id);
        let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        let long_str = "x".repeat(281);
        let message = soroban_sdk::String::from_str(&env, &long_str);
        let metadata = soroban_sdk::Map::new(&env);

        token_admin_client.mint(&sender, &1_000);
        tipjar_client.tip_with_message(&sender, &creator, &250, &message, &metadata);
    }

    #[test]
    fn test_balance_tracking_and_withdraw() {
        let (env, contract_id, token_id, _) = setup();
        let tipjar_client = TipJarContractClient::new(&env, &contract_id);
        let token_client = token::Client::new(&env, &token_id);
        let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
        let sender_a = Address::generate(&env);
        let sender_b = Address::generate(&env);
        let creator = Address::generate(&env);

        token_admin_client.mint(&sender_a, &1_000);
        token_admin_client.mint(&sender_b, &1_000);

        tipjar_client.tip(&sender_a, &creator, &100);
        tipjar_client.tip(&sender_b, &creator, &300);

        assert_eq!(tipjar_client.get_total_tips(&creator), 400);
        assert_eq!(tipjar_client.get_withdrawable_balance(&creator), 400);
        assert_eq!(token_client.balance(&contract_id), 400);

        tipjar_client.withdraw(&creator);

        assert_eq!(tipjar_client.get_withdrawable_balance(&creator), 0);
        assert_eq!(token_client.balance(&creator), 400);
        assert_eq!(token_client.balance(&contract_id), 0);
    }

    #[test]
    #[should_panic]
    fn test_invalid_tip_amount() {
        let (env, contract_id, token_id, _) = setup();
        let tipjar_client = TipJarContractClient::new(&env, &contract_id);
        let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        token_admin_client.mint(&sender, &100);

        // Zero tips are rejected to prevent accidental or abusive calls.
        tipjar_client.tip(&sender, &creator, &0);
    }

    #[test]
    fn test_pause_unpause() {
        let (env, contract_id, _token_id, admin) = setup();
        let tipjar_client = TipJarContractClient::new(&env, &contract_id);

        tipjar_client.pause(&admin);

        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        // This should fail
        let result = tipjar_client.try_tip(&sender, &creator, &100);
        assert!(result.is_err());

        // Unpause
        tipjar_client.unpause(&admin);

        // This should now succeed (once we mint tokens)
        let token_admin_client = token::StellarAssetClient::new(&env, &_token_id);
        token_admin_client.mint(&sender, &100);
        tipjar_client.tip(&sender, &creator, &100);
        assert_eq!(tipjar_client.get_total_tips(&creator), 100);
    }

    #[test]
    #[should_panic(expected = "Unauthorized")]
    fn test_pause_admin_only() {
        let (env, contract_id, _, _) = setup();
        let tipjar_client = TipJarContractClient::new(&env, &contract_id);
        let non_admin = Address::generate(&env);

        tipjar_client.pause(&non_admin);
    }

    #[test]
    fn test_withdraw_blocked_when_paused() {
        let (env, contract_id, token_id, admin) = setup();
        let tipjar_client = TipJarContractClient::new(&env, &contract_id);
        let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        token_admin_client.mint(&sender, &100);
        tipjar_client.tip(&sender, &creator, &100);

        tipjar_client.pause(&admin);

        let result = tipjar_client.try_withdraw(&creator);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_config_initialized() {
        let (env, contract_id, token_id, admin) = setup();
        let tipjar_client = TipJarContractClient::new(&env, &contract_id);

        // The Soroban client unwraps Ok automatically; use try_ to get the Result.
        let config = tipjar_client.get_config();
        assert_eq!(config.token, token_id);
        assert_eq!(config.admin, admin);
        assert!(!config.paused);
    }

    #[test]
    fn test_get_config_not_initialized() {
        let env = Env::default();
        let contract_id = env.register(TipJarContract, ());
        let tipjar_client = TipJarContractClient::new(&env, &contract_id);

        // try_ variant returns Result; Err means NotInitialized was returned.
        let result = tipjar_client.try_get_config();
        assert!(result.is_err());
    }

    #[test]
    fn test_get_config_reflects_pause_state() {
        let (env, contract_id, token_id, admin) = setup();
        let tipjar_client = TipJarContractClient::new(&env, &contract_id);

        let config_before = tipjar_client.get_config();
        assert!(!config_before.paused);
        assert_eq!(config_before.token, token_id);
        assert_eq!(config_before.admin, admin);

        tipjar_client.pause(&admin);

        let config_after = tipjar_client.get_config();
        assert!(config_after.paused);
        // token and admin must remain unchanged
        assert_eq!(config_after.token, token_id);
        assert_eq!(config_after.admin, admin);
    }
}
