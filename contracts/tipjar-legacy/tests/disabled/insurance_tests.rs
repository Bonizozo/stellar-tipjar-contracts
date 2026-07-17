#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, BytesN as _, Ledger},
    Address, BytesN, Env, Vec,
};
use tipjar::{ClaimStatus, InsurancePoolConfig, TipJarContract, TipJarContractClient, TipJarError};

fn setup() -> (
    Env,
    TipJarContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, TipJarContract);
    let client = TipJarContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);

    // Deploy a mock token.
    let token_id = env.register_stellar_asset_contract(token_admin.clone());

    client.init(&admin, &0u32, &0u64);
    client.add_token(&admin, &token_id);

    // Fund sender with tokens.
    let sender = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&sender, &1_000_000i128);

    (env, client, sender, admin, token_id)
}

#[test]
fn test_insurance_config() {
    let (env, client, _sender, admin, _token) = setup();

    client.insurance_set_config(
        &admin, &100i128,   // min_contribution
        &10000i128, // max_contribution
        &200u32,    // premium_rate_bps (2%)
        &5000u32,   // payout_ratio_bps (50%)
        &3600u64,   // claim_cooldown
        &100u32,    // admin_fee_bps
        &100u32,    // tip_premium_bps (1%)
    );

    let config = client.insurance_get_config();
    assert_eq!(config.min_contribution, 100);
    assert_eq!(config.max_contribution, 10000);
    assert_eq!(config.premium_rate_bps, 200);
    assert_eq!(config.payout_ratio_bps, 5000);
    assert_eq!(config.claim_cooldown, 3600);
    assert_eq!(config.tip_premium_bps, 100);
}

#[test]
fn test_insurance_manual_contribution() {
    let (env, client, creator, admin, token) = setup();

    // Configure insurance
    client.insurance_set_config(&admin, &100, &10000, &200, &5000, &3600, &100, &100);

    // Fund creator for contribution
    let token_asset = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_asset.mint(&creator, &1000i128);

    client.insurance_contribute(&creator, &token, &1000i128);

    let pool = client.insurance_get_pool(&token).unwrap();
    // amount(1000) - premium(1000 * 200 / 10000 = 20) = 980
    assert_eq!(pool.total_reserves, 980);
    assert_eq!(pool.total_contributions, 1000);

    let contrib = client.insurance_get_contribution(&creator, &token);
    assert_eq!(contrib, 1000);

    let coverage = client.insurance_get_coverage(&creator, &token);
    // (contrib(1000) + premium_earned(0)) * 50% = 500
    assert_eq!(coverage, 500);
}

#[test]
fn test_tip_automatic_premium() {
    let (env, client, sender, admin, token) = setup();
    let creator = Address::generate(&env);

    // Configure insurance with 1% tip premium
    client.insurance_set_config(&admin, &100, &10000, &200, &5000, &3600, &100, &100);

    client.tip(&sender, &creator, &token, &10000i128);

    let pool = client.insurance_get_pool(&token).unwrap();
    // 1% of 10000 = 100
    assert_eq!(pool.total_reserves, 100);

    let creator_bal = client.get_creator_balance(&creator, &token);
    // 10000 - 100 (premium) = 9900 (fee_bps is 0 in setup)
    assert_eq!(creator_bal, 9900);

    let coverage = client.insurance_get_coverage(&creator, &token);
    // Net received = 9900. Premium earned = 9900 * 1% = 99.
    // Coverage = (0 + 99) * 50% = 49 (integer math)
    assert_eq!(coverage, 49);
}

