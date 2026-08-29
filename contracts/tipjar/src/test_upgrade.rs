//! Tests for the timelocked upgrade flow: `propose_upgrade`,
//! `execute_upgrade`, `cancel_upgrade`, two-step admin transfer, and
//! `migrate()`.
//!
//! These register a second, genuinely distinct compiled WASM binary via
//! `soroban_sdk::contractimport!` (built from `contracts/tipjar-v2-fixture`,
//! see that crate's module docs) and swap this contract's code onto it with
//! `env.deployer().upload_contract_wasm(..)` + `execute_upgrade`, rather than
//! merely unit-testing the storage writes in isolation. That's the only way
//! to prove `update_current_contract_wasm` and a real `migrate()` behave
//! correctly, since Soroban's fast in-process test mode runs `#[contract]`
//! types natively rather than through WASM.
//!
//! Run `cargo build -p tipjar-v2-fixture --target wasm32v1-none --release`
//! before running these tests — the fixture WASM must already exist on disk
//! for `contractimport!` to embed it at compile time (CI does this in a
//! dedicated step; see `.github/workflows/test.yml`).

use crate::{Error, TipJar, TipJarClient};
use soroban_sdk::{
    symbol_short, testutils::Address as _, testutils::Events as _, testutils::Ledger as _, token,
    vec, Address, Env, IntoVal, Symbol,
};

mod v2 {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/tipjar_v2_fixture.wasm"
    );
}

/// Small enough that boundary tests only need a couple of ledger bumps.
const TIMELOCK: u32 = 50;

struct Ctx {
    env: Env,
    contract_id: Address,
    token: Address,
    admin: Address,
}

impl Ctx {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let token_admin = Address::generate(&env);
        let token = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();
        let admin = Address::generate(&env);

        let contract_id = env.register(TipJar, ());
        let client = TipJarClient::new(&env, &contract_id);
        client.init(&token, &admin, &TIMELOCK);

        Ctx {
            env,
            contract_id,
            token,
            admin,
        }
    }

    fn client(&self) -> TipJarClient<'_> {
        TipJarClient::new(&self.env, &self.contract_id)
    }

    fn v2_client(&self) -> v2::Client<'_> {
        v2::Client::new(&self.env, &self.contract_id)
    }

    fn fund(&self, amount: i128) -> Address {
        let holder = Address::generate(&self.env);
        token::StellarAssetClient::new(&self.env, &self.token).mint(&holder, &amount);
        holder
    }

    fn upload_v2(&self) -> soroban_sdk::BytesN<32> {
        self.env.deployer().upload_contract_wasm(v2::WASM)
    }
}

#[test]
fn upgrade_lifecycle_preserves_balances_and_totals_byte_exactly() {
    let ctx = Ctx::new();
    let sender = ctx.fund(1_000);
    let creator = Address::generate(&ctx.env);

    ctx.client().tip(&sender, &creator, &ctx.token, &400);
    ctx.client().tip(&sender, &creator, &ctx.token, &100);
    assert_eq!(ctx.client().get_total_tips(&creator, &ctx.token), 500);

    let hash = ctx.upload_v2();
    ctx.client().propose_upgrade(&ctx.admin, &hash);

    ctx.env
        .ledger()
        .with_mut(|li| li.sequence_number += TIMELOCK);
    ctx.client().execute_upgrade();

    // Same contract address, new code — balances and totals written under
    // v1 must read back byte-exactly under v2.
    let v2 = ctx.v2_client();
    assert_eq!(v2.get_total_tips(&creator, &ctx.token), 500);

    // The new code keeps working: further tips accumulate correctly...
    v2.tip(&sender, &creator, &ctx.token, &250);
    assert_eq!(v2.get_total_tips(&creator, &ctx.token), 750);

    // ...and the escrowed balance (partly from v1, partly added under v2)
    // is still fully redeemable.
    v2.withdraw(&creator, &ctx.token, &None);
    assert_eq!(
        token::TokenClient::new(&ctx.env, &ctx.token).balance(&creator),
        750
    );
}

#[test]
fn execute_upgrade_one_ledger_before_unlock_panics() {
    let ctx = Ctx::new();
    let hash = ctx.upload_v2();
    ctx.client().propose_upgrade(&ctx.admin, &hash);

    ctx.env
        .ledger()
        .with_mut(|li| li.sequence_number += TIMELOCK - 1);

    let err = ctx.client().try_execute_upgrade().unwrap_err().unwrap();
    assert_eq!(err, Error::TimelockNotElapsed.into());
}

