//! Condition evaluation logic.

use soroban_sdk::{token, BytesN, Env, Vec};

use crate::DataKey;

use super::types::Condition;

/// Sets or clears an off-chain approval flag.
pub fn set_offchain_approval(env: &Env, condition_id: &BytesN<32>, approved: bool) {
    env.storage()
        .persistent()
        .set(&DataKey::OffchainCondition(condition_id.clone()), &approved);
}

/// Evaluates all conditions using logical AND.
pub fn evaluate_all(env: &Env, conditions: &Vec<Condition>) -> bool {
    for condition in conditions.iter() {
        if !evaluate_one(env, &condition) {
            return false;
        }
    }
    true
}

/// Evaluates a single condition.
pub fn evaluate_one(env: &Env, condition: &Condition) -> bool {
    match condition {
        Condition::Always => true,
        Condition::MinLedger(min_ledger) => env.ledger().sequence() >= *min_ledger,
        Condition::MaxLedger(max_ledger) => env.ledger().sequence() <= *max_ledger,
        Condition::TimeAfter(min_time) => env.ledger().timestamp() >= *min_time,
        Condition::TimeBefore(max_time) => env.ledger().timestamp() <= *max_time,
        Condition::TokenBalanceAtLeast(account, token, min_balance) => {
            let token_client = token::Client::new(env, token);
            token_client.balance(account) >= *min_balance
        }
        Condition::OffchainApproved(condition_id) => env
            .storage()
            .persistent()
            .get(&DataKey::OffchainCondition(condition_id.clone()))
            .unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::{Address, BytesN, Env, Vec};

    use super::{evaluate_all, evaluate_one, set_offchain_approval};
    use crate::conditions::types::Condition;

    #[test]
    fn evaluates_multiple_conditions() {
        let env = Env::default();
        env.ledger().with_mut(|li| {
            li.sequence_number = 100;
            li.timestamp = 1_000;
        });

        let mut conditions = Vec::new(&env);
        conditions.push_back(Condition::MinLedger(50));
        conditions.push_back(Condition::TimeAfter(999));
        conditions.push_back(Condition::MaxLedger(200));

        assert!(evaluate_all(&env, &conditions));
    }

    #[test]
    fn fails_on_false_condition() {
        let env = Env::default();
        env.ledger().with_mut(|li| {
            li.sequence_number = 25;
            li.timestamp = 100;
        });

        let mut conditions = Vec::new(&env);
        conditions.push_back(Condition::MinLedger(26));
        conditions.push_back(Condition::TimeAfter(50));

        assert!(!evaluate_all(&env, &conditions));
    }

    #[test]
    fn evaluates_offchain_approval() {
        let env = Env::default();
        let condition_id = BytesN::from_array(&env, &[7; 32]);

        assert!(!evaluate_one(
            &env,
            &Condition::OffchainApproved(condition_id.clone())
        ));

        set_offchain_approval(&env, &condition_id, true);

        assert!(evaluate_one(&env, &Condition::OffchainApproved(condition_id)));
    }

    #[test]
    fn supports_condition_combinations() {
        let env = Env::default();
        let _unused = Address::generate(&env);

        env.ledger().with_mut(|li| {
            li.sequence_number = 500;
            li.timestamp = 5_000;
        });

        let condition_id = BytesN::from_array(&env, &[9; 32]);
        set_offchain_approval(&env, &condition_id, true);

        let mut conditions = Vec::new(&env);
        conditions.push_back(Condition::Always);
        conditions.push_back(Condition::MinLedger(500));
        conditions.push_back(Condition::TimeBefore(5_100));
        conditions.push_back(Condition::OffchainApproved(condition_id));

        assert!(evaluate_all(&env, &conditions));
    }
}
