use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use soroban_sdk::testutils::Snapshot;
use std::{collections::HashMap, fs, path::Path};

/// Persisted simulation state: a Soroban snapshot plus address labels.
#[derive(Serialize, Deserialize)]
pub struct SimState {
    pub snapshot: Snapshot,
    pub labels: HashMap<String, String>,
}

/// Save a snapshot to `path`.
pub fn save(path: &Path, snapshot: Snapshot, labels: HashMap<String, String>) -> Result<()> {
    let state = SimState { snapshot, labels };
    fs::write(path, serde_json::to_vec_pretty(&state)?).context("write state file")?;
    Ok(())
}

/// Load a previously saved state file.
pub fn load(path: &Path) -> Result<(Snapshot, HashMap<String, String>)> {
    let data = fs::read(path).context("read state file")?;
    let state: SimState = serde_json::from_slice(&data).context("parse state file")?;
    Ok((state.snapshot, state.labels))
}
