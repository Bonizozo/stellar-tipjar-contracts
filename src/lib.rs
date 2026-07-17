//! TipJar SDK for multi-network support
pub mod config;
pub mod simulation;

pub use config::{ContractAddresses, Network};
pub use simulation::{CostCalculator, PreviewGenerator, TransactionSimulator};
