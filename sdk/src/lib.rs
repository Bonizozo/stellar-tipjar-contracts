//! TipJar SDK for multi-network support.
//!
//! Provides network/contract-address configuration (`config`) and
//! client-side transaction simulation/cost-preview helpers (`simulation`)
//! for consumers integrating with TipJar off-chain.
pub mod config;
pub mod simulation;

pub use config::{ContractAddresses, Network};
pub use simulation::{CostCalculator, PreviewGenerator, TransactionSimulator};
