//! Migration history log — persists every run to a local JSON file.
//!
//! The history file is a JSON object with a single `"runs"` array.
//! Each entry records the outcome of one migration run so operators can
//! audit what happened and when.

use std::fs;
use std::path::Path;

use crate::types::{MigrationHistory, MigrationHistoryEntry};

/// Appends `entry` to the history file at `path`.
///
/// Creates the file (and parent directories) if it does not exist.
pub fn append_history_entry(path: &str, entry: MigrationHistoryEntry) -> Result<(), String> {
    let file_path = Path::new(path);

    // Load existing history or start fresh.
    let mut history: MigrationHistory = if file_path.exists() {
        let json = fs::read_to_string(file_path)
            .map_err(|e| format!("Cannot read history file {}: {}", path, e))?;
        serde_json::from_str(&json).unwrap_or_default()
    } else {
        MigrationHistory::default()
    };

    history.runs.push(entry);

    // Write back.
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create history dir: {}", e))?;
    }
    let json = serde_json::to_string_pretty(&history)
        .map_err(|e| format!("History serialisation error: {}", e))?;
    fs::write(file_path, json)
        .map_err(|e| format!("Cannot write history file {}: {}", path, e))?;

    Ok(())
}

/// Reads and prints the migration history from `path`.
pub fn print_history(path: &str) -> Result<(), String> {
    let json = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read history file {}: {}", path, e))?;
    let history: MigrationHistory = serde_json::from_str(&json)
        .map_err(|e| format!("History parse error: {}", e))?;

    println!("Migration History — {} run(s)", history.runs.len());
    println!("{:-<70}", "");
    for (i, run) in history.runs.iter().enumerate() {
        println!(
            "[{}] {} | {} → {} | {} | {:?} | exported={} imported={}{}",
            i + 1,
            run.started_at,
            run.source_contract_id,
            run.target_contract_id,
            run.network,
            run.status,
            run.records_exported,
            run.records_imported,
            run.error
                .as_deref()
                .map(|e| format!(" | ERROR: {}", e))
                .unwrap_or_default(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MigrationStatus;
    use std::fs;

    #[test]
    fn test_append_and_read_history() {
        let path = "/tmp/test-migration-history.json";
        // Clean up from previous runs.
        let _ = fs::remove_file(path);

        let entry = MigrationHistoryEntry {
            started_at: "2024-01-01T00:00:00Z".into(),
            finished_at: "2024-01-01T00:01:00Z".into(),
            label: "test-run".into(),
            source_contract_id: "CSOURCE".into(),
            target_contract_id: "CTARGET".into(),
            network: "testnet".into(),
            dry_run: false,
            status: MigrationStatus::Success,
            records_exported: 100,
            records_imported: 100,
            error: None,
            backup_path: Some("/tmp/backup.json".into()),
        };

        append_history_entry(path, entry.clone()).unwrap();

        let json = fs::read_to_string(path).unwrap();
        let history: MigrationHistory = serde_json::from_str(&json).unwrap();
        assert_eq!(history.runs.len(), 1);
        assert_eq!(history.runs[0].label, "test-run");
        assert_eq!(history.runs[0].records_exported, 100);

        // Append a second entry.
        let entry2 = MigrationHistoryEntry {
            started_at: "2024-01-02T00:00:00Z".into(),
            finished_at: "2024-01-02T00:01:00Z".into(),
            label: "test-run-2".into(),
            source_contract_id: "CSOURCE".into(),
            target_contract_id: "CTARGET".into(),
            network: "testnet".into(),
            dry_run: true,
            status: MigrationStatus::DryRun,
            records_exported: 50,
            records_imported: 0,
            error: None,
            backup_path: None,
        };
        append_history_entry(path, entry2).unwrap();

        let json2 = fs::read_to_string(path).unwrap();
        let history2: MigrationHistory = serde_json::from_str(&json2).unwrap();
        assert_eq!(history2.runs.len(), 2);
        assert_eq!(history2.runs[1].status, MigrationStatus::DryRun);

        fs::remove_file(path).unwrap();
    }
}
