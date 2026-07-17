mod common;
use common::*;
use tipjar::{LeaderboardEntry, ParticipantKind, TimePeriod};

// ── tipper stats ──────────────────────────────────────────────────────────────

#[test]
fn test_single_tip_appears_in_tipper_leaderboard() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);

    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &300);

    let board = ctx.tipjar_client.get_leaderboard(
        &TimePeriod::AllTime,
        &ParticipantKind::Tipper,
        &10,
    );
    assert_eq!(board.len(), 1);
    let entry: LeaderboardEntry = board.get(0).unwrap();
    assert_eq!(entry.address, sender);
    assert_eq!(entry.total_amount, 300);
    assert_eq!(entry.tip_count, 1);
}

#[test]
fn test_multiple_tips_accumulate_for_same_tipper() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);

    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &100);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &200);

    let board = ctx.tipjar_client.get_leaderboard(
        &TimePeriod::AllTime,
        &ParticipantKind::Tipper,
        &10,
    );
    assert_eq!(board.len(), 1);
    let entry: LeaderboardEntry = board.get(0).unwrap();
    assert_eq!(entry.total_amount, 300);
    assert_eq!(entry.tip_count, 2);
}

// ── creator stats ─────────────────────────────────────────────────────────────

#[test]
fn test_single_tip_appears_in_creator_leaderboard() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);

    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &500);

    let board = ctx.tipjar_client.get_leaderboard(
        &TimePeriod::AllTime,
        &ParticipantKind::Creator,
        &10,
    );
    assert_eq!(board.len(), 1);
    let entry: LeaderboardEntry = board.get(0).unwrap();
    assert_eq!(entry.address, creator);
    assert_eq!(entry.total_amount, 500);
    assert_eq!(entry.tip_count, 1);
}

#[test]
fn test_multiple_tippers_to_same_creator_accumulate() {
    let ctx = TestContext::new();
    let s1 = ctx.create_user();
    let s2 = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&s1, &ctx.token_1, 1_000);
    ctx.mint_tokens(&s2, &ctx.token_1, 1_000);

    ctx.tipjar_client.tip(&s1, &creator, &ctx.token_1, &300);
    ctx.tipjar_client.tip(&s2, &creator, &ctx.token_1, &400);

    let board = ctx.tipjar_client.get_leaderboard(
        &TimePeriod::AllTime,
        &ParticipantKind::Creator,
        &10,
    );
    assert_eq!(board.len(), 1);
    let entry: LeaderboardEntry = board.get(0).unwrap();
    assert_eq!(entry.total_amount, 700);
    assert_eq!(entry.tip_count, 2);
}

// ── sorting ───────────────────────────────────────────────────────────────────

#[test]
fn test_leaderboard_sorted_descending_by_total_amount() {
    let ctx = TestContext::new();
    let s1 = ctx.create_user();
    let s2 = ctx.create_user();
    let s3 = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&s1, &ctx.token_1, 1_000);
    ctx.mint_tokens(&s2, &ctx.token_1, 1_000);
    ctx.mint_tokens(&s3, &ctx.token_1, 1_000);

    ctx.tipjar_client.tip(&s1, &creator, &ctx.token_1, &100);
    ctx.tipjar_client.tip(&s2, &creator, &ctx.token_1, &500);
    ctx.tipjar_client.tip(&s3, &creator, &ctx.token_1, &300);

    let board = ctx.tipjar_client.get_leaderboard(
        &TimePeriod::AllTime,
        &ParticipantKind::Tipper,
        &10,
    );
    assert_eq!(board.len(), 3);
    assert_eq!(board.get(0).unwrap().total_amount, 500);
    assert_eq!(board.get(1).unwrap().total_amount, 300);
    assert_eq!(board.get(2).unwrap().total_amount, 100);
}

// ── pagination (limit) ────────────────────────────────────────────────────────

#[test]
fn test_limit_caps_leaderboard_results() {
    let ctx = TestContext::new();
    let creator = ctx.create_creator();
    for i in 1u32..=5 {
        let s = ctx.create_user();
        ctx.mint_tokens(&s, &ctx.token_1, 1_000);
        ctx.tipjar_client.tip(&s, &creator, &ctx.token_1, &(i as i128 * 100));
    }

    let board = ctx.tipjar_client.get_leaderboard(
        &TimePeriod::AllTime,
        &ParticipantKind::Tipper,
        &3,
    );
    assert_eq!(board.len(), 3);
    assert_eq!(board.get(0).unwrap().total_amount, 500);
    assert_eq!(board.get(1).unwrap().total_amount, 400);
    assert_eq!(board.get(2).unwrap().total_amount, 300);
}

#[test]
fn test_empty_leaderboard_returns_empty_vec() {
    let ctx = TestContext::new();
    let board = ctx.tipjar_client.get_leaderboard(
        &TimePeriod::AllTime,
        &ParticipantKind::Tipper,
        &10,
    );
    assert_eq!(board.len(), 0);
}

// ── tipper and creator boards are independent ─────────────────────────────────

#[test]
fn test_tipper_and_creator_boards_are_independent() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, &ctx.token_1, 1_000);
    ctx.tipjar_client.tip(&sender, &creator, &ctx.token_1, &200);

    let tipper_board = ctx.tipjar_client.get_leaderboard(
        &TimePeriod::AllTime,
        &ParticipantKind::Tipper,
        &10,
    );
    let creator_board = ctx.tipjar_client.get_leaderboard(
        &TimePeriod::AllTime,
        &ParticipantKind::Creator,
        &10,
    );

    assert_eq!(tipper_board.len(), 1);
    assert_eq!(tipper_board.get(0).unwrap().address, sender);

    assert_eq!(creator_board.len(), 1);
    assert_eq!(creator_board.get(0).unwrap().address, creator);
}
