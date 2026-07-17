/// Network configuration management
use serde::{Deserialize, Serialize};

/// Supported Stellar networks
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Network {
    /// Stellar testnet for development and testing
    Testnet,
    /// Stellar mainnet for production
    Mainnet,
    /// Stellar futurenet for experimental features
    Futurenet,
}

impl Network {
    /// Get RPC endpoint URL for the network
    pub fn rpc_url(&self) -> &str {
        match self {
            Network::Testnet => "https://soroban-testnet.stellar.org",
            Network::Mainnet => "https://soroban.stellar.org",
            Network::Futurenet => "https://rpc-futurenet.stellar.org",
        }
    }

    /// Get network passphrase for transaction signing
    pub fn passphrase(&self) -> &str {
        match self {
            Network::Testnet => "Test SDF Network ; September 2015",
            Network::Mainnet => "Public Global Stellar Network ; September 2015",
            Network::Futurenet => "Test SDF Future Network ; October 2022",
        }
    }

    /// Get network name
    pub fn name(&self) -> &str {
        match self {
            Network::Testnet => "testnet",
            Network::Mainnet => "mainnet",
            Network::Futurenet => "futurenet",
        }
    }

    /// Get network ID
    pub fn network_id(&self) -> u32 {
        match self {
            Network::Testnet => 0,
            Network::Mainnet => 1,
            Network::Futurenet => 2,
        }
    }

    /// Parse network from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "testnet" => Some(Network::Testnet),
            "mainnet" => Some(Network::Mainnet),
            "futurenet" => Some(Network::Futurenet),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_rpc_urls() {
        assert_eq!(
            Network::Testnet.rpc_url(),
            "https://soroban-testnet.stellar.org"
        );
        assert_eq!(Network::Mainnet.rpc_url(), "https://soroban.stellar.org");
        assert_eq!(
            Network::Futurenet.rpc_url(),
            "https://rpc-futurenet.stellar.org"
        );
    }

    #[test]
    fn test_network_passphrases() {
        assert_eq!(
            Network::Testnet.passphrase(),
            "Test SDF Network ; September 2015"
        );
        assert_eq!(
            Network::Mainnet.passphrase(),
            "Public Global Stellar Network ; September 2015"
        );
        assert_eq!(
            Network::Futurenet.passphrase(),
            "Test SDF Future Network ; October 2022"
        );
    }

    #[test]
    fn test_network_parsing() {
        assert_eq!(Network::from_str("testnet"), Some(Network::Testnet));
        assert_eq!(Network::from_str("mainnet"), Some(Network::Mainnet));
        assert_eq!(Network::from_str("futurenet"), Some(Network::Futurenet));
        assert_eq!(Network::from_str("invalid"), None);
    }
}
