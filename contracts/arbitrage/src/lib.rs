#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, Address,
    Env, Symbol, Vec,
};

/// Minimum profit threshold: 100 basis points (1%)
const MIN_PROFIT_THRESHOLD_BPS: i128 = 100;
const BPS: i128 = 10_000;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketPrice {
    pub market_id: Symbol,
    pub token: Address,
    pub price: i128,
    pub updated_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArbitrageRecord {
    pub id: u64,
    pub executor: Address,
    pub token: Address,
    pub buy_market: Symbol,
    pub sell_market: Symbol,
    pub profit: i128,
    pub executed_at: u64,
}

#[contracttype]
pub enum DataKey {
    Price(Symbol, Address),
    Record(u64),
    RecordCount,
    ExecutorRecords(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ArbitrageError {
    Unauthorized = 1,
    PriceNotFound = 2,
    NotProfitable = 3,
    InvalidAmount = 4,
}

#[contract]
pub struct ArbitrageContract;

#[contractimpl]
impl ArbitrageContract {
    pub fn update_price(env: Env, caller: Address, market_id: Symbol, token: Address, price: i128) {
        caller.require_auth();
        if price <= 0 {
            panic_with_error!(&env, ArbitrageError::InvalidAmount);
        }
        let mp = MarketPrice {
            market_id: market_id.clone(),
            token: token.clone(),
            price,
            updated_at: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::Price(market_id, token), &mp);
    }

    pub fn detect_opportunity(
        env: Env,
        token: Address,
        market_a: Symbol,
        market_b: Symbol,
    ) -> (i128, bool) {
        let price_a: MarketPrice = env
            .storage()
            .persistent()
            .get(&DataKey::Price(market_a.clone(), token.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ArbitrageError::PriceNotFound));

        let price_b: MarketPrice = env
            .storage()
            .persistent()
            .get(&DataKey::Price(market_b.clone(), token.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ArbitrageError::PriceNotFound));

        let price_diff = if price_b.price > price_a.price {
            price_b.price - price_a.price
        } else {
            price_a.price - price_b.price
        };

        // profitable if price_diff > MIN_PROFIT_THRESHOLD_BPS basis points of the lower price
        let lower_price = if price_a.price < price_b.price {
            price_a.price
        } else {
            price_b.price
        };
        let threshold = lower_price * MIN_PROFIT_THRESHOLD_BPS / BPS;
        let is_profitable = price_diff > threshold;

        (price_diff, is_profitable)
    }

    pub fn execute_arbitrage(
        env: Env,
        caller: Address,
        token: Address,
        buy_market: Symbol,
        sell_market: Symbol,
        amount: i128,
    ) -> i128 {
        caller.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, ArbitrageError::InvalidAmount);
        }

        let buy_price: MarketPrice = env
            .storage()
            .persistent()
            .get(&DataKey::Price(buy_market.clone(), token.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ArbitrageError::PriceNotFound));

        let sell_price: MarketPrice = env
            .storage()
            .persistent()
            .get(&DataKey::Price(sell_market.clone(), token.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ArbitrageError::PriceNotFound));

        if sell_price.price <= buy_price.price {
            panic_with_error!(&env, ArbitrageError::NotProfitable);
        }

        let price_diff = sell_price.price - buy_price.price;
        let threshold = buy_price.price * MIN_PROFIT_THRESHOLD_BPS / BPS;
        if price_diff <= threshold {
            panic_with_error!(&env, ArbitrageError::NotProfitable);
        }

        // profit = amount * (sell_price - buy_price) / buy_price
        let profit = amount * price_diff / buy_price.price;

        let record_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::RecordCount)
            .unwrap_or(0u64)
            + 1;

        let record = ArbitrageRecord {
            id: record_id,
            executor: caller.clone(),
            token: token.clone(),
            buy_market: buy_market.clone(),
            sell_market: sell_market.clone(),
            profit,
            executed_at: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Record(record_id), &record);
        env.storage()
            .persistent()
            .set(&DataKey::RecordCount, &record_id);

        let mut executor_records: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::ExecutorRecords(caller.clone()))
            .unwrap_or(Vec::new(&env));
        executor_records.push_back(record_id);
        env.storage()
            .persistent()
            .set(&DataKey::ExecutorRecords(caller.clone()), &executor_records);

        env.events().publish(
            (symbol_short!("arb"), symbol_short!("detected")),
            (token.clone(), buy_market, sell_market, price_diff),
        );

        env.events().publish(
            (symbol_short!("arb"), symbol_short!("executed")),
            (caller, token, profit),
        );

        profit
    }

    pub fn get_performance(env: Env, executor: Address) -> Vec<ArbitrageRecord> {
        let ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::ExecutorRecords(executor))
            .unwrap_or(Vec::new(&env));

        let mut result = Vec::new(&env);
        for id in ids.iter() {
            if let Some(record) = env
                .storage()
                .persistent()
                .get::<DataKey, ArbitrageRecord>(&DataKey::Record(id))
            {
                result.push_back(record);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env, Symbol};

    fn setup() -> (Env, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ArbitrageContract, ());
        (env, contract_id)
    }

    #[test]
    fn test_detect_opportunity_profitable() {
        let (env, contract_id) = setup();
        let client = ArbitrageContractClient::new(&env, &contract_id);

        let feeder = Address::generate(&env);
        let token = Address::generate(&env);
        let market_a = Symbol::new(&env, "MKTX");
        let market_b = Symbol::new(&env, "MKTY");

        client.update_price(&feeder, &market_a, &token, &1000i128);
        client.update_price(&feeder, &market_b, &token, &1200i128); // 20% spread > 1%

        let (diff, profitable) = client.detect_opportunity(&token, &market_a, &market_b);
        assert_eq!(diff, 200);
        assert!(profitable);
    }

    #[test]
    fn test_detect_opportunity_not_profitable() {
        let (env, contract_id) = setup();
        let client = ArbitrageContractClient::new(&env, &contract_id);

        let feeder = Address::generate(&env);
        let token = Address::generate(&env);
        let market_a = Symbol::new(&env, "MKTX");
        let market_b = Symbol::new(&env, "MKTY");

        client.update_price(&feeder, &market_a, &token, &1000i128);
        client.update_price(&feeder, &market_b, &token, &1005i128); // 0.5% spread < 1%

        let (diff, profitable) = client.detect_opportunity(&token, &market_a, &market_b);
        assert_eq!(diff, 5);
        assert!(!profitable);
    }

    #[test]
    fn test_execute_arbitrage_stores_record_and_emits_event() {
        let (env, contract_id) = setup();
        let client = ArbitrageContractClient::new(&env, &contract_id);

        let feeder = Address::generate(&env);
        let executor = Address::generate(&env);
        let token = Address::generate(&env);
        let buy_market = Symbol::new(&env, "MKTX");
        let sell_market = Symbol::new(&env, "MKTY");

        client.update_price(&feeder, &buy_market, &token, &1000i128);
        client.update_price(&feeder, &sell_market, &token, &1200i128);

        let profit =
            client.execute_arbitrage(&executor, &token, &buy_market, &sell_market, &100i128);
        assert!(profit > 0);

        let records = client.get_performance(&executor);
        assert_eq!(records.len(), 1);
        assert_eq!(records.get(0).unwrap().profit, profit);
    }

    #[test]
    #[should_panic]
    fn test_execute_arbitrage_fails_when_not_profitable() {
        let (env, contract_id) = setup();
        let client = ArbitrageContractClient::new(&env, &contract_id);

        let feeder = Address::generate(&env);
        let executor = Address::generate(&env);
        let token = Address::generate(&env);
        let buy_market = Symbol::new(&env, "MKTX");
        let sell_market = Symbol::new(&env, "MKTY");

        // sell <= buy: not profitable
        client.update_price(&feeder, &buy_market, &token, &1000i128);
        client.update_price(&feeder, &sell_market, &token, &1000i128);

        client.execute_arbitrage(&executor, &token, &buy_market, &sell_market, &100i128);
    }

    #[test]
    fn test_get_performance_returns_correct_records() {
        let (env, contract_id) = setup();
        let client = ArbitrageContractClient::new(&env, &contract_id);

        let feeder = Address::generate(&env);
        let executor = Address::generate(&env);
        let other = Address::generate(&env);
        let token = Address::generate(&env);
        let buy_market = Symbol::new(&env, "MKTX");
        let sell_market = Symbol::new(&env, "MKTY");

        client.update_price(&feeder, &buy_market, &token, &1000i128);
        client.update_price(&feeder, &sell_market, &token, &1200i128);

        client.execute_arbitrage(&executor, &token, &buy_market, &sell_market, &100i128);
        client.execute_arbitrage(&executor, &token, &buy_market, &sell_market, &200i128);
        client.execute_arbitrage(&other, &token, &buy_market, &sell_market, &50i128);

        let executor_records = client.get_performance(&executor);
        assert_eq!(executor_records.len(), 2);

        let other_records = client.get_performance(&other);
        assert_eq!(other_records.len(), 1);
    }
}
