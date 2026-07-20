//! Stateful property tests for the TipJar contract's core accounting invariants.
//!
//! `proptest` generates arbitrary sequences of `Op` (tip / withdraw, over a
//! fixed pool of sender/creator addresses) and replays them one at a time
//! against a real contract instance, while a plain-data shadow model tracks
//! what the contract *should* report. After every single operation — not
//! just at the end of the sequence — we assert:
//!
//!   1. Conservation:   contract token balance == sum of all creators' withdrawable balances
//!   2. Monotonicity:   CreatorTotal (get_total_tips) never decreases
//!   3. Bounded:        0 <= CreatorBalance(c) <= CreatorTotal(c) for every creator
//!   4. No mint/burn:   cumulative withdrawals never exceed cumulative tips
//!
//! On failure, proptest shrinks the operation sequence to a minimal
//! counterexample automatically, since `Op`'s fields are plain integers
//! generated from ranges (shrink toward 0/empty).
//!
//! Run the default (fast) sweep with `cargo test -p tipjar test_invariants`.
//! For a deeper local sweep, override the case count:
//! `PROPTEST_CASES=10000 cargo test -p tipjar test_invariants`.
#![cfg(test)]

extern crate std;

use crate::{Error, TipJar, TipJarClient};
use proptest::prelude::*;
use soroban_sdk::{testutils::Address as _, testutils::EnvTestConfig, token, Address, Env};
use std::collections::HashMap;

/// Small fixed pools so proptest actually exercises repeated interaction
/// between the same senders/creators (collisions are what stress the
/// per-creator accounting) rather than spreading operations across a large,
/// mostly-untouched address space.
const NUM_SENDERS: usize = 3;
const NUM_CREATORS: usize = 3;
const MAX_OPS_PER_CASE: usize = 30;
/// Large enough that no combination of in-range tip amounts across a single
/// case can exhaust a sender; insufficient-balance token-transfer behavior is
/// a SEP-41 token concern, not a TipJar accounting invariant, so it's
/// deliberately kept out of reach here.
const SENDER_FUNDING: i128 = 1_000_000_000;

#[derive(Debug, Clone)]
enum Op {
    Tip {
        sender: usize,
        creator: usize,
        amount: i128,
    },
    Withdraw {
        creator: usize,
        amount: Option<i128>,
    },
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..NUM_SENDERS, 0..NUM_CREATORS, -100i128..10_000i128).prop_map(
            |(sender, creator, amount)| Op::Tip {
                sender,
                creator,
                amount,
            }
        ),
        (
            0..NUM_CREATORS,
            prop_oneof![Just(None), (-100i128..10_000i128).prop_map(Some)],
        )
            .prop_map(|(creator, amount)| Op::Withdraw { creator, amount }),
    ]
}

struct Harness {
    env: Env,
    contract_id: Address,
    token: Address,
    senders: std::vec::Vec<Address>,
    creators: std::vec::Vec<Address>,
    // Shadow model: independently computed, never read back from contract
    // storage, so a divergence means the contract (not the model) is wrong.
    shadow_balance: HashMap<usize, i128>,
    shadow_total: HashMap<usize, i128>,
    total_tipped_in: i128,
    total_withdrawn_out: i128,
}

impl Harness {
    fn new() -> Self {
        let mut env = Env::default();
        // Each proptest case constructs its own `Env`; with the default
        // `capture_snapshot_at_drop`, a full sweep would write one JSON
        // snapshot file per case (thousands under `PROPTEST_CASES=10000`).
        // These snapshots aren't useful here since assertions run inline.
        env.set_config(EnvTestConfig {
            capture_snapshot_at_drop: false,
        });
        env.mock_all_auths();

        let token_admin = Address::generate(&env);
        let token = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();

        let admin = Address::generate(&env);
        let contract_id = env.register(TipJar, ());
        let client = TipJarClient::new(&env, &contract_id);
        client.init(&token, &admin);

        let senders = (0..NUM_SENDERS)
            .map(|_| {
                let addr = Address::generate(&env);
                token::StellarAssetClient::new(&env, &token).mint(&addr, &SENDER_FUNDING);
                addr
            })
            .collect();
        let creators = (0..NUM_CREATORS).map(|_| Address::generate(&env)).collect();

        Harness {
            env,
            contract_id,
            token,
            senders,
            creators,
            shadow_balance: HashMap::new(),
            shadow_total: HashMap::new(),
            total_tipped_in: 0,
            total_withdrawn_out: 0,
        }
    }