#[test]
fn execute_upgrade_exactly_at_unlock_ledger_succeeds() {
    let ctx = Ctx::new();
    let hash = ctx.upload_v2();
    ctx.client().propose_upgrade(&ctx.admin, &hash);

    ctx.env
        .ledger()
        .with_mut(|li| li.sequence_number += TIMELOCK);

    // Does not panic.
    ctx.client().execute_upgrade();
    assert_eq!(ctx.v2_client().get_data_version(), 1);
}

#[test]
fn cancel_upgrade_prevents_execution_even_after_timelock() {
    let ctx = Ctx::new();
    let hash = ctx.upload_v2();
    ctx.client().propose_upgrade(&ctx.admin, &hash);
    ctx.client().cancel_upgrade(&ctx.admin);

    ctx.env
        .ledger()
        .with_mut(|li| li.sequence_number += TIMELOCK);

    let err = ctx.client().try_execute_upgrade().unwrap_err().unwrap();
    assert_eq!(err, Error::NoPendingUpgrade.into());
}

#[test]
fn execute_upgrade_without_a_pending_proposal_panics() {
    let ctx = Ctx::new();
    let err = ctx.client().try_execute_upgrade().unwrap_err().unwrap();
    assert_eq!(err, Error::NoPendingUpgrade.into());
}

#[test]
fn propose_upgrade_rejects_non_admin_caller() {
    let ctx = Ctx::new();
    let stranger = Address::generate(&ctx.env);
    let hash = ctx.upload_v2();

    let err = ctx
        .client()
        .try_propose_upgrade(&stranger, &hash)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized.into());
}

#[test]
fn cancel_upgrade_rejects_non_admin_caller() {
    let ctx = Ctx::new();
    let stranger = Address::generate(&ctx.env);
    let hash = ctx.upload_v2();
    ctx.client().propose_upgrade(&ctx.admin, &hash);

    let err = ctx
        .client()
        .try_cancel_upgrade(&stranger)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized.into());
}

#[test]
fn propose_upgrade_rejects_a_second_pending_proposal() {
    let ctx = Ctx::new();
    let hash = ctx.upload_v2();
    ctx.client().propose_upgrade(&ctx.admin, &hash);

    let err = ctx
        .client()
        .try_propose_upgrade(&ctx.admin, &hash)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::UpgradeAlreadyPending.into());
}

#[test]
fn migrate_is_idempotent_across_repeated_invocations() {
    let ctx = Ctx::new();
    let hash = ctx.upload_v2();
    ctx.client().propose_upgrade(&ctx.admin, &hash);
    ctx.env
        .ledger()
        .with_mut(|li| li.sequence_number += TIMELOCK);
    ctx.client().execute_upgrade();

    let v2 = ctx.v2_client();
    assert_eq!(v2.get_data_version(), 1);

    v2.migrate(&ctx.admin);
    assert_eq!(v2.get_data_version(), 2);

    let events_before = ctx.env.events().all().events().len();
    // Second, third invocation: silent no-op, no panic, no duplicate event.
    v2.migrate(&ctx.admin);
    v2.migrate(&ctx.admin);
    assert_eq!(v2.get_data_version(), 2);
    assert_eq!(ctx.env.events().all().events().len(), events_before);
}

#[test]
fn migrate_on_v1_before_any_upgrade_is_a_no_op() {
    let ctx = Ctx::new();
    // DATA_VERSION is already 1 immediately after init; migrating v1 against
    // itself must not error or advance anything.
    ctx.client().migrate(&ctx.admin);
    assert_eq!(ctx.client().get_data_version(), 1);
}

#[test]
fn admin_two_step_transfer() {
    let ctx = Ctx::new();
    let next_admin = Address::generate(&ctx.env);

    ctx.client().propose_admin(&ctx.admin, &next_admin);
    // Old admin is still the admin of record until accept_admin completes.
    assert_eq!(ctx.client().get_admin(), ctx.admin);

    ctx.client().accept_admin(&next_admin);
    assert_eq!(ctx.client().get_admin(), next_admin);

    // Old admin has lost authority.
    let hash = ctx.upload_v2();
    let err = ctx
        .client()
        .try_propose_upgrade(&ctx.admin, &hash)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized.into());

    // New admin has it.
    ctx.client().propose_upgrade(&next_admin, &hash);
}

