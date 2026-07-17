//! Volatility history retrieval helpers.

use soroban_sdk::{Env, Vec};

use super::{get_snapshot, get_snapshot_count, VolatilitySnapshot};

/// Return up to `limit` most-recent snapshots for `index_id` in
/// reverse-chronological order (newest first).
pub fn get_recent_snapshots(env: &Env, index_id: u64, limit: u32) -> Vec<VolatilitySnapshot> {
    let total = get_snapshot_count(env, index_id);
    let mut result: Vec<VolatilitySnapshot> = Vec::new(env);

    if total == 0 {
        return result;
    }

    let count = (limit as u64).min(total);
    // Snapshots are numbered 0 .. total-1; newest is total-1.
    for i in 0..count {
        let seq = total - 1 - i;
        if let Some(snap) = get_snapshot(env, index_id, seq) {
            result.push_back(snap.clone());
        }
    }

    result
}

/// Return the single most-recent snapshot for `index_id`, or `None`.
pub fn get_latest_snapshot(env: &Env, index_id: u64) -> Option<VolatilitySnapshot> {
    let total = get_snapshot_count(env, index_id);
    if total == 0 {
        return None;
    }
    get_snapshot(env, index_id, total - 1)
}

/// Return snapshots within a time range `[start_ts, end_ts]` (inclusive),
/// up to `limit` results, newest first.
pub fn get_snapshots_in_range(
    env: &Env,
    index_id: u64,
    start_ts: u64,
    end_ts: u64,
    limit: u32,
) -> Vec<VolatilitySnapshot> {
    let total = get_snapshot_count(env, index_id);
    let mut result: Vec<VolatilitySnapshot> = Vec::new(env);

    if total == 0 {
        return result;
    }

    let mut collected: u32 = 0;
    // Walk backwards from newest
    let mut seq = total;
    while seq > 0 && collected < limit {
        seq -= 1;
        if let Some(snap) = get_snapshot(env, index_id, seq) {
            if snap.timestamp >= start_ts && snap.timestamp <= end_ts {
                result.push_back(snap.clone());
                collected += 1;
            }
            // Once we go below start_ts we can stop
            if snap.timestamp < start_ts {
                break;
            }
        }
    }

    result
}