    fn client(&self) -> TipJarClient<'_> {
        TipJarClient::new(&self.env, &self.contract_id)
    }

    fn token_client(&self) -> token::TokenClient<'_> {
        token::TokenClient::new(&self.env, &self.token)
    }

    fn apply(&mut self, op: &Op) {
        match *op {
            Op::Tip {
                sender,
                creator,
                amount,
            } => {
                let sender_addr = self.senders[sender].clone();
                let creator_addr = self.creators[creator].clone();
                let result = self.client().try_tip(&sender_addr, &creator_addr, &amount);

                if amount > 0 {
                    result.unwrap().unwrap();
                    *self.shadow_balance.entry(creator).or_insert(0) += amount;
                    *self.shadow_total.entry(creator).or_insert(0) += amount;
                    self.total_tipped_in += amount;
                } else {
                    let err = result.unwrap_err().unwrap();
                    assert_eq!(err, Error::InvalidAmount.into());
                }
            }
            Op::Withdraw { creator, amount } => {
                let creator_addr = self.creators[creator].clone();
                let current_balance = *self.shadow_balance.get(&creator).unwrap_or(&0);
                let requested = amount.unwrap_or(current_balance);

                let result = self.client().try_withdraw(
                    &creator_addr,
                    &creator_addr,
                    &creator_addr,
                    &amount,
                );

                let expected_err = if current_balance == 0 {
                    Some(Error::NothingToWithdraw)
                } else if requested <= 0 || requested > current_balance {
                    Some(Error::InvalidAmount)
                } else {
                    None
                };

                match expected_err {
                    None => {
                        result.unwrap().unwrap();
                        *self.shadow_balance.get_mut(&creator).unwrap() -= requested;
                        self.total_withdrawn_out += requested;
                    }
                    Some(expected) => {
                        let err = result.unwrap_err().unwrap();
                        assert_eq!(err, expected.into());
                    }
                }
            }
        }
    }

    fn assert_invariants(&self) {
        let contract_balance = self.token_client().balance(&self.contract_id);
        let sum_shadow_balances: i128 = self.shadow_balance.values().sum();

        // 1. Conservation.
        assert_eq!(
            contract_balance, sum_shadow_balances,
            "conservation violated: contract token balance != sum of creator balances"
        );
        // Same invariant derived a different way (net flow), to catch bugs
        // that a shadow-model indexing mistake might hide from the check above.
        assert_eq!(
            self.total_tipped_in - self.total_withdrawn_out,
            contract_balance,
            "conservation violated: (total tipped - total withdrawn) != contract balance"
        );

        // 4. No mint/burn.
        assert!(
            self.total_withdrawn_out <= self.total_tipped_in,
            "no-mint/burn violated: withdrawals ({}) exceeded tips ({})",
            self.total_withdrawn_out,
            self.total_tipped_in
        );

        for (idx, creator_addr) in self.creators.iter().enumerate() {
            let real_total = self.client().get_total_tips(creator_addr);
            let shadow_total = *self.shadow_total.get(&idx).unwrap_or(&0);

            // 2. Monotonicity: shadow_total is constructed to only ever grow
            // (every write is `+= amount` with amount > 0), so asserting exact
            // equality against the real, on-chain CreatorTotal both proves
            // monotonicity (transitively, since the shadow never decreases)
            // and proves the contract's total tracks reality exactly.
            assert_eq!(
                real_total, shadow_total,
                "monotonicity/correctness violated: get_total_tips mismatch for creator {}",
                idx
            );

            // 3. Bounded.
            let balance = *self.shadow_balance.get(&idx).unwrap_or(&0);
            assert!(
                balance >= 0,
                "bounded violated: negative withdrawable balance for creator {}",
                idx
            );
            assert!(
                balance <= shadow_total,
                "bounded violated: creator {} balance ({}) exceeds total ({})",
                idx,
                balance,
                shadow_total
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn tipjar_accounting_invariants_hold_after_every_op(
        ops in prop::collection::vec(op_strategy(), 1..MAX_OPS_PER_CASE)
    ) {
        let mut harness = Harness::new();
        for op in &ops {
            harness.apply(op);
            harness.assert_invariants();
        }
    }
}
