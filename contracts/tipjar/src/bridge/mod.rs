pub mod relayer;
pub mod validator;

use soroban_sdk::{contracttype, Address, BytesN, String};

/// Supported source chains for bridged tips.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceChain {
    Ethereum,
    Polygon,
    BinanceSmartChain,
    Avalanche,
    Arbitrum,
}

/// Returns the chain ID for a source chain.
/// These are standard EVM chain IDs used in cross-chain verification.
pub fn chain_id(chain: &SourceChain) -> u64 {
    match chain {
        SourceChain::Ethereum => 1,
        SourceChain::Polygon => 137,
        SourceChain::BinanceSmartChain => 56,
        SourceChain::Avalanche => 43114,
        SourceChain::Arbitrum => 42161,
    }
}

/// Per-chain bridge configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeConfig {
    /// Source chain identifier.
    pub chain: SourceChain,
    /// Whether this chain is enabled for bridging.
    pub enabled: bool,
    /// Minimum tip amount accepted from this chain.
    pub min_amount: i128,
    /// Maximum tip amount accepted from this chain (0 = unlimited).
    pub max_amount: i128,
    /// Required confirmation blocks on source chain.
    pub required_confirmations: u32,
}

/// A structured bridge message for cross-chain verification.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeMessage {
    /// Originating chain.
    pub source_chain: SourceChain,
    /// Unique transaction hash on the source chain.
    pub source_tx_hash: BytesN<32>,
    /// Stellar creator address to receive the tip.
    pub creator: Address,
    /// Amount in the Stellar tip token's smallest unit.
    pub amount: i128,
    /// Message nonce for replay protection.
    pub nonce: u64,
    /// Optional message from the tipper.
    pub message: String,
    /// Merkle proof root for cross-chain verification.
    pub proof_root: Option<BytesN<32>>,
}

/// Bridge fee information.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeFee {
    /// Fee amount deducted.
    pub fee_amount: i128,
    /// Fee in basis points.
    pub fee_bps: u32,
    /// Net amount credited to creator.
    pub net_amount: i128,
}

/// A bridge tip request submitted by a relayer.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeTip {
    /// Originating chain.
    pub source_chain: SourceChain,
    /// Unique transaction hash on the source chain (32 bytes).
    pub source_tx_hash: BytesN<32>,
    /// Stellar creator address to receive the tip.
    pub creator: Address,
    /// Amount in the Stellar tip token's smallest unit.
    pub amount: i128,
    /// Optional message from the tipper.
    pub message: String,
}

/// Bridge statistics per source chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeStats {
    /// Total tips received from this chain.
    pub total_tips: u64,
    /// Total amount received from this chain.
    pub total_amount: i128,
    /// Total fees collected from this chain.
    pub total_fees: i128,
}

/// Bridge-specific storage keys to avoid bloating the main DataKey enum.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BridgeDataKey {
    /// Authorized bridge relayer address.
    BridgeRelayer,
    /// Bridge token address.
    BridgeToken,
    /// Processed source transaction hashes for replay protection.
    BridgeProcessed(BytesN<32>),
    /// Bridge fee in basis points.
    BridgeFeeBps,
    /// Bridge feature enabled flag.
    BridgeEnabled,
}
