#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, token,
    Address, Env, Symbol, Vec,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Order {
    pub id: u64,
    pub owner: Address,
    pub derivative_type: Symbol,
    pub underlying: Address,
    pub strike_price: i128,
    pub expiry: u64,
    pub quantity: i128,
    pub is_buy: bool,
    pub settled: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Position {
    pub owner: Address,
    pub derivative_id: u64,
    pub quantity: i128,
    pub entry_price: i128,
}

#[contracttype]
pub enum DataKey {
    Order(u64),
    Position(Address, u64),
    OrderCount,
    OrderIds,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum DerivativesError {
    Unauthorized = 1,
    OrderNotFound = 2,
    AlreadySettled = 3,
    NotExpired = 4,
    InvalidAmount = 5,
    PositionNotFound = 6,
}

#[contract]
pub struct DerivativesContract;

#[contractimpl]
impl DerivativesContract {
    pub fn place_order(
        env: Env,
        caller: Address,
        derivative_type: Symbol,
        underlying: Address,
        strike_price: i128,
        expiry: u64,
        quantity: i128,
        is_buy: bool,
    ) -> u64 {
        caller.require_auth();
        if quantity <= 0 || strike_price <= 0 {
            panic_with_error!(&env, DerivativesError::InvalidAmount);
        }

        let order_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::OrderCount)
            .unwrap_or(0u64)
            + 1;

        let order = Order {
            id: order_id,
            owner: caller.clone(),
            derivative_type,
            underlying,
            strike_price,
            expiry,
            quantity,
            is_buy,
            settled: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Order(order_id), &order);
        env.storage()
            .persistent()
            .set(&DataKey::OrderCount, &order_id);

        let mut ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::OrderIds)
            .unwrap_or(Vec::new(&env));
        ids.push_back(order_id);
        env.storage().persistent().set(&DataKey::OrderIds, &ids);

        env.events().publish(
            (symbol_short!("deriv"), symbol_short!("placed")),
            (order_id, caller),
        );

        order_id
    }

    pub fn match_orders(env: Env) {
        let ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::OrderIds)
            .unwrap_or(Vec::new(&env));

        let mut orders: Vec<Order> = Vec::new(&env);
        for id in ids.iter() {
            if let Some(o) = env
                .storage()
                .persistent()
                .get::<DataKey, Order>(&DataKey::Order(id))
            {
                if !o.settled {
                    orders.push_back(o);
                }
            }
        }

        let len = orders.len();
        for i in 0..len {
            let buy = orders.get(i).unwrap();
            if !buy.is_buy {
                continue;
            }
            for j in (i + 1)..len {
                let sell = orders.get(j).unwrap();
                if sell.is_buy {
                    continue;
                }
                if buy.derivative_type != sell.derivative_type {
                    continue;
                }
                if buy.strike_price < sell.strike_price {
                    continue;
                }

                let match_price = sell.strike_price;
                let match_qty = if buy.quantity < sell.quantity {
                    buy.quantity
                } else {
                    sell.quantity
                };

                let buy_pos = Position {
                    owner: buy.owner.clone(),
                    derivative_id: buy.id,
                    quantity: match_qty,
                    entry_price: match_price,
                };
                let sell_pos = Position {
                    owner: sell.owner.clone(),
                    derivative_id: sell.id,
                    quantity: match_qty,
                    entry_price: match_price,
                };

                env.storage()
                    .persistent()
                    .set(&DataKey::Position(buy.owner.clone(), buy.id), &buy_pos);
                env.storage()
                    .persistent()
                    .set(&DataKey::Position(sell.owner.clone(), sell.id), &sell_pos);

                env.events().publish(
                    (symbol_short!("deriv"), symbol_short!("matched")),
                    (buy.id, sell.id, match_price),
                );

                // Mark both as settled after match
                let mut buy_mut = buy.clone();
                buy_mut.settled = true;
                let mut sell_mut = sell.clone();
                sell_mut.settled = true;
                env.storage()
                    .persistent()
                    .set(&DataKey::Order(buy.id), &buy_mut);
                env.storage()
                    .persistent()
                    .set(&DataKey::Order(sell.id), &sell_mut);
                break;
            }
        }
    }

    pub fn settle_order(env: Env, caller: Address, order_id: u64) {
        caller.require_auth();

        let mut order: Order = env
            .storage()
            .persistent()
            .get(&DataKey::Order(order_id))
            .unwrap_or_else(|| panic_with_error!(&env, DerivativesError::OrderNotFound));

        if order.settled {
            panic_with_error!(&env, DerivativesError::AlreadySettled);
        }

        let now = env.ledger().timestamp();
        if now < order.expiry {
            panic_with_error!(&env, DerivativesError::NotExpired);
        }

        let settlement_value = order.quantity * order.strike_price;

        let token_client = token::Client::new(&env, &order.underlying);
        token_client.transfer(
            &env.current_contract_address(),
            &order.owner,
            &settlement_value,
        );

        order.settled = true;
        env.storage()
            .persistent()
            .set(&DataKey::Order(order_id), &order);

        env.events().publish(
            (symbol_short!("deriv"), symbol_short!("settled")),
            (order_id, settlement_value),
        );
    }

    pub fn get_position(env: Env, owner: Address, derivative_id: u64) -> Position {
        env.storage()
            .persistent()
            .get(&DataKey::Position(owner, derivative_id))
            .unwrap_or_else(|| panic_with_error!(&env, DerivativesError::PositionNotFound))
    }

    pub fn cancel_order(env: Env, caller: Address, order_id: u64) {
        caller.require_auth();

        let mut order: Order = env
            .storage()
            .persistent()
            .get(&DataKey::Order(order_id))
            .unwrap_or_else(|| panic_with_error!(&env, DerivativesError::OrderNotFound));

        if order.owner != caller {
            panic_with_error!(&env, DerivativesError::Unauthorized);
        }

        if order.settled {
            panic_with_error!(&env, DerivativesError::AlreadySettled);
        }

        order.settled = true;
        env.storage()
            .persistent()
            .set(&DataKey::Order(order_id), &order);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::StellarAssetClient,
        Env, Symbol,
    };

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(DerivativesContract, ());
        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
        (env, contract_id, token_contract.address())
    }

    #[test]
    fn test_place_and_match_orders() {
        let (env, contract_id, token) = setup();
        let client = DerivativesContractClient::new(&env, &contract_id);

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let deriv_type = Symbol::new(&env, "CALL");

        let buy_id = client.place_order(
            &buyer,
            &deriv_type,
            &token,
            &100i128,
            &9999u64,
            &10i128,
            &true,
        );
        let sell_id = client.place_order(
            &seller,
            &deriv_type,
            &token,
            &100i128,
            &9999u64,
            &10i128,
            &false,
        );

        assert_eq!(buy_id, 1);
        assert_eq!(sell_id, 2);

        client.match_orders();

        let buy_pos = client.get_position(&buyer, &buy_id);
        assert_eq!(buy_pos.quantity, 10);
        assert_eq!(buy_pos.entry_price, 100);
        assert_eq!(buy_pos.owner, buyer);
        assert_eq!(buy_pos.derivative_id, buy_id);

        let sell_pos = client.get_position(&seller, &sell_id);
        assert_eq!(sell_pos.quantity, 10);
        assert_eq!(sell_pos.entry_price, 100);
        assert_eq!(sell_pos.owner, seller);
        assert_eq!(sell_pos.derivative_id, sell_id);
    }

    #[test]
    fn test_settle_after_expiry() {
        let (env, contract_id, _token) = setup();
        let client = DerivativesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_addr = token_contract.address();

        // Mint tokens to contract
        let asset_client = StellarAssetClient::new(&env, &token_addr);
        asset_client.mint(&contract_id, &10000i128);

        let deriv_type = Symbol::new(&env, "PUT");
        let expiry = 1000u64;

        let order_id = client.place_order(
            &owner,
            &deriv_type,
            &token_addr,
            &50i128,
            &expiry,
            &5i128,
            &true,
        );

        // Advance time past expiry
        env.ledger().with_mut(|l| l.timestamp = 2000);

        client.settle_order(&owner, &order_id);

        let order: Order = env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .get::<DataKey, Order>(&DataKey::Order(order_id))
                .unwrap()
        });
        assert!(order.settled);
        assert_eq!(order.id, order_id);
        assert_eq!(order.quantity, 5);
        assert_eq!(order.strike_price, 50);
    }

    #[test]
    #[should_panic]
    fn test_settle_before_expiry_fails() {
        let (env, contract_id, token) = setup();
        let client = DerivativesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let deriv_type = Symbol::new(&env, "CALL");
        let expiry = 9999u64;

        env.ledger().with_mut(|l| l.timestamp = 100);

        let order_id = client.place_order(
            &owner,
            &deriv_type,
            &token,
            &100i128,
            &expiry,
            &1i128,
            &true,
        );
        client.settle_order(&owner, &order_id);
    }

    #[test]
    #[should_panic]
    fn test_non_owner_cancel_fails() {
        let (env, contract_id, token) = setup();
        let client = DerivativesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let deriv_type = Symbol::new(&env, "CALL");

        let order_id = client.place_order(
            &owner,
            &deriv_type,
            &token,
            &100i128,
            &9999u64,
            &1i128,
            &true,
        );
        client.cancel_order(&attacker, &order_id);
    }

    #[test]
    fn test_owner_cancel_succeeds() {
        let (env, contract_id, _token) = setup();
        let client = DerivativesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let deriv_type = Symbol::new(&env, "CALL");

        let order_id = client.place_order(
            &owner,
            &deriv_type,
            &_token,
            &100i128,
            &9999u64,
            &1i128,
            &true,
        );
        client.cancel_order(&owner, &order_id);

        let order: Order = env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .get::<DataKey, Order>(&DataKey::Order(order_id))
                .unwrap()
        });
        assert!(order.settled);
        assert_eq!(order.owner, owner);
        assert_eq!(order.quantity, 1);
    }
}
