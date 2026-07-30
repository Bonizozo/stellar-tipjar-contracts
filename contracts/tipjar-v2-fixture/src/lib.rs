#![no_std]

//! A stand-in "v2" contract used only to exercise `contracts/tipjar`'s
//! upgrade flow against a genuinely distinct, precompiled WASM binary — see
//! `contracts/tipjar/src/test_upgrade.rs`. Soroban's test environment runs
//! `#[contract]` types natively rather than through WASM, so proving that
//! `execute_upgrade`'s `update_current_contract_wasm` call and a real
//! `migrate()` behave correctly requires a second crate actually compiled to
//! `wasm32v1-none` and registered via `soroban_sdk::contractimport!`, not
//! just a second Rust type in the same test binary.
//!
//! This crate deliberately does **not** depend on `contracts/tipjar` as a
//! library: `#[contractimpl]` exports every function as a forced WASM
//! symbol, so linking another contract's compiled code into this one — even
//! just to reuse a type — pulls those exports along and collides with this
//! crate's own (`cargo build` fails with "symbol multiply defined"). A real
//! v2 contract living in its own crate/repo would face the same constraint,
//! so `DataKey` and `Error` below are independent, storage-compatible
//! copies of the subset `tipjar::DataKey` / `tipjar::Error` this fixture
//! touches — Soroban encodes `contracttype` enum variants by name, so an
//! independently-declared enum with matching variant names and field shapes
//! reads and writes the exact same storage entries.
//!
//! `tip` and `get_total_tips` are copied from `tipjar::TipJar` essentially
//! unchanged (proving storage written under v1 reads back correctly under
//! different compiled code); `withdraw` is deliberately reduced to the
//! creator-pays-self path (operator delegation and payout-address maturation
//! aren't re-tested here — they're already covered against v1 in
//! `contracts/tipjar/src/test_exhaustive.rs`, and a real v2 dropping or
//! reworking a feature is exactly the kind of change this upgrade mechanism
//! needs to support). `migrate` is the genuinely new piece of behavior: it
//! advances `DataKey::DataVersion` from 1 to 2.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Env, MuxedAddress,
};

/// Storage-compatible subset of `tipjar::DataKey` — see module docs. Matches
/// v1's current (creator, token)-keyed multi-token storage shape, since v1
/// is the multi-token contract this fixture upgrades from.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Token,
    Balance(Address, Address),
    Total(Address, Address),
    Admin,
    DataVersion,
}

/// Storage-compatible subset of `tipjar::Error`'s discriminants relevant to
/// this fixture's own functions.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 2,
    InvalidAmount = 3,
    NothingToWithdraw = 4,
    Unauthorized = 11,
}

/// Matches `tipjar::LEDGER_THRESHOLD` / `LEDGER_BUMP` — internal tuning
/// knobs, not part of the storage contract that has to stay in sync across
/// versions.
const LEDGER_THRESHOLD: u32 = 100_000;
const LEDGER_BUMP: u32 = 120_960;

/// Storage schema version this fixture expects — one step ahead of the
/// deployed v1 contract's `tipjar::DATA_VERSION` of 1.
const DATA_VERSION: u32 = 2;

/// Topics `("migrated",)`, data `(from_version, to_version)`.
#[contractevent(data_format = "vec")]
pub struct Migrated {
    from_version: u32,
    to_version: u32,
}

#[contract]
pub struct TipJarV2Fixture;

#[contractimpl]
impl TipJarV2Fixture {
    /// Unchanged from v1 — proves tips placed before the upgrade keep
    /// accumulating correctly under the new WASM.
    pub fn tip(env: Env, sender: Address, creator: Address, token: Address, amount: i128) {
        sender.require_auth();

        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }

        let contract_address = env.current_contract_address();

        token::TokenClient::new(&env, &token).transfer(
            &sender,
            MuxedAddress::from(contract_address),
            &amount,
        );

        let balance_key = DataKey::Balance(creator.clone(), token.clone());
        let total_key = DataKey::Total(creator.clone(), token.clone());

        let balance: i128 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        let total: i128 = env.storage().persistent().get(&total_key).unwrap_or(0);

        let new_balance = balance
            .checked_add(amount)
            .unwrap_or_else(|| panic_with_error!(&env, Error::InvalidAmount));
        let new_total = total
            .checked_add(amount)
            .unwrap_or_else(|| panic_with_error!(&env, Error::InvalidAmount));

        env.storage().persistent().set(&balance_key, &new_balance);
        env.storage().persistent().set(&total_key, &new_total);
        env.storage()
            .persistent()
            .extend_ttl(&balance_key, LEDGER_THRESHOLD, LEDGER_BUMP);
        env.storage()
            .persistent()
            .extend_ttl(&total_key, LEDGER_THRESHOLD, LEDGER_BUMP);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
    }

    /// Reduced from v1: always pays the creator's own full-or-partial
    /// balance to themselves. See the module doc for why.
    pub fn withdraw(env: Env, creator: Address, token: Address, amount: Option<i128>) {
        creator.require_auth();

        let balance_key = DataKey::Balance(creator.clone(), token.clone());
        let balance: i128 = env.storage().persistent().get(&balance_key).unwrap_or(0);

        if balance == 0 {
            panic_with_error!(&env, Error::NothingToWithdraw);
        }

        let amount_to_withdraw = amount.unwrap_or(balance);
        if amount_to_withdraw <= 0 || amount_to_withdraw > balance {
            panic_with_error!(&env, Error::InvalidAmount);
        }

        let contract_address = env.current_contract_address();

        token::TokenClient::new(&env, &token).transfer(
            &contract_address,
            MuxedAddress::from(creator.clone()),
            &amount_to_withdraw,
        );

        let new_balance = balance - amount_to_withdraw;
        env.storage().persistent().set(&balance_key, &new_balance);
        env.storage()
            .persistent()
            .extend_ttl(&balance_key, LEDGER_THRESHOLD, LEDGER_BUMP);
        env.storage()
            .instance()
            .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
    }

    pub fn get_total_tips(env: Env, creator: Address, token: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Total(creator, token))
            .unwrap_or(0)
    }

    pub fn get_data_version(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::DataVersion)
            .unwrap_or(1)
    }

    /// The storage migration this "v2" introduces: advances `DataVersion`
    /// from 1 to 2. Idempotent and version-gated like v1's own `migrate()`
    /// — a second call once the stored version already reads 2 is a silent
    /// no-op, not a panic.
    pub fn migrate(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotInitialized));
        if admin != stored_admin {
            panic_with_error!(&env, Error::Unauthorized);
        }

        let current: u32 = env
            .storage()
            .instance()
            .get(&DataKey::DataVersion)
            .unwrap_or(1);
        if current >= DATA_VERSION {
            return;
        }

        env.storage()
            .instance()
            .set(&DataKey::DataVersion, &DATA_VERSION);

        Migrated {
            from_version: current,
            to_version: DATA_VERSION,
        }
        .publish(&env);
    }
}