#[test]
fn test_insurance_claim_flow() {
    let (env, client, creator, admin, token) = setup();
    let sender = Address::generate(&env);

    // Fund sender
    let token_asset = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_asset.mint(&sender, &100000i128);

    // Configure insurance: 1% premium, 100% payout ratio for simplicity
    client.insurance_set_config(
        &admin, &100, &100000, &200, &10000, &3600, &100, &1000, // 10% premium
    );

    // Tip creator to build pool and coverage
    client.tip(&sender, &creator, &token, &10000i128);
    // Pool reserves = 1000. Creator coverage = 9000 * 10% * 100% = 900.

    let tx_hash = BytesN::from_array(&env, &[0u8; 32]);
    let claim_id = client.insurance_submit_claim(&creator, &token, &500i128, &tx_hash);

    let claim = client.insurance_get_claim(&claim_id);
    assert_eq!(claim.amount, 500);
    assert!(matches!(claim.status, ClaimStatus::Pending));

    let creator_claims = client.insurance_get_claims_by_creator(&creator, &token);
    assert_eq!(creator_claims.len(), 1);
    assert_eq!(creator_claims.get(0).unwrap(), claim_id);

    // Approve claim
    client.insurance_approve_claim(&admin, &claim_id);
    let claim = client.insurance_get_claim(&claim_id);
    assert!(matches!(claim.status, ClaimStatus::Approved));

    // Pay claim
    let old_creator_bal = token_asset.balance(&creator);
    client.insurance_pay_claim(&admin, &claim_id);

    let claim = client.insurance_get_claim(&claim_id);
    assert!(matches!(claim.status, ClaimStatus::Paid));

    let new_creator_bal = token_asset.balance(&creator);
    assert_eq!(new_creator_bal - old_creator_bal, 500);

    let pool = client.insurance_get_pool(&token).unwrap();
    assert_eq!(pool.total_reserves, 500); // 1000 - 500
    assert_eq!(pool.total_claims_paid, 500);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #55)")] // NoCoverage
fn test_claim_without_coverage_fails() {
    let (env, client, creator, admin, token) = setup();

    client.insurance_set_config(&admin, &100, &10000, &200, &5000, &3600, &100, &100);

    let tx_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.insurance_submit_claim(&creator, &token, &500i128, &tx_hash);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #65)")] // PayoutExceedsReserves (actually exceeds creator coverage)
fn test_claim_exceeding_coverage_fails() {
    let (env, client, sender, admin, token) = setup();
    let creator = Address::generate(&env);

    client.insurance_set_config(
        &admin, &100, &10000, &200, &5000, &3600, &100, &100, // 50% payout ratio
    );

    // Tip to build coverage
    client.tip(&sender, &creator, &token, &2000i128);
    // Net=1980. Premium=20. Coverage = 19 * 50% = 9.

    let tx_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.insurance_submit_claim(&creator, &token, &100i128, &tx_hash); // 100 > 9
}

#[test]
fn test_insurance_batch_processing() {
    let (env, client, creator, admin, token) = setup();
    let sender = Address::generate(&env);

    // Fund sender
    let token_asset = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_asset.mint(&sender, &100000i128);

    // Configure insurance
    client.insurance_set_config(&admin, &100, &100000, &200, &10000, &3600, &100, &1000);

    // Tip creator to build coverage
    client.tip(&sender, &creator, &token, &20000i128);
    // Pool reserves = 2000. Coverage = 18000 * 10% * 100% = 1800.

    let tx_hash = BytesN::from_array(&env, &[0u8; 32]);
    let claim_id_1 = client.insurance_submit_claim(&creator, &token, &500i128, &tx_hash);
    let claim_id_2 = client.insurance_submit_claim(&creator, &token, &300i128, &tx_hash);

    let mut claim_ids = Vec::new(&env);
    claim_ids.push_back(claim_id_1);
    claim_ids.push_back(claim_id_2);

    // Batch approve
    client.insurance_process_claims_batch(
        &admin,
        &claim_ids,
        &soroban_sdk::String::from_str(&env, "approve"),
    );

    assert!(matches!(
        client.insurance_get_claim(&claim_id_1).status,
        ClaimStatus::Approved
    ));
    assert!(matches!(
        client.insurance_get_claim(&claim_id_2).status,
        ClaimStatus::Approved
    ));

    // Batch pay
    let old_creator_bal = token_asset.balance(&creator);
    client.insurance_process_claims_batch(
        &admin,
        &claim_ids,
        &soroban_sdk::String::from_str(&env, "pay"),
    );

    assert!(matches!(
        client.insurance_get_claim(&claim_id_1).status,
        ClaimStatus::Paid
    ));
    assert!(matches!(
        client.insurance_get_claim(&claim_id_2).status,
        ClaimStatus::Paid
    ));

    let new_creator_bal = token_asset.balance(&creator);
    assert_eq!(new_creator_bal - old_creator_bal, 800);

    let pool = client.insurance_get_pool(&token).unwrap();
    assert_eq!(pool.total_reserves, 1200); // 2000 - 800
}
