//! Synthetic Assets Module
//!
//! This module provides functionality for creating and managing synthetic assets
//! backed by creator tip pools. Synthetic assets allow users to gain exposure to
//! creator performance while providing creators with upfront liquidity.

pub mod admin;
pub mod events;
pub mod minting;
pub mod oracle;
pub mod queries;
pub mod redemption;
pub mod supply;
pub mod types;

pub use events::*;
pub use types::*;
