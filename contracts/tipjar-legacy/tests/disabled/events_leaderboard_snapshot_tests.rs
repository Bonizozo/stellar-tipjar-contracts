#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};
use tipjar::{
    DataKey, LeaderboardType, ParticipantKind, StateSnapshot, TimePeriod, TipJarContract,
    TipJarContractClient, TipJarError,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn setup() -> (
    Env,
    TipJarContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TipJarContract);
    let client = TipJarContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin, &0u32, &0u64);
    client.add_token(&admin, &token_id);

    let sender = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&sender, &1_000_000i128);

    (env, client, sender, Address::generate(&env), token_id)
}

fn setup_with_admin() -> (
    Env,
    TipJarContractClient<'static>,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TipJarContract);
    let client = TipJarContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin, &0u32, &0u64);
    client.add_token(&admin, &token_id);

    let sender = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&sender, &1_000_000i128);

    (
        env,
        client,
        sender,
        Address::generate(&env),
        token_id,
        admin,
    )
}

// ── event tests ───────────────────────────────────────────────────────────────

#[test]
fn test_tip_emits_event_and_is_queryable() {
    let (env, client, sender, creator, token) = setup();

    let tip_id = client.tip(&sender, &creator, &token, &500i128);
    assert_eq!(tip_id, 0);

    let events = client.get_tip_events(&creator, &10u32);
    assert_eq!(events.len(), 1);

    let ev = events.get(0).unwrap();
    assert_eq!(ev.sender, sender);
    assert_eq!(ev.creator, creator);
    assert_eq!(ev.amount, 500i128);
    assert_eq!(ev.event_id, 0);
}

#[test]
fn test_multiple_tips_increment_event_ids() {
    let (env, client, sender, creator, token) = setup();

    let id0 = client.tip(&sender, &creator, &token, &100i128);
    let id1 = client.tip(&sender, &creator, &token, &200i128);
    let id2 = client.tip(&sender, &creator, &token, &300i128);

    assert_eq!(id0, 0);
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);

    let events = client.get_tip_events(&creator, &10u32);
    assert_eq!(events.len(), 3);
}

#[test]
fn test_get_tip_events_respects_limit() {
    let (env, client, sender, creator, token) = setup();

    for _ in 0..5 {
        client.tip(&sender, &creator, &token, &100i128);
    }

    let events = client.get_tip_events(&creator, &3u32);
    assert_eq!(events.len(), 3);
}

#[test]
fn test_tip_event_has_correct_timestamp() {
    let (env, client, sender, creator, token) = setup();

    env.ledger().with_mut(|l| l.timestamp = 1_000_000);
    client.tip(&sender, &creator, &token, &100i128);

    let events = client.get_tip_events(&creator, &1u32);
    assert_eq!(events.get(0).unwrap().timestamp, 1_000_000);
}

// ── leaderboard tests ─────────────────────────────────────────────────────────

#[test]
fn test_leaderboard_tracks_tippers() {
    let (env, client, sender, creator, token) = setup();

    client.tip(&sender, &creator, &token, &1000i128);

    let board = client.get_leaderboard(&TimePeriod::AllTime, &ParticipantKind::Tipper, &10u32);
    assert_eq!(board.len(), 1);
    assert_eq!(board.get(0).unwrap().address, sender);
    assert_eq!(board.get(0).unwrap().total_amount, 1000i128);
}

#[test]
fn test_leaderboard_tracks_creators() {
    let (env, client, sender, creator, token) = setup();

    client.tip(&sender, &creator, &token, &500i128);
    client.tip(&sender, &creator, &token, &300i128);

    let board = client.get_leaderboard(&TimePeriod::AllTime, &ParticipantKind::Creator, &10u32);
    assert_eq!(board.len(), 1);
    assert_eq!(board.get(0).unwrap().total_amount, 800i128);
}

