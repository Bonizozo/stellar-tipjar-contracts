use soroban_sdk::{Address, Env};

use crate::sidechain::{Checkpoint, SidechainState};
use crate::DataKey;

/// Returns the current sidechain state summary.
pub fn get_state(env: &Env) -> SidechainState {
    let enabled: bool = env
        .storage()
        .instance()
        .get(&DataKey::SidechainEnabled)
        .unwrap_or(false);

    let latest_checkpoint: u64 = env
        .storage()
        .instance()
        .get(&DataKey::SidechainLatestCheckpoint)
        .unwrap_or(0);

    // Sum finalized volume across all checkpoints up to latest
    let mut total_finalized_volume: i128 = 0;
    let mut total_checkpoints: u64 = 0;
    for seq in 1..=latest_checkpoint {
        if let Some(cp) = env
            .storage()
            .persistent()
            .get::<DataKey, Checkpoint>(&DataKey::SidechainCheckpoint(seq))
        {
            if cp.finalized {
                total_finalized_volume += cp.total_volume;
                total_checkpoints += 1;
            }
        }
    }

    SidechainState {
        enabled,
        latest_checkpoint,
        total_checkpoints,
        total_finalized_volume,
    }
}

/// Returns the finalized tip total for a creator/token pair from sidechain batches.
pub fn get_finalized_total(env: &Env, creator: &Address, token: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::SidechainFinalizedTotal(
            creator.clone(),
            token.clone(),
        ))
        .unwrap_or(0)
}

/// Returns a specific checkpoint by sequence number.
pub fn get_checkpoint(env: &Env, seq: u64) -> Option<Checkpoint> {
    env.storage()
        .persistent()
        .get(&DataKey::SidechainCheckpoint(seq))
}
