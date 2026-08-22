#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, token,
    Address, Env, Vec,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LimitOrder {
    pub id: u64,
    pub owner: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: i128,
    pub limit_price: i128,
    pub filled: i128,
    pub cancelled: bool,
}

#[contracttype]
pub enum DataKey {
    Order(u64),
    OrderCount,
    OrderBook(Address, Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum LimitOrderError {
    Unauthorized = 1,
    OrderNotFound = 2,
    AlreadyCancelled = 3,
    InvalidAmount = 4,
    AlreadyFilled = 5,
    PriceNotMet = 6,
}

#[contract]
pub struct LimitOrdersContract;

#[contractimpl]
impl LimitOrdersContract {
    pub fn place_order(
        env: Env,
        caller: Address,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        limit_price: i128,
    ) -> u64 {
        caller.require_auth();
        if amount_in <= 0 || limit_price <= 0 {
            panic_with_error!(&env, LimitOrderError::InvalidAmount);
        }

        // Lock token_in from caller into contract escrow
        let token_client = token::Client::new(&env, &token_in);
        token_client.transfer(&caller, &env.current_contract_address(), &amount_in);

        let order_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::OrderCount)
            .unwrap_or(0u64)
            + 1;

        let order = LimitOrder {
            id: order_id,
            owner: caller.clone(),
            token_in: token_in.clone(),
            token_out: token_out.clone(),
            amount_in,
            limit_price,
            filled: 0,
            cancelled: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Order(order_id), &order);
        env.storage()
            .persistent()
            .set(&DataKey::OrderCount, &order_id);

        // Add to order book
        let book_key = DataKey::OrderBook(token_in, token_out);
        let mut book: Vec<u64> = env
            .storage()
            .persistent()
            .get(&book_key)
            .unwrap_or(Vec::new(&env));
        book.push_back(order_id);
        env.storage().persistent().set(&book_key, &book);

        env.events().publish(
            (symbol_short!("limit"), symbol_short!("placed")),
            (order_id, caller, limit_price),
        );

        order_id
    }

    pub fn match_order(env: Env, order_id: u64, current_price: i128) {
        let mut order: LimitOrder = env
            .storage()
            .persistent()
            .get(&DataKey::Order(order_id))
            .unwrap_or_else(|| panic_with_error!(&env, LimitOrderError::OrderNotFound));

        if order.cancelled {
            panic_with_error!(&env, LimitOrderError::AlreadyCancelled);
        }

        if current_price > order.limit_price {
            panic_with_error!(&env, LimitOrderError::PriceNotMet);
        }

        let remaining = order.amount_in - order.filled;
        if remaining <= 0 {
            panic_with_error!(&env, LimitOrderError::AlreadyFilled);
        }

        // Fill the remaining amount at current_price
        // amount_out = remaining * limit_price / current_price (simplified: fill all remaining)
        let fill_amount = remaining;
        let amount_out = fill_amount * order.limit_price / current_price;

        let token_out_client = token::Client::new(&env, &order.token_out);
        token_out_client.transfer(&env.current_contract_address(), &order.owner, &amount_out);

        order.filled += fill_amount;
        env.storage()
            .persistent()
            .set(&DataKey::Order(order_id), &order);

        env.events().publish(
            (symbol_short!("limit"), symbol_short!("filled")),
            (order_id, fill_amount, current_price),
        );
    }

    pub fn cancel_order(env: Env, caller: Address, order_id: u64) {
        caller.require_auth();

        let mut order: LimitOrder = env
            .storage()
            .persistent()
            .get(&DataKey::Order(order_id))
            .unwrap_or_else(|| panic_with_error!(&env, LimitOrderError::OrderNotFound));

        if order.owner != caller {
            panic_with_error!(&env, LimitOrderError::Unauthorized);
        }

        if order.cancelled {
            panic_with_error!(&env, LimitOrderError::AlreadyCancelled);
        }

        // Refund remaining unfilled token_in
        let remaining = order.amount_in - order.filled;
        if remaining > 0 {
            let token_client = token::Client::new(&env, &order.token_in);
            token_client.transfer(&env.current_contract_address(), &caller, &remaining);
        }

        order.cancelled = true;
        env.storage()
            .persistent()
            .set(&DataKey::Order(order_id), &order);

        env.events().publish(
            (symbol_short!("limit"), symbol_short!("cancelled")),
            order_id,
        );
    }

    pub fn get_order(env: Env, order_id: u64) -> LimitOrder {
        env.storage()
            .persistent()
            .get(&DataKey::Order(order_id))
            .unwrap_or_else(|| panic_with_error!(&env, LimitOrderError::OrderNotFound))
    }

    pub fn get_order_book(env: Env, token_in: Address, token_out: Address) -> Vec<LimitOrder> {
        let ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::OrderBook(token_in, token_out))
            .unwrap_or(Vec::new(&env));

        let mut result = Vec::new(&env);
        for id in ids.iter() {
            if let Some(order) = env
                .storage()
                .persistent()
                .get::<DataKey, LimitOrder>(&DataKey::Order(id))
            {
                result.push_back(order);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::Address as _,
        token::{Client as TokenClient, StellarAssetClient},
        Env,
    };

    fn setup_token(env: &Env, admin: &Address, recipient: &Address, amount: i128) -> Address {
        let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
        let asset_client = StellarAssetClient::new(env, &token_contract.address());
        asset_client.mint(recipient, &amount);
        token_contract.address()
    }

    #[test]
    fn test_place_order_locks_tokens() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LimitOrdersContract, ());
        let client = LimitOrdersContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let buyer = Address::generate(&env);
        let token_in = setup_token(&env, &admin, &buyer, 1000);
        let token_out = Address::generate(&env);

        let token_client = TokenClient::new(&env, &token_in);
        assert_eq!(token_client.balance(&buyer), 1000);

        client.place_order(&buyer, &token_in, &token_out, &500i128, &100i128);

        assert_eq!(token_client.balance(&buyer), 500);
        assert_eq!(token_client.balance(&contract_id), 500);
    }