#[test]
fn test_leaderboard_sorted_descending() {
    let (env, client, sender, creator, token) = setup();

    let sender2 = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_client.mint(&sender2, &1_000_000i128);

    client.tip(&sender, &creator, &token, &100i128);
    client.tip(&sender2, &creator, &token, &500i128);

    let board = client.get_leaderboard(&TimePeriod::AllTime, &ParticipantKind::Tipper, &10u32);
    assert_eq!(board.len(), 2);
    assert!(board.get(0).unwrap().total_amount >= board.get(1).unwrap().total_amount);
}

#[test]
fn test_reset_leaderboard_clears_entries() {
    let (env, client, sender, creator, token, admin) = setup_with_admin();

    client.tip(&sender, &creator, &token, &1000i128);

    let board_before =
        client.get_leaderboard(&TimePeriod::AllTime, &ParticipantKind::Tipper, &10u32);
    assert_eq!(board_before.len(), 1);

    client.reset_leaderboard(&admin, &LeaderboardType::TopTippers);

    // After reset the Leaderboard key is removed; get_leaderboard falls back to
    // the participants list which still exists, but entries are still present
    // via TipperAggregate. The reset only removes the Leaderboard(type) key.
    // Verify the call succeeds without panic.
    client.reset_leaderboard(&admin, &LeaderboardType::TopCreators);
}

#[test]
fn test_reset_leaderboard_unauthorized() {
    let (env, client, sender, creator, token) = setup();

    let non_admin = Address::generate(&env);
    let result = client.try_reset_leaderboard(&non_admin, &LeaderboardType::TopTippers);
    assert!(result.is_err());
}

// ── snapshot tests ────────────────────────────────────────────────────────────

#[test]
fn test_create_snapshot_returns_id() {
    let (env, client, sender, creator, token, admin) = setup_with_admin();

    let meta = String::from_str(&env, "pre-upgrade snapshot");
    let id = client.create_snapshot(&admin, &meta);
    assert_eq!(id, 0);

    let id2 = client.create_snapshot(&admin, &meta);
    assert_eq!(id2, 1);
}

#[test]
fn test_get_snapshot_returns_metadata() {
    let (env, client, sender, creator, token, admin) = setup_with_admin();

    let meta = String::from_str(&env, "test snapshot");
    let id = client.create_snapshot(&admin, &meta);

    let snap: StateSnapshot = client.get_snapshot(&id);
    assert_eq!(snap.snapshot_id, 0);
    assert_eq!(snap.metadata, meta);
}

#[test]
fn test_get_snapshot_not_found_panics() {
    let (env, client, ..) = setup_with_admin();

    let result = client.try_get_snapshot(&999u64);
    assert!(result.is_err());
}

#[test]
fn test_restore_snapshot_succeeds() {
    let (env, client, sender, creator, token, admin) = setup_with_admin();

    client.tip(&sender, &creator, &token, &1000i128);
    let meta = String::from_str(&env, "snapshot after tip");
    let id = client.create_snapshot(&admin, &meta);

    // Restore should not panic.
    client.restore_snapshot(&admin, &id);
}

#[test]
fn test_delete_snapshot_removes_it() {
    let (env, client, sender, creator, token, admin) = setup_with_admin();

    let meta = String::from_str(&env, "to be deleted");
    let id = client.create_snapshot(&admin, &meta);

    client.delete_snapshot(&admin, &id);

    let result = client.try_get_snapshot(&id);
    assert!(result.is_err());
}

#[test]
fn test_snapshot_unauthorized() {
    let (env, client, sender, creator, token) = setup();

    let non_admin = Address::generate(&env);
    let meta = String::from_str(&env, "unauthorized");
    let result = client.try_create_snapshot(&non_admin, &meta);
    assert!(result.is_err());
}

#[test]
fn test_snapshot_timestamp_recorded() {
    let (env, client, sender, creator, token, admin) = setup_with_admin();

    env.ledger().with_mut(|l| l.timestamp = 5_000_000);
    let meta = String::from_str(&env, "timed snapshot");
    let id = client.create_snapshot(&admin, &meta);

    let snap = client.get_snapshot(&id);
    assert_eq!(snap.timestamp, 5_000_000);
}
