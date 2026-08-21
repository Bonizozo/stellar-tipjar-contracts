/// Contract address management per network
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::networks::Network;

/// Contract addresses for different networks
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractAddresses {
    addresses: HashMap<String, String>,
}

impl ContractAddresses {
    /// Create new contract addresses configuration
    pub fn new() -> Self {
        Self {
            addresses: HashMap::new(),
        }
    }

    /// Set contract address for a network
    pub fn set(&mut self, network: &Network, address: String) {
        self.addresses.insert(network.name().to_string(), address);
    }

    /// Get contract address for a network
    pub fn get(&self, network: &Network) -> Option<&str> {
        self.addresses.get(network.name()).map(|s| s.as_str())
    }

    /// Get all addresses
    pub fn all(&self) -> &HashMap<String, String> {
        &self.addresses
    }

    /// Load from environment variables
    pub fn from_env() -> Self {
        let mut addresses = Self::new();

        if let Ok(testnet_addr) = std::env::var("CONTRACT_ADDRESS_TESTNET") {
            addresses.set(&Network::Testnet, testnet_addr);
        }

        if let Ok(mainnet_addr) = std::env::var("CONTRACT_ADDRESS_MAINNET") {
            addresses.set(&Network::Mainnet, mainnet_addr);
        }

        if let Ok(futurenet_addr) = std::env::var("CONTRACT_ADDRESS_FUTURENET") {
            addresses.set(&Network::Futurenet, futurenet_addr);
        }

        addresses
    }
}

impl Default for ContractAddresses {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get_address() {
        let mut addresses = ContractAddresses::new();
        let testnet_addr = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";

        addresses.set(&Network::Testnet, testnet_addr.to_string());
        assert_eq!(addresses.get(&Network::Testnet), Some(testnet_addr));
    }

    #[test]
    fn test_get_nonexistent_address() {
        let addresses = ContractAddresses::new();
        assert_eq!(addresses.get(&Network::Testnet), None);
    }

    #[test]
    fn test_multiple_networks() {
        let mut addresses = ContractAddresses::new();
        let testnet_addr = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";
        let mainnet_addr = "CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBSC4";

        addresses.set(&Network::Testnet, testnet_addr.to_string());
        addresses.set(&Network::Mainnet, mainnet_addr.to_string());

        assert_eq!(addresses.get(&Network::Testnet), Some(testnet_addr));
        assert_eq!(addresses.get(&Network::Mainnet), Some(mainnet_addr));
    }
}
