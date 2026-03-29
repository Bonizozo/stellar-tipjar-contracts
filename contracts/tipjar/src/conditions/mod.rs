//! Conditional tip execution module.
//!
//! Conditions can reference on-chain state (ledger/time/balance) or off-chain
//! oracle approvals tracked in contract storage.

pub mod evaluator;
pub mod types;
