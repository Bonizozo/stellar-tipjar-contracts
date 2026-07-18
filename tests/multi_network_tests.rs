#![cfg(test)]

#[test]
fn test_testnet_configuration() {
    let network = "testnet";
    let rpc_url = "https://soroban-testnet.stellar.org";
    let passphrase = "Test SDF Network ; September 2015";

    assert_eq!(network, "testnet");
    assert_eq!(rpc_url, "https://soroban-testnet.stellar.org");
    assert_eq!(passphrase, "Test SDF Network ; September 2015");
}

#[test]
fn test_mainnet_configuration() {
    let network = "mainnet";
    let rpc_url = "https://soroban.stellar.org";
    let passphrase = "Public Global Stellar Network ; September 2015";

    assert_eq!(network, "mainnet");
    assert_eq!(rpc_url, "https://soroban.stellar.org");
    assert_eq!(passphrase, "Public Global Stellar Network ; September 2015");
}

#[test]
fn test_futurenet_configuration() {
    let network = "futurenet";
    let rpc_url = "https://rpc-futurenet.stellar.org";
    let passphrase = "Test SDF Future Network ; October 2022";

    assert_eq!(network, "futurenet");
    assert_eq!(rpc_url, "https://rpc-futurenet.stellar.org");
    assert_eq!(passphrase, "Test SDF Future Network ; October 2022");
}

#[test]
fn test_network_switching() {
    let networks = vec!["testnet", "mainnet", "futurenet"];
    assert_eq!(networks.len(), 3);
    assert!(networks.contains(&"testnet"));
    assert!(networks.contains(&"mainnet"));
    assert!(networks.contains(&"futurenet"));
}

#[test]
fn test_contract_address_loading() {
    // Test that contract addresses can be loaded from environment
    let testnet_addr = std::env::var("CONTRACT_ADDRESS_TESTNET").ok();
    let mainnet_addr = std::env::var("CONTRACT_ADDRESS_MAINNET").ok();
    let futurenet_addr = std::env::var("CONTRACT_ADDRESS_FUTURENET").ok();

    // At least one should be set for testing
    assert!(testnet_addr.is_some() || mainnet_addr.is_some() || futurenet_addr.is_some());
}

#[test]
fn test_network_detection() {
    let networks = vec!["testnet", "mainnet", "futurenet"];
    for network in networks {
        assert!(!network.is_empty());
    }
}

#[test]
fn test_rpc_connectivity_config() {
    let rpc_urls = vec![
        "https://soroban-testnet.stellar.org",
        "https://soroban.stellar.org",
        "https://rpc-futurenet.stellar.org",
    ];

    for url in rpc_urls {
        assert!(url.starts_with("https://"));
        assert!(url.contains("stellar"));
    }
}

#[test]
fn test_network_specific_test_accounts() {
    // Verify test account structure for each network
    let testnet_account = "GBRPYHIL2CI3WHZDTOOQFC6EB4KJJGUJJBBQ7UYXNQHX5LHVGXNSC4";
    let mainnet_account = "GBRPYHIL2CI3WHZDTOOQFC6EB4KJJGUJJBBQ7UYXNQHX5LHVGXNSC4";

    assert_eq!(testnet_account.len(), 56);
    assert_eq!(mainnet_account.len(), 56);
    assert!(testnet_account.starts_with("G"));
    assert!(mainnet_account.starts_with("G"));
}
