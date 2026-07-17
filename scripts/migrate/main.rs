//! TipJar Contract State Migration Toolkit
//!
//! # Subcommands
//! | Command     | Description                                              |
//! |-------------|----------------------------------------------------------|
//! | `export`    | Export source contract state to a JSON snapshot          |
//! | `transform` | Apply schema transformation rules to a snapshot          |
//! | `import`    | Import a transformed snapshot into the target contract   |
//! | `verify`    | Compare snapshot against live target contract state      |
//! | `rollback`  | Restore target contract from a backup snapshot           |
//! | `history`   | Print the migration history log                          |
//!
//! # Quick start
//! ```bash
//! # 1. Copy and edit the config
//! cp scripts/migrate/config.toml config.local.toml
//! # (edit source_contract_id, target_contract_id, network, dry_run)
//!
//! # 2. Export (dry-run first)
//! cargo run --bin migrate -- export --config config.local.toml --dry-run
//!
//! # 3. Transform
//! cargo run --bin migrate -- transform --config config.local.toml
//!
//! # 4. Import (dry-run first)
//! cargo run --bin migrate -- import --config config.local.toml --dry-run
//!
//! # 5. Import for real
//! cargo run --bin migrate -- import --config config.local.toml
//!
//! # 6. Verify
//! cargo run --bin migrate -- verify --config config.local.toml
//!
//! # 7. View history
//! cargo run --bin migrate -- history --config config.local.toml
//! ```

mod export_state;
mod history;
mod import_state;
mod rpc_client;
mod transform_state;
mod types;
mod verify_migration;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let result = match args[1].as_str() {
        "export" => export_state::run_export(&args[2..].to_vec()),
        "transform" => transform_state::run_transform(&args[2..].to_vec()),
        "import" => import_state::run_import(&args[2..].to_vec()),
        "verify" => verify_migration::run_verify(&args[2..].to_vec()),
        "rollback" => run_rollback(&args[2..].to_vec()),
        "history" => run_history(&args[2..].to_vec()),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        cmd => Err(format!("Unknown subcommand: '{}'. Run with 'help' for usage.", cmd)),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_rollback(args: &[String]) -> Result<(), String> {
    let config_path = parse_flag(args, "--config")
        .unwrap_or_else(|| "scripts/migrate/config.toml".to_string());
    let backup_override = parse_flag(args, "--backup");

    let toml_str = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Cannot read config {}: {}", config_path, e))?;
    let config: types::MigrationConfig = toml::from_str(&toml_str)
        .map_err(|e| format!("Config parse error: {}", e))?;

    let backup_path = backup_override.unwrap_or_else(|| {
        format!(
            "{}/{}",
            config.migration.snapshot_dir, config.migration.backup_filename
        )
    });

    let migrator = export_state::StateMigrator::new(config.clone());
    migrator.rollback(&backup_path, &config.migration.target_contract_id)?;
    Ok(())
}

fn run_history(args: &[String]) -> Result<(), String> {
    let config_path = parse_flag(args, "--config")
        .unwrap_or_else(|| "scripts/migrate/config.toml".to_string());

    let toml_str = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Cannot read config {}: {}", config_path, e))?;
    let config: types::MigrationConfig = toml::from_str(&toml_str)
        .map_err(|e| format!("Config parse error: {}", e))?;

    history::print_history(&config.migration.history_file)
}

fn parse_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
}

fn print_usage() {
    println!(
        r#"
TipJar Contract State Migration Toolkit

USAGE:
    migrate <SUBCOMMAND> [OPTIONS]

SUBCOMMANDS:
    export      Export source contract state to a JSON snapshot
    transform   Apply schema transformation rules to a snapshot
    import      Import a transformed snapshot into the target contract
    verify      Compare snapshot against live target contract state
    rollback    Restore target contract from a backup snapshot
    history     Print the migration history log
    help        Print this help message

OPTIONS (all subcommands):
    --config <PATH>     Path to config.toml  [default: scripts/migrate/config.toml]
    --dry-run           Override config dry_run to true (export/import only)

OPTIONS (transform):
    --input  <PATH>     Input snapshot path  [default: from config]
    --output <PATH>     Output snapshot path [default: from config]
    --from-version <N>  Source schema version override
    --to-version   <N>  Target schema version override

OPTIONS (verify):
    --snapshot <PATH>   Snapshot to verify against [default: transformed snapshot]

OPTIONS (rollback):
    --backup <PATH>     Backup snapshot to restore from [default: from config]

ENVIRONMENT VARIABLES:
    MIGRATION_ADMIN_SECRET    Stellar secret key for signing write transactions
    MIGRATION_ADMIN_ADDRESS   Stellar public key of the admin account
"#
    );
}