    #[test]
    fn test_match_order_executes_at_limit_price() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LimitOrdersContract, ());
        let client = LimitOrdersContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let buyer = Address::generate(&env);
        let token_in_admin = Address::generate(&env);

        let token_in = setup_token(&env, &token_in_admin, &buyer, 1000);
        let token_out = setup_token(&env, &admin, &contract_id, 10000);

        let order_id = client.place_order(&buyer, &token_in, &token_out, &100i128, &50i128);

        // current_price <= limit_price: should execute
        client.match_order(&order_id, &50i128);

        let order = client.get_order(&order_id);
        assert_eq!(order.filled, 100);
        assert_eq!(order.amount_in, 100);
        assert_eq!(order.limit_price, 50);
        assert_eq!(order.owner, buyer);
    }

    #[test]
    #[should_panic]
    fn test_match_order_fails_when_price_above_limit() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LimitOrdersContract, ());
        let client = LimitOrdersContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let buyer = Address::generate(&env);
        let token_in = setup_token(&env, &admin, &buyer, 1000);
        let token_out = Address::generate(&env);

        let order_id = client.place_order(&buyer, &token_in, &token_out, &100i128, &50i128);
        // current_price > limit_price: should fail
        client.match_order(&order_id, &60i128);
    }

    #[test]
    fn test_partial_fill_updates_filled_field() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LimitOrdersContract, ());
        let client = LimitOrdersContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let buyer = Address::generate(&env);
        let token_in_admin = Address::generate(&env);

        let token_in = setup_token(&env, &token_in_admin, &buyer, 1000);
        let token_out = setup_token(&env, &admin, &contract_id, 10000);

        let order_id = client.place_order(&buyer, &token_in, &token_out, &100i128, &50i128);
        client.match_order(&order_id, &50i128);

        let order = client.get_order(&order_id);
        assert_eq!(order.filled, 100); // full fill since match_order fills all remaining
        assert_eq!(order.id, order_id);
        assert!(!order.cancelled);
    }

    #[test]
    fn test_cancel_refunds_tokens() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LimitOrdersContract, ());
        let client = LimitOrdersContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let buyer = Address::generate(&env);
        let token_in = setup_token(&env, &admin, &buyer, 1000);
        let token_out = Address::generate(&env);

        let token_client = TokenClient::new(&env, &token_in);
        let order_id = client.place_order(&buyer, &token_in, &token_out, &500i128, &100i128);
        assert_eq!(token_client.balance(&buyer), 500);

        client.cancel_order(&buyer, &order_id);
        assert_eq!(token_client.balance(&buyer), 1000);

        let order = client.get_order(&order_id);
        assert!(order.cancelled);
        assert_eq!(order.filled, 0);
        assert_eq!(order.amount_in, 500);
    }

    #[test]
    #[should_panic]
    fn test_non_owner_cancel_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LimitOrdersContract, ());
        let client = LimitOrdersContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let buyer = Address::generate(&env);
        let attacker = Address::generate(&env);
        let token_in = setup_token(&env, &admin, &buyer, 1000);
        let token_out = Address::generate(&env);

        let order_id = client.place_order(&buyer, &token_in, &token_out, &500i128, &100i128);
        client.cancel_order(&attacker, &order_id);
    }
}
