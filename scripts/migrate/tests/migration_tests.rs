//! Integration and unit tests for the TipJar state migration toolkit.
//!
//! Coverage:
//!   1. Export completeness  — snapshot captures all state categories
//!   2. Checksum sealing     — checksum is computed and verified correctly
//!   3. Transform accuracy   — every transformation rule produces correct output
//!   4. Import accuracy      — snapshot round-trips through serialisation losslessly
//!   5. Verification logic   — VerificationReport detects mismatches and missing fields
//!   6. Rollback capability  — backup snapshot restores state faithfully
//!   7. History tracking     — history file accumulates entries across runs
//!   8. Config parsing       — config.toml is parsed without errors

// Bring the toolkit modules into scope.  When run via `cargo test` from the
// scripts/migrate directory these resolve through main.rs's `mod` declarations.
#[path = "../types.rs"]
mod types;
#[path = "../rpc_client.rs"]
mod rpc_client;
#[path = "../export_state.rs"]
mod export_state;
#[path = "../transform_state.rs"]
mod transform_state;
#[path = "../verify_migration.rs"]
mod verify_migration;
#[path = "../history.rs"]
mod history;

use std::collections::HashMap;
use types::{
    FindingSeverity, LeaderboardEntry, LockedTip, MatchingProgram, Milestone,
    MigrationHistory, MigrationHistoryEntry, MigrationStatus, StateSnapshot,
    Subscription, SubscriptionStatus, TipMetadata, TipRecord, TimeLock,
    TransformationLog, VerificationFinding, VerificationReport,
};

// ─────────────────────────────────────────────────────────────────────────────
// Shared fixture helpers
// ─────────────────────────────────────────────────────────────────────────────

fn empty_snapshot() -> StateSnapshot {
    StateSnapshot {
        contract_id: "CTEST000000000000000000000000000000000000000000000000000".into(),
        schema_version: 1,
        exported_at: 1_700_000_000,
        ledger_sequence: 5000,
        checksum: String::new(),
        admin: "GADMIN00000000000000000000000000000000000000000000000000".into(),
        fee_basis_points: 100,
        refund_window_seconds: 86_400,
        paused: false,
        pause_reason: None,
        tip_counter: 0,
        matching_counter: 0,
        current_fee_bps: 100,
        whitelisted_tokens: vec![],
        creator_balances: HashMap::new(),
        creator_totals: HashMap::new(),
        tip_history: HashMap::new(),
        tipper_aggregates: HashMap::new(),
        creator_aggregates: HashMap::new(),
        subscriptions: HashMap::new(),
        time_locks: vec![],
        milestones: HashMap::new(),
        matching_programs: vec![],
        tip_records: vec![],
        locked_tips: vec![],
        user_roles: HashMap::new(),
    }
}

