#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address, Ledger},
    vec, Address as SdkAddress, Env
};
use tipjar::{royalty::SplitRecipient, TipJarContract, TipJarContractClient};

fn setup() -> (Env, TipJarContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TipJarContract);
    let client = TipJarContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract(token_admin);
    client.init(&admin);
    client.add_token(&admin, &token_id);

    (env, client, admin)
}

// ── set_split ────────────────────────────────────────────────────────────────

#[test]
fn test_set_split_stores_config() {
    let (env, client, _) = setup();
    let split_id = Address::generate(&env);
    let owner = Address::generate(&env);
    let a = Address::generate(&env);
    let b = Address::generate(&env);

    let recipients = vec![
        &env,
        SplitRecipient {
            recipient: a.clone(),
            share_bps: 6_000,
        },
        SplitRecipient {
            recipient: b.clone(),
            share_bps: 4_000,
        },
    ];
    client.set_split(&split_id, &owner, &recipients);

    let cfg = client.get_split(&split_id).expect("config should exist");
    assert_eq!(cfg.owner, owner);
    assert_eq!(cfg.recipients.len(), 2);
}

#[test]
fn test_set_split_no_config_returns_none() {
    let (env, client, _) = setup();
    let split_id = Address::generate(&env);
    assert!(client.get_split(&split_id).is_none());
}

// ── modify_split ─────────────────────────────────────────────────────────────

#[test]
fn test_modify_split_updates_recipients() {
    let (env, client, _) = setup();
    let split_id = Address::generate(&env);
    let owner = Address::generate(&env);
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);

    client.set_split(
        &split_id,
        &owner,
        &vec![
            &env,
            SplitRecipient {
                recipient: a.clone(),
                share_bps: 5_000,
            },
            SplitRecipient {
                recipient: b.clone(),
                share_bps: 5_000,
            },
        ],
    );

    client.modify_split(
        &split_id,
        &owner,
        &vec![
            &env,
            SplitRecipient {
                recipient: a.clone(),
                share_bps: 3_000,
            },
            SplitRecipient {
                recipient: b.clone(),
                share_bps: 3_000,
            },
            SplitRecipient {
                recipient: c.clone(),
                share_bps: 4_000,
            },
        ],
    );

    let cfg = client.get_split(&split_id).unwrap();
    assert_eq!(cfg.recipients.len(), 3);
}

// ── distribute_split ─────────────────────────────────────────────────────────

#[test]
fn test_distribute_credits_balances_proportionally() {
    let (env, client, _) = setup();
    let split_id = Address::generate(&env);
    let owner = Address::generate(&env);
    let a = Address::generate(&env);
    let b = Address::generate(&env);

    client.set_split(
        &split_id,
        &owner,
        &vec![
            &env,
            SplitRecipient {
                recipient: a.clone(),
                share_bps: 7_000,
            },
            SplitRecipient {
                recipient: b.clone(),
                share_bps: 3_000,
            },
        ],
    );

    client.distribute_split(&split_id, &1_000i128);

    assert_eq!(client.get_split_balance(&a), 700);
    assert_eq!(client.get_split_balance(&b), 300);
}

#[test]
fn test_distribute_no_config_returns_zero() {
    let (env, client, _) = setup();
    let split_id = Address::generate(&env);
    let result = client.distribute_split(&split_id, &500i128);
    assert_eq!(result, 0);
}

// ── nested splits ─────────────────────────────────────────────────────────────

#[test]
fn test_nested_split_distributes_recursively() {
    let (env, client, _) = setup();
    let owner = Address::generate(&env);
    let team = Address::generate(&env); // inner split
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);

    // Inner split: team → a 50%, b 50%
    client.set_split(
        &team,
        &owner,
        &vec![
            &env,
            SplitRecipient {
                recipient: a.clone(),
                share_bps: 5_000,
            },
            SplitRecipient {
                recipient: b.clone(),
                share_bps: 5_000,
            },
        ],
    );

    // Outer split: c 40%, team 60%
    let outer = Address::generate(&env);
    client.set_split(
        &outer,
        &owner,
        &vec![
            &env,
            SplitRecipient {
                recipient: c.clone(),
                share_bps: 4_000,
            },
            SplitRecipient {
                recipient: team.clone(),
                share_bps: 6_000,
            },
        ],
    );

    client.distribute_split(&outer, &1_000i128);

    // c gets 400; team's 600 is split 50/50 → a=300, b=300
    assert_eq!(client.get_split_balance(&c), 400);
    assert_eq!(client.get_split_balance(&a), 300);
    assert_eq!(client.get_split_balance(&b), 300);
}

// ── history ───────────────────────────────────────────────────────────────────

#[test]
fn test_history_count_increments_on_distribute() {
    let (env, client, _) = setup();
    let split_id = Address::generate(&env);
    let owner = Address::generate(&env);
    let a = Address::generate(&env);

    client.set_split(
        &split_id,
        &owner,
        &vec![
            &env,
            SplitRecipient {
                recipient: a.clone(),
                share_bps: 10_000,
            },
        ],
    );

    assert_eq!(client.get_split_history_count(&split_id), 0);
    client.distribute_split(&split_id, &100i128);
    assert_eq!(client.get_split_history_count(&split_id), 1);
    client.distribute_split(&split_id, &200i128);
    assert_eq!(client.get_split_history_count(&split_id), 2);
}

#[test]
fn test_history_entry_records_amount() {
    let (env, client, _) = setup();
    let split_id = Address::generate(&env);
    let owner = Address::generate(&env);
    let a = Address::generate(&env);

    client.set_split(
        &split_id,
        &owner,
        &vec![
            &env,
            SplitRecipient {
                recipient: a.clone(),
                share_bps: 10_000,
            },
        ],
    );

    client.distribute_split(&split_id, &500i128);

    let entry = client
        .get_split_history_entry(&split_id, &0u64)
        .expect("entry should exist");
    assert_eq!(entry.amount, 500);
}

#[test]
fn test_history_entry_out_of_range_returns_none() {
    let (env, client, _) = setup();
    let split_id = Address::generate(&env);
    assert!(client.get_split_history_entry(&split_id, &99u64).is_none());
}
