use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{collections::HashMap, path::PathBuf};
use tipjar_simulator::{debugger, state_manager, Simulator};

#[derive(Parser)]
#[command(name = "tipjar-sim", about = "TipJar local contract simulator")]
struct Cli {
    /// Enable verbose debug output.
    #[arg(short, long)]
    verbose: bool,

    /// State file for snapshot persistence.
    #[arg(short, long, default_value = "sim-state.json")]
    state: PathBuf,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a scripted tip-and-withdraw scenario.
    Run {
        /// Number of tip transactions to simulate.
        #[arg(short, long, default_value_t = 5)]
        tips: u32,
        /// Amount per tip.
        #[arg(short, long, default_value_t = 100)]
        amount: i128,
        /// Save state snapshot after run.
        #[arg(long)]
        save: bool,
    },
    /// Restore a saved snapshot and print balances.
    Inspect {
        /// Path to a previously saved state file.
        file: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Run { tips, amount, save } => {
            let mut sim = Simulator::new(cli.verbose);

            let sender = sim.new_address();
            let creator = sim.new_address();
            sim.mint(&sender, amount * tips as i128);

            println!("==> Simulating {tips} tips of {amount} each");
            debugger::dump_ledger(&sim);

            for i in 0..tips {
                let result = sim.simulate_tip(&sender, &creator, amount);
                println!(
                    "  tip[{i}]: ok={} cpu={}",
                    result.ok, result.cpu_instructions
                );
            }

            debugger::dump_balances(&sim, &[("creator", &creator)]);
            debugger::dump_events(&sim);

            println!("==> Simulating withdrawal");
            let result = sim.simulate_withdraw(&creator);
            println!(
                "  withdraw: ok={} cpu={}",
                result.ok, result.cpu_instructions
            );
            debugger::dump_balances(&sim, &[("creator", &creator)]);

            if save {
                let snap = sim.snapshot();
                let mut labels = HashMap::new();
                labels.insert(format!("{creator:?}"), "creator".into());
                labels.insert(format!("{sender:?}"), "sender".into());
                state_manager::save(&cli.state, snap, labels)?;
                println!("==> State saved to {}", cli.state.display());
            }
        }

        Cmd::Inspect { file } => {
            let (snap, labels) = state_manager::load(&file)?;
            let mut sim = Simulator::new(cli.verbose);
            sim.restore_snapshot(snap);
            println!("==> Restored snapshot from {}", file.display());
            println!("    Labels: {labels:?}");
            debugger::dump_ledger(&sim);
        }
    }

    Ok(())
}