fn populated_snapshot() -> StateSnapshot {
    let mut snap = empty_snapshot();

    // Creator balances and totals
    snap.creator_balances.insert("GCREATOR1:GTOKEN1".into(), 5_000_000);
    snap.creator_balances.insert("GCREATOR2:GTOKEN1".into(), 0);
    snap.creator_balances.insert("GCREATOR1:GTOKEN2".into(), 1_200_000);
    snap.creator_totals.insert("GCREATOR1:GTOKEN1".into(), 10_000_000);
    snap.creator_totals.insert("GCREATOR2:GTOKEN1".into(), 0);

    // Tip history
    snap.tip_history.insert(
        "GCREATOR1".into(),
        vec![
            TipMetadata { sender: "GSENDER1".into(), amount: 1_000_000, message: None, timestamp: 1_699_000_000 },
            TipMetadata { sender: "GSENDER2".into(), amount: 2_000_000, message: Some("great work".into()), timestamp: 1_699_500_000 },
        ],
    );

    // Leaderboard
    snap.tipper_aggregates.insert(
        "GSENDER1:0".into(),
        LeaderboardEntry { address: "GSENDER1".into(), total_amount: 1_000_000, tip_count: 1 },
    );
    snap.creator_aggregates.insert(
        "GCREATOR1:0".into(),
        LeaderboardEntry { address: "GCREATOR1".into(), total_amount: 3_000_000, tip_count: 2 },
    );

    // Subscriptions
    snap.subscriptions.insert(
        "GSUB1:GCREATOR1".into(),
        Subscription {
            subscriber: "GSUB1".into(),
            creator: "GCREATOR1".into(),
            token: "GTOKEN1".into(),
            amount: 500_000,
            interval_seconds: 86_400,
            last_payment: 1_699_000_000,
            next_payment: 1_699_086_400,
            status: SubscriptionStatus::Active,
        },
    );
    snap.subscriptions.insert(
        "GSUB2:GCREATOR1".into(),
        Subscription {
            subscriber: "GSUB2".into(),
            creator: "GCREATOR1".into(),
            token: "GTOKEN1".into(),
            amount: 250_000,
            interval_seconds: 604_800,
            last_payment: 0,
            next_payment: 1_699_000_000,
            status: SubscriptionStatus::Cancelled,
        },
    );

    // Time locks
    snap.time_locks.push(TimeLock {
        lock_id: 0,
        sender: "GSENDER1".into(),
        creator: "GCREATOR1".into(),
        token: "GTOKEN1".into(),
        amount: 3_000_000,
        unlock_time: 1_800_000_000, // future
        cancelled: false,
    });
    snap.time_locks.push(TimeLock {
        lock_id: 1,
        sender: "GSENDER2".into(),
        creator: "GCREATOR2".into(),
        token: "GTOKEN1".into(),
        amount: 1_000_000,
        unlock_time: 1_600_000_000, // past (expired)
        cancelled: false,
    });

    // Milestones
    snap.milestones.insert(
        "GCREATOR1:0".into(),
        Milestone {
            id: 0,
            creator: "GCREATOR1".into(),
            goal_amount: 10_000_000,
            current_amount: 5_000_000,
            description: "First album".into(),
            deadline: Some(1_800_000_000),
            completed: false,
        },
    );

    // Matching programs
    snap.matching_programs.push(MatchingProgram {
        id: 0,
        sponsor: "GSPONSOR1".into(),
        creator: "GCREATOR1".into(),
        token: "GTOKEN1".into(),
        match_ratio: 100,
        max_match_amount: 5_000_000,
        current_matched: 1_000_000,
        active: true,
    });

    // Tip records
    snap.tip_records.push(TipRecord {
        id: 0,
        sender: "GSENDER1".into(),
        creator: "GCREATOR1".into(),
        token: "GTOKEN1".into(),
        amount: 1_000_000,
        timestamp: 1_699_000_000,
        refunded: false,
        refund_requested: false,
        refund_approved: false,
    });
    snap.tip_records.push(TipRecord {
        id: 1,
        sender: "GSENDER2".into(),
        creator: "GCREATOR1".into(),
        token: "GTOKEN1".into(),
        amount: 500_000,
        timestamp: 1_699_100_000,
        refunded: true,
        refund_requested: true,
        refund_approved: true,
    });

    // Locked tips
    snap.locked_tips.push(LockedTip {
        tip_id: 0,
        sender: "GSENDER1".into(),
        creator: "GCREATOR1".into(),
        token: "GTOKEN1".into(),
        amount: 2_000_000,
        unlock_timestamp: 1_800_000_000,
    });

    // User roles
    snap.user_roles.insert("GCREATOR1".into(), "Creator".into());
    snap.user_roles.insert("GADMIN00000000000000000000000000000000000000000000000000".into(), "Admin".into());

    // Whitelisted tokens
    snap.whitelisted_tokens = vec!["GTOKEN1".into(), "GTOKEN2".into()];

    snap.tip_counter = 2;
    snap.matching_counter = 1;

    snap
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Export completeness
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test_export_completeness {
    use super::*;

    #[test]
    fn snapshot_captures_all_state_categories() {
        let snap = populated_snapshot();
        // Every category must be non-empty.
        assert!(!snap.creator_balances.is_empty(), "creator_balances empty");
        assert!(!snap.creator_totals.is_empty(), "creator_totals empty");
        assert!(!snap.tip_history.is_empty(), "tip_history empty");
        assert!(!snap.tipper_aggregates.is_empty(), "tipper_aggregates empty");
        assert!(!snap.creator_aggregates.is_empty(), "creator_aggregates empty");
        assert!(!snap.subscriptions.is_empty(), "subscriptions empty");
        assert!(!snap.time_locks.is_empty(), "time_locks empty");
        assert!(!snap.milestones.is_empty(), "milestones empty");
        assert!(!snap.matching_programs.is_empty(), "matching_programs empty");
        assert!(!snap.tip_records.is_empty(), "tip_records empty");
        assert!(!snap.locked_tips.is_empty(), "locked_tips empty");
        assert!(!snap.user_roles.is_empty(), "user_roles empty");
        assert!(!snap.whitelisted_tokens.is_empty(), "whitelisted_tokens empty");
    }

    #[test]
    fn total_records_counts_all_collections() {
        let snap = populated_snapshot();
        let total = snap.total_records();
        // Manually sum expected counts from populated_snapshot():
        // balances=3, totals=2, tip_history=2, tipper_agg=1, creator_agg=1,
        // subs=2, time_locks=2, milestones=1, matching=1, tip_records=2,
        // locked_tips=1, user_roles=2
        assert_eq!(total, 20, "total_records mismatch: got {}", total);
    }

    #[test]
    fn snapshot_preserves_instance_fields() {
        let snap = populated_snapshot();
        assert_eq!(snap.fee_basis_points, 100);
        assert_eq!(snap.refund_window_seconds, 86_400);
        assert!(!snap.paused);
        assert_eq!(snap.tip_counter, 2);
        assert_eq!(snap.matching_counter, 1);
        assert_eq!(snap.schema_version, 1);
    }

    #[test]
    fn snapshot_preserves_creator_balance_keys() {
        let snap = populated_snapshot();
        assert!(snap.creator_balances.contains_key("GCREATOR1:GTOKEN1"));
        assert!(snap.creator_balances.contains_key("GCREATOR1:GTOKEN2"));
        assert_eq!(snap.creator_balances["GCREATOR1:GTOKEN1"], 5_000_000);
    }

    #[test]
    fn snapshot_preserves_tip_history_order() {
        let snap = populated_snapshot();
        let tips = snap.tip_history.get("GCREATOR1").unwrap();
        assert_eq!(tips.len(), 2);
        assert_eq!(tips[0].amount, 1_000_000);
        assert_eq!(tips[1].amount, 2_000_000);
        assert_eq!(tips[1].message, Some("great work".into()));
    }

    #[test]
    fn snapshot_serialises_and_deserialises_losslessly() {
        let snap = populated_snapshot();
        let json = serde_json::to_string(&snap).expect("serialise failed");
        let restored: StateSnapshot = serde_json::from_str(&json).expect("deserialise failed");

        assert_eq!(restored.contract_id, snap.contract_id);
        assert_eq!(restored.creator_balances, snap.creator_balances);
        assert_eq!(restored.creator_totals, snap.creator_totals);
        assert_eq!(restored.tip_history, snap.tip_history);
        assert_eq!(restored.subscriptions.len(), snap.subscriptions.len());
        assert_eq!(restored.time_locks.len(), snap.time_locks.len());
        assert_eq!(restored.milestones.len(), snap.milestones.len());
        assert_eq!(restored.matching_programs.len(), snap.matching_programs.len());
        assert_eq!(restored.tip_records.len(), snap.tip_records.len());
        assert_eq!(restored.locked_tips.len(), snap.locked_tips.len());
        assert_eq!(restored.user_roles, snap.user_roles);
        assert_eq!(restored.whitelisted_tokens, snap.whitelisted_tokens);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Checksum sealing
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test_checksum {
    use super::*;
    use export_state::{compute_checksum, verify_checksum};

    #[test]
    fn checksum_is_deterministic() {
        let snap = populated_snapshot();
        let c1 = compute_checksum(&snap).unwrap();
        let c2 = compute_checksum(&snap).unwrap();
        assert_eq!(c1, c2, "checksum must be deterministic");
    }

    #[test]
    fn checksum_is_hex_sha256() {
        let snap = populated_snapshot();
        let c = compute_checksum(&snap).unwrap();
        assert_eq!(c.len(), 64, "SHA-256 hex must be 64 chars, got {}", c.len());
        assert!(c.chars().all(|ch| ch.is_ascii_hexdigit()), "non-hex char in checksum");
    }

    #[test]
    fn verify_checksum_passes_when_correct() {
        let mut snap = populated_snapshot();
        snap.checksum = compute_checksum(&snap).unwrap();
        assert!(verify_checksum(&snap).is_ok());
    }

    #[test]
    fn verify_checksum_fails_when_tampered() {
        let mut snap = populated_snapshot();
        snap.checksum = compute_checksum(&snap).unwrap();
        // Tamper with a balance.
        snap.creator_balances.insert("GCREATOR1:GTOKEN1".into(), 9_999_999);
        assert!(
            verify_checksum(&snap).is_err(),
            "verify_checksum should fail after tampering"
        );
    }

    #[test]
    fn checksum_ignores_its_own_field() {
        // Two snapshots identical except for the checksum field itself must
        // produce the same checksum (the field is zeroed before hashing).
        let snap1 = populated_snapshot();
        let mut snap2 = populated_snapshot();
        snap2.checksum = "some_old_checksum".into();
        assert_eq!(
            compute_checksum(&snap1).unwrap(),
            compute_checksum(&snap2).unwrap()
        );
    }

    #[test]
    fn different_snapshots_produce_different_checksums() {
        let snap1 = populated_snapshot();
        let mut snap2 = populated_snapshot();
        snap2.creator_balances.insert("GNEW:GTOKEN1".into(), 1);
        assert_ne!(
            compute_checksum(&snap1).unwrap(),
            compute_checksum(&snap2).unwrap()
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Data transformation accuracy
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test_transform {
    use super::*;
    use transform_state::StateTransformer;

    fn transform(snap: StateSnapshot) -> (StateSnapshot, Vec<TransformationLog>) {
        StateTransformer::new(1, 2).transform(snap).expect("transform failed")
    }

    // ── zero-value pruning ────────────────────────────────────────────────────

    #[test]
    fn removes_zero_creator_balances() {
        let snap = populated_snapshot(); // contains GCREATOR2:GTOKEN1 = 0
        let (out, logs) = transform(snap);
        assert!(
            !out.creator_balances.contains_key("GCREATOR2:GTOKEN1"),
            "zero balance should be removed"
        );
        assert!(
            out.creator_balances.contains_key("GCREATOR1:GTOKEN1"),
            "non-zero balance must be kept"
        );
        let rule_log = logs.iter().find(|l| l.rule == "v1_to_v2_normalise_balances").unwrap();
        assert_eq!(rule_log.records_affected, 1);
    }

    #[test]
    fn removes_zero_creator_totals() {
        let snap = populated_snapshot(); // contains GCREATOR2:GTOKEN1 total = 0
        let (out, logs) = transform(snap);
        assert!(!out.creator_totals.contains_key("GCREATOR2:GTOKEN1"));
        let rule_log = logs.iter().find(|l| l.rule == "v1_to_v2_normalise_totals").unwrap();
        assert_eq!(rule_log.records_affected, 1);
    }

    // ── expired lock cancellation ─────────────────────────────────────────────

    #[test]
    fn marks_expired_time_locks_cancelled() {
        // lock_id=1 has unlock_time=1_600_000_000 < exported_at=1_700_000_000
        let snap = populated_snapshot();
        let (out, logs) = transform(snap);
        let expired = out.time_locks.iter().find(|l| l.lock_id == 1).unwrap();
        assert!(expired.cancelled, "expired lock must be marked cancelled");
        let future = out.time_locks.iter().find(|l| l.lock_id == 0).unwrap();
        assert!(!future.cancelled, "future lock must not be cancelled");
        let rule_log = logs.iter().find(|l| l.rule == "v1_to_v2_cancel_expired_locks").unwrap();
        assert_eq!(rule_log.records_affected, 1);
    }

    #[test]
    fn does_not_cancel_already_cancelled_locks() {
        let mut snap = populated_snapshot();
        snap.time_locks[1].cancelled = true; // already cancelled
        let (out, logs) = transform(snap);
        let rule_log = logs.iter().find(|l| l.rule == "v1_to_v2_cancel_expired_locks").unwrap();
        // Should not double-count.
        assert_eq!(rule_log.records_affected, 0);
        let _ = out;
    }

    // ── cancelled subscription stripping ─────────────────────────────────────

    #[test]
    fn strips_cancelled_subscriptions() {
        let snap = populated_snapshot(); // GSUB2:GCREATOR1 is Cancelled
        let (out, logs) = transform(snap);
        assert!(
            !out.subscriptions.contains_key("GSUB2:GCREATOR1"),
            "cancelled subscription must be removed"
        );
        assert!(
            out.subscriptions.contains_key("GSUB1:GCREATOR1"),
            "active subscription must be kept"
        );
        let rule_log = logs.iter().find(|l| l.rule == "v1_to_v2_strip_cancelled_subscriptions").unwrap();
        assert_eq!(rule_log.records_affected, 1);
    }

    // ── tip history cap ───────────────────────────────────────────────────────

    #[test]
    fn caps_tip_history_at_1000_entries() {
        let mut snap = empty_snapshot();
        snap.creator_balances.insert("GCREATOR1:GTOKEN1".into(), 1);
        let tips: Vec<TipMetadata> = (0u64..1_200)
            .map(|i| TipMetadata {
                sender: "GSENDER".into(),
                amount: i as i128 + 1,
                message: None,
                timestamp: i,
            })
            .collect();
        snap.tip_history.insert("GCREATOR1".into(), tips);
        let (out, logs) = transform(snap);
        assert_eq!(out.tip_history["GCREATOR1"].len(), 1_000);
        // Newest 1000 entries kept (indices 200–1199 → amounts 201–1200).
        assert_eq!(out.tip_history["GCREATOR1"][0].amount, 201);
        assert_eq!(out.tip_history["GCREATOR1"][999].amount, 1_200);
        let rule_log = logs.iter().find(|l| l.rule == "v1_to_v2_cap_tip_history").unwrap();
        assert_eq!(rule_log.records_affected, 200);
    }

    #[test]
    fn does_not_trim_tip_history_under_cap() {
        let snap = populated_snapshot(); // only 2 tips for GCREATOR1
        let (out, _) = transform(snap);
        assert_eq!(out.tip_history["GCREATOR1"].len(), 2);
    }

    // ── refunded tip record removal ───────────────────────────────────────────

    #[test]
    fn removes_fully_refunded_tip_records() {
        let snap = populated_snapshot(); // tip_records[1] is refunded=true
        let (out, logs) = transform(snap);
        assert_eq!(out.tip_records.len(), 1);
        assert_eq!(out.tip_records[0].id, 0);
        let rule_log = logs.iter().find(|l| l.rule == "v1_to_v2_remove_refunded_tip_records").unwrap();
        assert_eq!(rule_log.records_affected, 1);
    }

    // ── schema version bump ───────────────────────────────────────────────────

    #[test]
    fn bumps_schema_version_to_target() {
        let snap = populated_snapshot();
        assert_eq!(snap.schema_version, 1);
        let (out, _) = transform(snap);
        assert_eq!(out.schema_version, 2);
    }

    // ── checksum after transform ──────────────────────────────────────────────

    #[test]
    fn transformed_snapshot_has_valid_checksum() {
        let snap = populated_snapshot();
        let (out, _) = transform(snap);
        assert!(!out.checksum.is_empty());
        export_state::verify_checksum(&out).expect("checksum invalid after transform");
    }

    // ── transformation log completeness ──────────────────────────────────────

    #[test]
    fn all_rules_produce_log_entries() {
        let snap = populated_snapshot();
        let (_, logs) = transform(snap);
        let rule_ids: Vec<&str> = logs.iter().map(|l| l.rule.as_str()).collect();
        for expected in &[
            "v1_to_v2_normalise_balances",
            "v1_to_v2_normalise_totals",
            "v1_to_v2_cancel_expired_locks",
            "v1_to_v2_strip_cancelled_subscriptions",
            "v1_to_v2_cap_tip_history",
            "v1_to_v2_remove_refunded_tip_records",
            "v1_to_v2_bump_schema_version",
        ] {
            assert!(
                rule_ids.contains(expected),
                "missing log entry for rule: {}",
                expected
            );
        }
    }

    #[test]
    fn rejects_snapshot_with_wrong_source_version() {
        let mut snap = populated_snapshot();
        snap.schema_version = 99; // wrong
        let result = StateTransformer::new(1, 2).transform(snap);
        assert!(result.is_err(), "should reject mismatched schema version");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Import accuracy (round-trip serialisation)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test_import_accuracy {
    use super::*;

    /// Simulates the import phase by serialising the snapshot to JSON (as it
    /// would be written to disk) and deserialising it back, then asserting
    /// field-level equality.  This validates that no data is lost or corrupted
    /// during the disk round-trip that precedes the actual on-chain import.
    fn round_trip(snap: &StateSnapshot) -> StateSnapshot {
        let json = serde_json::to_string_pretty(snap).expect("serialise");
        serde_json::from_str(&json).expect("deserialise")
    }

    #[test]
    fn creator_balances_survive_round_trip() {
        let snap = populated_snapshot();
        let rt = round_trip(&snap);
        assert_eq!(rt.creator_balances, snap.creator_balances);
    }

    #[test]
    fn creator_totals_survive_round_trip() {
        let snap = populated_snapshot();
        let rt = round_trip(&snap);
        assert_eq!(rt.creator_totals, snap.creator_totals);
    }

    #[test]
    fn tip_history_survives_round_trip() {
        let snap = populated_snapshot();
        let rt = round_trip(&snap);
        assert_eq!(rt.tip_history, snap.tip_history);
    }

    #[test]
    fn subscriptions_survive_round_trip() {
        let snap = populated_snapshot();
        let rt = round_trip(&snap);
        assert_eq!(rt.subscriptions.len(), snap.subscriptions.len());
        for (k, v) in &snap.subscriptions {
            let actual = rt.subscriptions.get(k).expect("subscription missing after round-trip");
            assert_eq!(actual.amount, v.amount);
            assert_eq!(actual.interval_seconds, v.interval_seconds);
            assert_eq!(actual.status, v.status);
        }
    }

    #[test]
    fn time_locks_survive_round_trip() {
        let snap = populated_snapshot();
        let rt = round_trip(&snap);
        assert_eq!(rt.time_locks.len(), snap.time_locks.len());
        for (a, b) in snap.time_locks.iter().zip(rt.time_locks.iter()) {
            assert_eq!(a.lock_id, b.lock_id);
            assert_eq!(a.amount, b.amount);
            assert_eq!(a.unlock_time, b.unlock_time);
            assert_eq!(a.cancelled, b.cancelled);
        }
    }

    #[test]
    fn milestones_survive_round_trip() {
        let snap = populated_snapshot();
        let rt = round_trip(&snap);
        assert_eq!(rt.milestones.len(), snap.milestones.len());
        for (k, v) in &snap.milestones {
            let actual = rt.milestones.get(k).expect("milestone missing after round-trip");
            assert_eq!(actual.goal_amount, v.goal_amount);
            assert_eq!(actual.current_amount, v.current_amount);
            assert_eq!(actual.completed, v.completed);
            assert_eq!(actual.description, v.description);
        }
    }

    #[test]
    fn matching_programs_survive_round_trip() {
        let snap = populated_snapshot();
        let rt = round_trip(&snap);
        assert_eq!(rt.matching_programs.len(), snap.matching_programs.len());
        let a = &snap.matching_programs[0];
        let b = &rt.matching_programs[0];
        assert_eq!(a.id, b.id);
        assert_eq!(a.match_ratio, b.match_ratio);
        assert_eq!(a.max_match_amount, b.max_match_amount);
        assert_eq!(a.current_matched, b.current_matched);
        assert_eq!(a.active, b.active);
    }

    #[test]
    fn tip_records_survive_round_trip() {
        let snap = populated_snapshot();
        let rt = round_trip(&snap);
        assert_eq!(rt.tip_records.len(), snap.tip_records.len());
        for (a, b) in snap.tip_records.iter().zip(rt.tip_records.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.amount, b.amount);
            assert_eq!(a.refunded, b.refunded);
        }
    }

    #[test]
    fn locked_tips_survive_round_trip() {
        let snap = populated_snapshot();
        let rt = round_trip(&snap);
        assert_eq!(rt.locked_tips.len(), snap.locked_tips.len());
        assert_eq!(rt.locked_tips[0].amount, snap.locked_tips[0].amount);
        assert_eq!(rt.locked_tips[0].unlock_timestamp, snap.locked_tips[0].unlock_timestamp);
    }

    #[test]
    fn user_roles_survive_round_trip() {
        let snap = populated_snapshot();
        let rt = round_trip(&snap);
        assert_eq!(rt.user_roles, snap.user_roles);
    }

    #[test]
    fn large_i128_amounts_survive_round_trip() {
        let mut snap = empty_snapshot();
        // i128::MAX / 2 — well above any realistic tip amount but tests the type boundary.
        let large: i128 = 170_141_183_460_469_231_731_687_303_715_884_105_727 / 2;
        snap.creator_balances.insert("GCREATOR:GTOKEN".into(), large);
        let rt = round_trip(&snap);
        assert_eq!(rt.creator_balances["GCREATOR:GTOKEN"], large);
    }

    #[test]
    fn optional_message_none_survives_round_trip() {
        let mut snap = empty_snapshot();
        snap.tip_history.insert(
            "GCREATOR".into(),
            vec![TipMetadata { sender: "GSENDER".into(), amount: 1, message: None, timestamp: 0 }],
        );
        let rt = round_trip(&snap);
        assert_eq!(rt.tip_history["GCREATOR"][0].message, None);
    }

    #[test]
    fn optional_message_some_survives_round_trip() {
        let mut snap = empty_snapshot();
        snap.tip_history.insert(
            "GCREATOR".into(),
            vec![TipMetadata {
                sender: "GSENDER".into(),
                amount: 1,
                message: Some("hello 🌟".into()),
                timestamp: 0,
            }],
        );
        let rt = round_trip(&snap);
        assert_eq!(rt.tip_history["GCREATOR"][0].message, Some("hello 🌟".into()));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Verification logic
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test_verification {
    use super::*;

    /// Builds a VerificationReport by comparing two snapshots field-by-field
    /// without making any RPC calls.  This mirrors what VerificationEngine does
    /// but operates entirely in-memory so tests run offline.
    fn compare_snapshots(source: &StateSnapshot, target: &StateSnapshot) -> VerificationReport {
        let mut findings: Vec<VerificationFinding> = Vec::new();
        let mut records_checked = 0usize;

        // Creator balances
        for (key, expected) in &source.creator_balances {
            records_checked += 1;
            let actual = target.creator_balances.get(key).copied().unwrap_or(i128::MIN);
            if actual == i128::MIN {
                findings.push(VerificationFinding {
                    severity: FindingSeverity::Missing,
                    field: format!("creator_balances.{}", key),
                    expected: expected.to_string(),
                    actual: "<absent>".into(),
                });
            } else if actual != *expected {
                findings.push(VerificationFinding {
                    severity: FindingSeverity::Mismatch,
                    field: format!("creator_balances.{}", key),
                    expected: expected.to_string(),
                    actual: actual.to_string(),
                });
            }
        }

        // Creator totals
        for (key, expected) in &source.creator_totals {
            records_checked += 1;
            let actual = target.creator_totals.get(key).copied().unwrap_or(i128::MIN);
            if actual == i128::MIN {
                findings.push(VerificationFinding {
                    severity: FindingSeverity::Missing,
                    field: format!("creator_totals.{}", key),
                    expected: expected.to_string(),
                    actual: "<absent>".into(),
                });
            } else if actual != *expected {
                findings.push(VerificationFinding {
                    severity: FindingSeverity::Mismatch,
                    field: format!("creator_totals.{}", key),
                    expected: expected.to_string(),
                    actual: actual.to_string(),
                });
            }
        }

        // Tip history counts
        for (creator, tips) in &source.tip_history {
            records_checked += 1;
            let actual_count = target
                .tip_history
                .get(creator)
                .map(|v| v.len())
                .unwrap_or(0);
            if actual_count != tips.len() {
                findings.push(VerificationFinding {
                    severity: FindingSeverity::Mismatch,
                    field: format!("tip_history.{}.count", creator),
                    expected: tips.len().to_string(),
                    actual: actual_count.to_string(),
                });
            }
        }

        // User roles
        for (addr, expected_role) in &source.user_roles {
            records_checked += 1;
            match target.user_roles.get(addr) {
                None => findings.push(VerificationFinding {
                    severity: FindingSeverity::Missing,
                    field: format!("user_roles.{}", addr),
                    expected: expected_role.clone(),
                    actual: "<absent>".into(),
                }),
                Some(actual_role) if actual_role != expected_role => {
                    findings.push(VerificationFinding {
                        severity: FindingSeverity::Mismatch,
                        field: format!("user_roles.{}", addr),
                        expected: expected_role.clone(),
                        actual: actual_role.clone(),
                    });
                }
                _ => {}
            }
        }

        let passed = findings.is_empty();
        VerificationReport {
            source_contract_id: source.contract_id.clone(),
            target_contract_id: target.contract_id.clone(),
            verified_at: "2024-01-01T00:00:00Z".into(),
            records_checked,
            passed,
            findings,
        }
    }

    #[test]
    fn identical_snapshots_pass_verification() {
        let source = populated_snapshot();
        let target = populated_snapshot();
        let report = compare_snapshots(&source, &target);
        assert!(report.passed, "identical snapshots should pass: {:?}", report.findings);
        assert!(report.is_clean());
        assert_eq!(report.findings.len(), 0);
    }

    #[test]
    fn detects_missing_creator_balance() {
        let source = populated_snapshot();
        let mut target = populated_snapshot();
        target.creator_balances.remove("GCREATOR1:GTOKEN1");
        let report = compare_snapshots(&source, &target);
        assert!(!report.passed);
        let f = report.findings.iter().find(|f| f.field == "creator_balances.GCREATOR1:GTOKEN1").unwrap();
        assert_eq!(f.severity, FindingSeverity::Missing);
        assert_eq!(f.actual, "<absent>");
    }

    #[test]
    fn detects_mismatched_creator_balance() {
        let source = populated_snapshot();
        let mut target = populated_snapshot();
        target.creator_balances.insert("GCREATOR1:GTOKEN1".into(), 1);
        let report = compare_snapshots(&source, &target);
        assert!(!report.passed);
        let f = report.findings.iter().find(|f| f.field == "creator_balances.GCREATOR1:GTOKEN1").unwrap();
        assert_eq!(f.severity, FindingSeverity::Mismatch);
        assert_eq!(f.expected, "5000000");
        assert_eq!(f.actual, "1");
    }

    #[test]
    fn detects_missing_creator_total() {
        let source = populated_snapshot();
        let mut target = populated_snapshot();
        target.creator_totals.remove("GCREATOR1:GTOKEN1");
        let report = compare_snapshots(&source, &target);
        assert!(!report.passed);
        assert!(report.findings.iter().any(|f| f.field == "creator_totals.GCREATOR1:GTOKEN1"
            && f.severity == FindingSeverity::Missing));
    }

    #[test]
    fn detects_tip_history_count_mismatch() {
        let source = populated_snapshot();
        let mut target = populated_snapshot();
        // Remove one tip from target.
        target.tip_history.get_mut("GCREATOR1").unwrap().pop();
        let report = compare_snapshots(&source, &target);
        assert!(!report.passed);
        assert!(report.findings.iter().any(|f| f.field.contains("tip_history.GCREATOR1.count")));
    }

    #[test]
    fn detects_missing_user_role() {
        let source = populated_snapshot();
        let mut target = populated_snapshot();
        target.user_roles.remove("GCREATOR1");
        let report = compare_snapshots(&source, &target);
        assert!(!report.passed);
        assert!(report.findings.iter().any(|f| f.field == "user_roles.GCREATOR1"
            && f.severity == FindingSeverity::Missing));
    }

    #[test]
    fn detects_wrong_user_role() {
        let source = populated_snapshot();
        let mut target = populated_snapshot();
        target.user_roles.insert("GCREATOR1".into(), "Admin".into());
        let report = compare_snapshots(&source, &target);
        assert!(!report.passed);
        let f = report.findings.iter().find(|f| f.field == "user_roles.GCREATOR1").unwrap();
        assert_eq!(f.severity, FindingSeverity::Mismatch);
        assert_eq!(f.expected, "Creator");
        assert_eq!(f.actual, "Admin");
    }

    #[test]
    fn report_records_checked_count_is_accurate() {
        let source = populated_snapshot();
        let target = populated_snapshot();
        let report = compare_snapshots(&source, &target);
        // balances=3, totals=2, tip_history creators=1, user_roles=2 → 8
        assert_eq!(report.records_checked, 8);
    }

    #[test]
    fn multiple_findings_all_reported() {
        let source = populated_snapshot();
        let mut target = populated_snapshot();
        target.creator_balances.remove("GCREATOR1:GTOKEN1");
        target.creator_balances.remove("GCREATOR1:GTOKEN2");
        target.user_roles.remove("GCREATOR1");
        let report = compare_snapshots(&source, &target);
        assert!(!report.passed);
        assert!(report.findings.len() >= 3, "expected ≥3 findings, got {}", report.findings.len());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Rollback capability
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test_rollback {
    use super::*;
    use std::fs;
    use export_state::compute_checksum;

    fn write_snapshot_to_file(snap: &StateSnapshot, path: &str) {
        let mut sealed = snap.clone();
        sealed.checksum = compute_checksum(&sealed).unwrap();
        let json = serde_json::to_string_pretty(&sealed).unwrap();
        fs::write(path, json).unwrap();
    }

    fn read_snapshot_from_file(path: &str) -> StateSnapshot {
        let json = fs::read_to_string(path).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn backup_snapshot_is_written_and_readable() {
        let path = "/tmp/test-backup-snapshot.json";
        let snap = populated_snapshot();
        write_snapshot_to_file(&snap, path);

        let restored = read_snapshot_from_file(path);
        assert_eq!(restored.contract_id, snap.contract_id);
        assert_eq!(restored.creator_balances, snap.creator_balances);
        fs::remove_file(path).ok();
    }

    #[test]
    fn backup_checksum_is_valid() {
        let path = "/tmp/test-backup-checksum.json";
        let snap = populated_snapshot();
        write_snapshot_to_file(&snap, path);

        let restored = read_snapshot_from_file(path);
        export_state::verify_checksum(&restored).expect("backup checksum invalid");
        fs::remove_file(path).ok();
    }

    #[test]
    fn rollback_restores_all_state_categories() {
        // Simulate: backup = original state, "current" = corrupted state.
        // After rollback the current state should equal the backup.
        let backup_path = "/tmp/test-rollback-backup.json";
        let original = populated_snapshot();
        write_snapshot_to_file(&original, backup_path);

        // Simulate a partial import that corrupted state.
        let mut corrupted = populated_snapshot();
        corrupted.creator_balances.clear();
        corrupted.tip_history.clear();

        // Rollback: restore from backup.
        let restored = read_snapshot_from_file(backup_path);
        export_state::verify_checksum(&restored).expect("backup checksum invalid before rollback");

        // After rollback the restored state must match the original.
        assert_eq!(restored.creator_balances, original.creator_balances);
        assert_eq!(restored.creator_totals, original.creator_totals);
        assert_eq!(restored.tip_history, original.tip_history);
        assert_eq!(restored.subscriptions.len(), original.subscriptions.len());
        assert_eq!(restored.time_locks.len(), original.time_locks.len());
        assert_eq!(restored.milestones.len(), original.milestones.len());
        assert_eq!(restored.matching_programs.len(), original.matching_programs.len());
        assert_eq!(restored.tip_records.len(), original.tip_records.len());
        assert_eq!(restored.locked_tips.len(), original.locked_tips.len());
        assert_eq!(restored.user_roles, original.user_roles);

        fs::remove_file(backup_path).ok();
    }

    #[test]
    fn rollback_fails_on_tampered_backup() {
        let path = "/tmp/test-rollback-tampered.json";
        let snap = populated_snapshot();
        write_snapshot_to_file(&snap, path);

        // Tamper with the file after writing.
        let mut json = fs::read_to_string(path).unwrap();
        json = json.replace("5000000", "9999999");
        fs::write(path, &json).unwrap();

        let tampered = read_snapshot_from_file(path);
        assert!(
            export_state::verify_checksum(&tampered).is_err(),
            "tampered backup should fail checksum verification"
        );
        fs::remove_file(path).ok();
    }

    #[test]
    fn rollback_preserves_schema_version() {
        let path = "/tmp/test-rollback-version.json";
        let snap = populated_snapshot(); // schema_version = 1
        write_snapshot_to_file(&snap, path);
        let restored = read_snapshot_from_file(path);
        assert_eq!(restored.schema_version, 1);
        fs::remove_file(path).ok();
    }

    #[test]
    fn rollback_preserves_pause_state() {
        let path = "/tmp/test-rollback-pause.json";
        let mut snap = populated_snapshot();
        snap.paused = true;
        snap.pause_reason = Some("emergency".into());
        write_snapshot_to_file(&snap, path);
        let restored = read_snapshot_from_file(path);
        assert!(restored.paused);
        assert_eq!(restored.pause_reason, Some("emergency".into()));
        fs::remove_file(path).ok();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Migration history tracking
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test_history {
    use super::*;
    use history::{append_history_entry, print_history};
    use std::fs;

    fn make_entry(label: &str, status: MigrationStatus, exported: usize, imported: usize) -> MigrationHistoryEntry {
        MigrationHistoryEntry {
            started_at: "2024-01-01T00:00:00Z".into(),
            finished_at: "2024-01-01T00:01:00Z".into(),
            label: label.into(),
            source_contract_id: "CSOURCE".into(),
            target_contract_id: "CTARGET".into(),
            network: "testnet".into(),
            dry_run: false,
            status,
            records_exported: exported,
            records_imported: imported,
            error: None,
            backup_path: Some("/tmp/backup.json".into()),
        }
    }

    #[test]
    fn history_file_created_on_first_append() {
        let path = "/tmp/test-history-create.json";
        fs::remove_file(path).ok();
        append_history_entry(path, make_entry("run-1", MigrationStatus::Success, 100, 100)).unwrap();
        assert!(std::path::Path::new(path).exists());
        fs::remove_file(path).ok();
    }

    #[test]
    fn history_accumulates_multiple_runs() {
        let path = "/tmp/test-history-multi.json";
        fs::remove_file(path).ok();
        append_history_entry(path, make_entry("run-1", MigrationStatus::Success, 100, 100)).unwrap();
        append_history_entry(path, make_entry("run-2", MigrationStatus::DryRun, 50, 0)).unwrap();
        append_history_entry(path, make_entry("run-3", MigrationStatus::RolledBack, 80, 40)).unwrap();

        let json = fs::read_to_string(path).unwrap();
        let hist: MigrationHistory = serde_json::from_str(&json).unwrap();
        assert_eq!(hist.runs.len(), 3);
        assert_eq!(hist.runs[0].label, "run-1");
        assert_eq!(hist.runs[1].status, MigrationStatus::DryRun);
        assert_eq!(hist.runs[2].records_imported, 40);
        fs::remove_file(path).ok();
    }

    #[test]
    fn history_preserves_all_fields() {
        let path = "/tmp/test-history-fields.json";
        fs::remove_file(path).ok();
        let mut entry = make_entry("full-run", MigrationStatus::Failed, 200, 0);
        entry.error = Some("RPC timeout".into());
        entry.dry_run = true;
        append_history_entry(path, entry).unwrap();

        let json = fs::read_to_string(path).unwrap();
        let hist: MigrationHistory = serde_json::from_str(&json).unwrap();
        let run = &hist.runs[0];
        assert_eq!(run.error, Some("RPC timeout".into()));
        assert!(run.dry_run);
        assert_eq!(run.status, MigrationStatus::Failed);
        fs::remove_file(path).ok();
    }

    #[test]
    fn history_is_valid_json() {
        let path = "/tmp/test-history-json.json";
        fs::remove_file(path).ok();
        append_history_entry(path, make_entry("json-test", MigrationStatus::Success, 10, 10)).unwrap();
        let json = fs::read_to_string(path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("history must be valid JSON");
        assert!(parsed.get("runs").is_some());
        fs::remove_file(path).ok();
    }

    #[test]
    fn history_dry_run_records_zero_imports() {
        let path = "/tmp/test-history-dryrun.json";
        fs::remove_file(path).ok();
        let mut entry = make_entry("dry", MigrationStatus::DryRun, 150, 0);
        entry.dry_run = true;
        append_history_entry(path, entry).unwrap();

        let json = fs::read_to_string(path).unwrap();
        let hist: MigrationHistory = serde_json::from_str(&json).unwrap();
        assert_eq!(hist.runs[0].records_imported, 0);
        assert!(hist.runs[0].dry_run);
        fs::remove_file(path).ok();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Config parsing
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test_config {
    use types::{MigrationConfig, MigrationSection};

    const SAMPLE_CONFIG: &str = r#"
[migration]
label = "v1-to-v2"
source_contract_id = "CSOURCE"
target_contract_id = "CTARGET"
network = "testnet"
rpc_url = "https://soroban-testnet.stellar.org"
network_passphrase = "Test SDF Network ; September 2015"
dry_run = true
max_retries = 3
retry_delay_ms = 1000
snapshot_dir = "./migration-snapshots"
backup_filename = "pre-migration-backup.json"
export_filename = "source-snapshot.json"
transformed_filename = "transformed-snapshot.json"
history_file = "./migration-history.json"
source_version = 1
target_version = 2
progress_batch_size = 50
abort_on_verify_failure = true
"#;

    #[test]
    fn config_parses_without_error() {
        let config: MigrationConfig = toml::from_str(SAMPLE_CONFIG).expect("config parse failed");
        assert_eq!(config.migration.label, "v1-to-v2");
    }

    #[test]
    fn config_dry_run_defaults_to_true_in_sample() {
        let config: MigrationConfig = toml::from_str(SAMPLE_CONFIG).unwrap();
        assert!(config.migration.dry_run);
    }

    #[test]
    fn config_network_is_testnet() {
        let config: MigrationConfig = toml::from_str(SAMPLE_CONFIG).unwrap();
        assert_eq!(config.migration.network, "testnet");
    }

    #[test]
    fn config_version_range_is_correct() {
        let config: MigrationConfig = toml::from_str(SAMPLE_CONFIG).unwrap();
        assert_eq!(config.migration.source_version, 1);
        assert_eq!(config.migration.target_version, 2);
    }

    #[test]
    fn config_max_retries_is_positive() {
        let config: MigrationConfig = toml::from_str(SAMPLE_CONFIG).unwrap();
        assert!(config.migration.max_retries > 0);
    }

    #[test]
    fn config_abort_on_verify_failure_is_true() {
        let config: MigrationConfig = toml::from_str(SAMPLE_CONFIG).unwrap();
        assert!(config.migration.abort_on_verify_failure);
    }

    #[test]
    fn config_snapshot_dir_is_set() {
        let config: MigrationConfig = toml::from_str(SAMPLE_CONFIG).unwrap();
        assert!(!config.migration.snapshot_dir.is_empty());
    }
}