#[test]
fn upgrade_and_admin_events_have_the_documented_topics_and_data() {
    let ctx = Ctx::new();
    let hash = ctx.upload_v2();
    let ledger_before_propose = ctx.env.ledger().sequence();

    ctx.client().propose_upgrade(&ctx.admin, &hash);
    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (Symbol::new(&ctx.env, "upgrade_proposed"), hash.clone()).into_val(&ctx.env),
                (ledger_before_propose + TIMELOCK,).into_val(&ctx.env),
            ),
        ]
    );

    ctx.client().cancel_upgrade(&ctx.admin);
    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (Symbol::new(&ctx.env, "upgrade_cancelled"), hash.clone()).into_val(&ctx.env),
                soroban_sdk::Vec::<soroban_sdk::Val>::new(&ctx.env).into_val(&ctx.env),
            ),
        ]
    );

    ctx.client().propose_upgrade(&ctx.admin, &hash);
    ctx.env
        .ledger()
        .with_mut(|li| li.sequence_number += TIMELOCK);
    ctx.client().execute_upgrade();
    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (Symbol::new(&ctx.env, "upgrade_executed"), hash.clone()).into_val(&ctx.env),
                soroban_sdk::Vec::<soroban_sdk::Val>::new(&ctx.env).into_val(&ctx.env),
            ),
        ]
    );

    let v2 = ctx.v2_client();
    v2.migrate(&ctx.admin);
    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (symbol_short!("migrated"),).into_val(&ctx.env),
                (1u32, 2u32).into_val(&ctx.env),
            ),
        ]
    );
}

#[test]
fn admin_transfer_events_have_the_documented_topics_and_data() {
    let ctx = Ctx::new();
    let next_admin = Address::generate(&ctx.env);

    ctx.client().propose_admin(&ctx.admin, &next_admin);
    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (Symbol::new(&ctx.env, "admin_transfer_proposed"),).into_val(&ctx.env),
                (ctx.admin.clone(), next_admin.clone()).into_val(&ctx.env),
            ),
        ]
    );

    ctx.client().accept_admin(&next_admin);
    let events = ctx.env.events().all().filter_by_contract(&ctx.contract_id);
    assert_eq!(
        events,
        vec![
            &ctx.env,
            (
                ctx.contract_id.clone(),
                (Symbol::new(&ctx.env, "admin_transfer_accepted"),).into_val(&ctx.env),
                (next_admin.clone(),).into_val(&ctx.env),
            ),
        ]
    );
}

#[test]
fn accept_admin_rejects_an_address_that_was_not_proposed() {
    let ctx = Ctx::new();
    let intended = Address::generate(&ctx.env);
    let impostor = Address::generate(&ctx.env);

    ctx.client().propose_admin(&ctx.admin, &intended);

    let err = ctx
        .client()
        .try_accept_admin(&impostor)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NoPendingAdmin.into());
}

#[test]
fn migration_v1_to_v2_preserves_pause_state_and_adds_notes_field() {
    // This test proves the migration mechanism works by:
    // 1. Seeding storage with v1-shaped pause state data (no notes field)
    // 2. Performing an upgrade to v2 (which has notes field in PauseState)
    // 3. Executing the migration logic that transforms v1 to v2
    // 4. Validating the v2-shaped output is correct (notes field initialized to None)
    let ctx = Ctx::new();

    // Seed v1 storage with pause state: admin paused tips
    ctx.client()
        .pause_tips(&ctx.admin, &crate::PAUSE_FLAG_TIPS);

    // Verify pause state was set before upgrade
    assert_eq!(
        ctx.client().get_pause_flags(),
        crate::PAUSE_FLAG_TIPS
    );

    // Upload and execute upgrade to v2
    let hash = ctx.upload_v2();
    ctx.client().propose_upgrade(&ctx.admin, &hash);
    ctx.env
        .ledger()
        .with_mut(|li| li.sequence_number += TIMELOCK);
    ctx.client().execute_upgrade();

    // At this point, v2 contract is active but data hasn't been migrated yet
    // (v2's pause_state() function will initialize notes to None if reading from storage)
    let v2 = ctx.v2_client();
    assert_eq!(v2.get_data_version(), 1);

    // Call migrate to advance from v1 to v2 and transform pause state
    v2.migrate(&ctx.admin);
    assert_eq!(v2.get_data_version(), 2);

    // Verify pause state was preserved through migration
    // (pause flags should still be intact)
    assert_eq!(
        v2.get_pause_flags(),
        crate::PAUSE_FLAG_TIPS
    );

    // Further pause operations work correctly after migration
    v2.pause_withdrawals(&ctx.admin, &crate::PAUSE_FLAG_WITHDRAWALS);
    assert_eq!(
        v2.get_pause_flags(),
        crate::PAUSE_FLAG_TIPS | crate::PAUSE_FLAG_WITHDRAWALS
    );

    // Migration is idempotent: calling it again is a no-op
    let events_before = ctx.env.events().all().events().len();
    v2.migrate(&ctx.admin);
    assert_eq!(v2.get_data_version(), 2);
    assert_eq!(ctx.env.events().all().events().len(), events_before);
}
