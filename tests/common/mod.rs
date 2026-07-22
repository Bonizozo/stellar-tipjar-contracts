//! Shared harness for the tipjar circuit-breaker integration tests
//! (`tests/pause_tests.rs`, `tests/partial_pause_tests.rs`).
//!
//! These files live at the repo root rather than under
//! `contracts/tipjar/tests/` for historical reasons (they predate the
//! `contracts/` restructuring), but they're compiled as part of the `tipjar`
//! package via explicit `[[test]]` entries in `contracts/tipjar/Cargo.toml`
//! and exercise the real `tipjar` crate — not scaffolding.
//!
//! `mod common;` is compiled fresh into each `[[test]]` binary, so an item
//! only one of the two test files uses is still "dead code" from the other
//! binary's point of view; allowed wholesale rather than cfg-gating each item.
#![allow(dead_code)]

use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, EnvTestConfig, Ledger as _},
    token, Address, Env, MuxedAddress,
};
use tipjar::{TipJar, TipJarClient};

/// A test `Env` with snapshot-file capture disabled — unlike
/// `contracts/tipjar/src/test.rs`'s golden-fixture test, none of these tests
/// assert against a snapshot file, so skip writing one (matches
/// `contracts/tipjar/src/test_invariants.rs`'s harness).
pub fn fresh_env() -> Env {
    let mut env = Env::default();
    env.set_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    env.mock_all_auths();
    env
}

pub struct TestContext {
    pub env: Env,
    pub contract_id: Address,
    pub admin: Address,
    pub guardian: Address,
    pub token: Address,
}

impl TestContext {
    /// Fresh contract, initialized with a real SEP-41 token (a Stellar Asset
    /// Contract), an admin, and a guardian appointed by that admin.
    pub fn new() -> Self {
        let env = fresh_env();

        let token_admin = Address::generate(&env);
        let token = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();

        Self::with_token(env, token)
    }

    /// Like `new`, but with a caller-supplied token address — used to swap in
    /// a non-conforming/malicious token contract for pause-check-ordering tests.
    pub fn with_token(env: Env, token: Address) -> Self {
        let admin = Address::generate(&env);
        let contract_id = env.register(TipJar, ());
        let client = TipJarClient::new(&env, &contract_id);
        client.init(&token, &admin, &1000);

        let guardian = Address::generate(&env);
        client.set_guardian(&admin, &guardian);

        TestContext {
            env,
            contract_id,
            admin,
            guardian,
            token,
        }
    }

    pub fn client(&self) -> TipJarClient<'_> {
        TipJarClient::new(&self.env, &self.contract_id)
    }

    pub fn token_client(&self) -> token::TokenClient<'_> {
        token::TokenClient::new(&self.env, &self.token)
    }

    pub fn create_user(&self) -> Address {
        Address::generate(&self.env)
    }

    pub fn create_creator(&self) -> Address {
        Address::generate(&self.env)
    }

    pub fn mint_tokens(&self, user: &Address, amount: i128) {
        token::StellarAssetClient::new(&self.env, &self.token).mint(user, &amount);
    }

    pub fn advance_ledger(&self, ledgers: u32) {
        self.env
            .ledger()
            .with_mut(|li| li.sequence_number += ledgers);
    }

    pub fn ledger_sequence(&self) -> u32 {
        self.env.ledger().sequence()
    }
}

/// Unwraps a `try_*` client call down to the host-level error code a
/// contract panic surfaces as, panicking on success or on a genuine
/// host/network-level error. Compare the result against `tipjar::Error::X.into()`,
/// matching this crate's existing test idiom (see `contracts/tipjar/src/test_exhaustive.rs`).
pub fn expect_error<T: core::fmt::Debug, E: core::fmt::Debug>(
    result: Result<Result<T, E>, Result<soroban_sdk::Error, soroban_sdk::InvokeError>>,
) -> soroban_sdk::Error {
    match result {
        Ok(ok) => panic!("expected error, call succeeded with {:?}", ok),
        Err(Ok(e)) => e,
        Err(Err(host_err)) => panic!("unexpected host-level error: {:?}", host_err),
    }
}

#[contracttype]
enum MaliciousTokenKey {
    TransferCalls,
}

/// A token whose `transfer` unconditionally "succeeds" — no balance check,
/// no auth check, no error path at all — standing in for a compromised or
/// malicious SEP-41 token that ignores errors. Used to prove that a paused
/// tipjar entrypoint never reaches the token transfer at all, rather than
/// relying on the token to correctly reject it.
#[contract]
pub struct MaliciousToken;

#[contractimpl]
impl MaliciousToken {
    pub fn transfer(env: Env, _from: Address, _to: MuxedAddress, _amount: i128) {
        let calls: u32 = env
            .storage()
            .instance()
            .get(&MaliciousTokenKey::TransferCalls)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&MaliciousTokenKey::TransferCalls, &(calls + 1));
    }

    pub fn transfer_calls(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&MaliciousTokenKey::TransferCalls)
            .unwrap_or(0)
    }
}
