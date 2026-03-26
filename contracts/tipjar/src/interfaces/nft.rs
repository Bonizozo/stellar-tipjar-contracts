use soroban_sdk::{contractclient, Address, Env, String};

/// Minimal NFT interface for minting reward tokens.
#[contractclient(name = "NftClient")]
pub trait NftInterface {
    /// Mints an NFT to `recipient` with the given `metadata` URI.
    fn mint(env: Env, recipient: Address, metadata: String);
}
