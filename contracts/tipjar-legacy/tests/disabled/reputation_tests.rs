#![cfg(test)]

extern crate std;

use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env};
use tipjar::reputation::{
    self, apply_decay, streak_bonus_bps,
    RepReason, PRECISION, HALF_LIFE_SECS, MAX_STREAK_DAYS,
};

// ── apply_decay ───────────────────────────────────────────────────────────────

#[test]
fn test_decay_zero_elapsed() {
    assert_eq!(apply_decay(1_000_000, 0), 1_000_000);
}

#[test]
fn test_decay_one_half_life() {
    let score = 2_000_000;
    let decayed = apply_decay(score, HALF_LIFE_SECS);
    assert_eq!(decayed, 1_000_000);
}

#[test]
fn test_decay_two_half_lives() {
    let score = 4_000_000;
    let decayed = apply_decay(score, HALF_LIFE_SECS * 2);
    assert_eq!(decayed, 1_000_000);
}

#[test]
fn test_decay_partial_period() {
    let score = 2_000_000;
    // Half a half-life: should be between 1.0 and 2.0
    let decayed = apply_decay(score, HALF_LIFE_SECS / 2);
    assert!(decayed > 1_000_000 && decayed < 2_000_000);
}

#[test]
fn test_decay_zero_score() {
    assert_eq!(apply_decay(0, HALF_LIFE_SECS * 10), 0);
}

// ── streak_bonus_bps ──────────────────────────────────────────────────────────

#[test]
fn test_streak_bonus_zero_days() {
    assert_eq!(streak_bonus_bps(0), 10_000); // 1.0x
}

#[test]
fn test_streak_bonus_ten_days() {
    assert_eq!(streak_bonus_bps(10), 11_000); // 1.1x
}

#[test]
fn test_streak_bonus_capped() {
    assert_eq!(streak_bonus_bps(MAX_STREAK_DAYS), streak_bonus_bps(MAX_STREAK_DAYS + 100));
}

// ── record_tip_sent ───────────────────────────────────────────────────────────

#[test]
fn test_record_tip_sent_increases_score() {
    let env = Env::default();
    let account = Address::generate(&env);

    reputation::record_tip_sent(&env, &account, 1_000_000);

    let rep = reputation::get_score(&env, &account);
    assert!(rep.score > 0);
    assert_eq!(rep.tips_sent, 1);
    assert_eq!(rep.total_tipped, 1_000_000);
}

#[test]
fn test_record_tip_sent_multiple_builds_streak() {
    let env = Env::default();
    let account = Address::generate(&env);

    reputation::record_tip_sent(&env, &account, 1_000_000);
    // Advance time by 26 hours (within streak window)
    env.ledger().with_mut(|l| l.timestamp += 26 * 3_600);
    reputation::record_tip_sent(&env, &account, 1_000_000);

    let rep = reputation::get_score(&env, &account);
    assert_eq!(rep.tips_sent, 2);
    assert!(rep.streak_days >= 2);
}

#[test]
fn test_streak_resets_after_window() {
    let env = Env::default();
    let account = Address::generate(&env);

    reputation::record_tip_sent(&env, &account, 1_000_000);
    // Advance past streak window (50 hours)
    env.ledger().with_mut(|l| l.timestamp += 50 * 3_600);
    reputation::record_tip_sent(&env, &account, 1_000_000);

    let rep = reputation::get_score(&env, &account);
    assert_eq!(rep.streak_days, 1, "streak should reset");
}

// ── record_tip_received ───────────────────────────────────────────────────────

#[test]
fn test_record_tip_received_increases_score() {
    let env = Env::default();
    let account = Address::generate(&env);

    reputation::record_tip_received(&env, &account, 2_000_000);

    let rep = reputation::get_score(&env, &account);
    assert!(rep.score > 0);
    assert_eq!(rep.tips_received, 1);
    assert_eq!(rep.total_received, 2_000_000);
}

#[test]
fn test_tipper_gains_more_than_creator_per_unit() {
    let env = Env::default();
    let tipper = Address::generate(&env);
    let creator = Address::generate(&env);
    let amount = 1_000_000;

    reputation::record_tip_sent(&env, &tipper, amount);
    reputation::record_tip_received(&env, &creator, amount);

    let tipper_score = reputation::get_score(&env, &tipper).score;
    let creator_score = reputation::get_score(&env, &creator).score;
    assert!(tipper_score > creator_score, "tipper should earn more per unit");
}

// ── trigger_decay ─────────────────────────────────────────────────────────────

#[test]
fn test_trigger_decay_reduces_score() {
    let env = Env::default();
    let account = Address::generate(&env);

    reputation::record_tip_sent(&env, &account, 10_000_000);
    let score_before = reputation::get_score(&env, &account).score;

    env.ledger().with_mut(|l| l.timestamp += HALF_LIFE_SECS);
    reputation::trigger_decay(&env, &account);

    let score_after = reputation::get_score(&env, &account).score;
    assert!(score_after < score_before, "score should decay");
}

#[test]
fn test_trigger_decay_no_elapsed_is_noop() {
    let env = Env::default();
    let account = Address::generate(&env);

    reputation::record_tip_sent(&env, &account, 1_000_000);
    let score_before = reputation::get_score(&env, &account).score;

    reputation::trigger_decay(&env, &account); // no time elapsed

    let score_after = reputation::get_score(&env, &account).score;
    assert_eq!(score_before, score_after);
}

// ── history ───────────────────────────────────────────────────────────────────

#[test]
fn test_history_records_tip_sent() {
    let env = Env::default();
    let account = Address::generate(&env);

    reputation::record_tip_sent(&env, &account, 1_000_000);

    let hist = reputation::get_reputation_history(&env, &account);
    assert!(hist.len() >= 1);
    assert!(hist.iter().any(|e| e.reason == RepReason::TipSent));
}

#[test]
fn test_history_records_decay() {
    let env = Env::default();
    let account = Address::generate(&env);

    reputation::record_tip_sent(&env, &account, 10_000_000);
    env.ledger().with_mut(|l| l.timestamp += HALF_LIFE_SECS);
    reputation::trigger_decay(&env, &account);

    let hist = reputation::get_reputation_history(&env, &account);
    assert!(hist.iter().any(|e| e.reason == RepReason::Decay));
}

#[test]
fn test_history_bounded_at_max_size() {
    let env = Env::default();
    let account = Address::generate(&env);

    // Push more entries than REP_HISTORY_SIZE
    for i in 0..25u64 {
        env.ledger().with_mut(|l| l.timestamp += 1);
        reputation::record_tip_sent(&env, &account, 1_000_000);
    }

    let hist = reputation::get_reputation_history(&env, &account);
    assert!(hist.len() <= tipjar::reputation::REP_HISTORY_SIZE);
}

#[test]
fn test_fresh_account_has_zero_score() {
    let env = Env::default();
    let account = Address::generate(&env);
    let rep = reputation::get_score(&env, &account);
    assert_eq!(rep.score, 0);
    assert_eq!(rep.tips_sent, 0);
    assert_eq!(rep.tips_received, 0);
}
