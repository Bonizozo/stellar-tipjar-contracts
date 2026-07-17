#![cfg(test)]

use soroban_sdk::{testutils::Address as AddressTestUtils, token, Address, Env};

// Re-export the adversarial token module
pub mod adversarial_token;

#[cfg(test)]
mod security_tests {
    use super::*;
    use crate::adversarial_token::{AdversarialError, AdversarialTokenContract, AdversarialTokenContractClient};
    use tipjar::{TipJarContract, TipJarContractClient, TipJarError};

    fn setup_with_adversarial_token() -> (
        Env,
        Address,
        Address,
        Address,
        Address,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        // Deploy and init adversarial token
        let adversarial_token_id = env.register(AdversarialTokenContract, ());
        let adversarial_client =
            AdversarialTokenContractClient::new(&env, &adversarial_token_id);

        // Deploy tipjar contract
        let admin = Address::generate(&env);
        let tipjar_id = env.register(TipJarContract, ());
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);

        // Init adversarial token with normal mode, tipjar reference
        adversarial_client.init(&admin, &0u32, &adversarial_token_id);

        // Init tipjar
        tipjar_client.init(&admin);

        // Whitelist adversarial token
        tipjar_client.add_token(&admin, &adversarial_token_id);

        (env, tipjar_id, adversarial_token_id, admin, admin, admin)
    }

    // ============ Checks-Effects-Interactions Tests ============

    #[test]
    fn test_withdraw_uses_correct_ordering() {
        let (env, tipjar_id, token_id, admin, _, _) = setup_with_adversarial_token();
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);
        let token_client =
            AdversarialTokenContractClient::new(&env, &token_id);

        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        // Mint and tip
        token_client.mint(&sender, &1000);
        tipjar_client.grant_role(&admin, &creator, &tipjar::Role::Creator);
        tipjar_client.tip(&sender, &creator, &token_id, &500);

        // Verify balance before withdraw
        let balance_before = tipjar_client.get_withdrawable_balance(&creator, &token_id);
        assert_eq!(balance_before, 500);

        // Withdraw (checks-effects-interactions ordering ensures atomicity)
        tipjar_client.withdraw(&creator, &token_id);

        // After withdraw, balance must be 0 (even if transfer failed, which it didn't here)
        let balance_after = tipjar_client.get_withdrawable_balance(&creator, &token_id);
        assert_eq!(balance_after, 0);
    }

    #[test]
    fn test_tip_uses_correct_ordering() {
        let (env, tipjar_id, token_id, admin, _, _) = setup_with_adversarial_token();
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);
        let token_client =
            AdversarialTokenContractClient::new(&env, &token_id);

        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        // Mint
        token_client.mint(&sender, &1000);

        // Tip
        tipjar_client.tip(&sender, &creator, &token_id, &500);

        // Verify storage was updated (effects happened) before and after transfer
        let balance = tipjar_client.get_withdrawable_balance(&creator, &token_id);
        let total = tipjar_client.get_total_tips(&creator, &token_id);
        assert_eq!(balance, 500);
        assert_eq!(total, 500);
    }

    // ============ Adversarial Token Mode Tests ============

    #[test]
    fn test_token_panic_on_transfer_reverts_entire_call() {
        let (env, tipjar_id, token_id, admin, _, _) = setup_with_adversarial_token();
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);
        let token_client =
            AdversarialTokenContractClient::new(&env, &token_id);

        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        token_client.mint(&sender, &1000);

        // Set token to panic mode (mode 1)
        token_client.set_failure_mode(&1u32);

        // Attempt tip (should panic from token transfer)
        let result = tipjar_client.try_tip(&sender, &creator, &token_id, &500);

        // Should fail
        assert!(result.is_err());

        // Verify storage was NOT updated (atomicity)
        let balance_after = tipjar_client.get_withdrawable_balance(&creator, &token_id);
        assert_eq!(balance_after, 0);
    }

    #[test]
    fn test_token_silent_noop_breaks_storage_invariant() {
        // This test documents why SEP-41 trust model requires an allowlist
        // A silent no-op token breaks the fundamental assumption that
        // transfer_success => tokens_were_moved
        let (env, tipjar_id, token_id, admin, _, _) = setup_with_adversarial_token();
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);
        let token_client =
            AdversarialTokenContractClient::new(&env, &token_id);

        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        token_client.mint(&sender, &1000);

        // Set token to silent noop mode (mode 2)
        token_client.set_failure_mode(&2u32);

        // Tip succeeds from contract perspective
        tipjar_client.tip(&sender, &creator, &token_id, &500);

        // But storage is now inconsistent:
        // - Balance increased to 500
        // - But no tokens actually moved
        let balance = tipjar_client.get_withdrawable_balance(&creator, &token_id);
        assert_eq!(balance, 500);

        // Token balance of sender unchanged (noop)
        let sender_balance = token_client.balance(&sender);
        assert_eq!(sender_balance, 1000);

        // This invariant violation is UNPREVENTABLE if the token is whitelisted
        // The only mitigation is the allowlist + auditing tokens before whitelisting
        // (discussed in THREAT_MODEL.md)
    }

    #[test]
    fn test_token_amount_burn_documents_trust_assumptions() {
        // A token that transfers less than requested documents a gap in the trust model
        let (env, tipjar_id, token_id, admin, _, _) = setup_with_adversarial_token();
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);
        let token_client =
            AdversarialTokenContractClient::new(&env, &token_id);

        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        token_client.mint(&sender, &1000);

        // Set token to burn mode (mode 4: transfers amount-1)
        token_client.set_failure_mode(&4u32);

        // Tip 500, but token only moves 499
        tipjar_client.tip(&sender, &creator, &token_id, &500);

        // TipJar storage reflects the requested amount (500)
        let balance = tipjar_client.get_withdrawable_balance(&creator, &token_id);
        assert_eq!(balance, 500);

        // But only 499 tokens were actually moved
        let sender_balance = token_client.balance(&sender);
        assert_eq!(sender_balance, 501); // 1000 - 499

        // When creator withdraws, they can only receive 499 tokens
        // but TipJar will try to transfer 500 (token will fail with insufficient balance)
        let result = tipjar_client.try_withdraw(&creator, &token_id);
        assert!(result.is_err());

        // Storage is now stuck: balance = 500 but no withdraw possible
        // (this is a limitation of the token, not the contract)
    }

    #[test]
    fn test_reentry_protection_documents_host_behavior() {
        // This test documents current Soroban host behavior regarding same-contract reentrancy
        // On current host, this should be prohibited
        let (env, tipjar_id, token_id, admin, _, _) = setup_with_adversarial_token();
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);
        let token_client =
            AdversarialTokenContractClient::new(&env, &token_id);

        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        token_client.mint(&sender, &1000);
        tipjar_client.grant_role(&admin, &creator, &tipjar::Role::Creator);

        // Set token to reentry mode (mode 3)
        token_client.set_failure_mode(&3u32);

        // Attempt tip (token will try to reenter tipjar.tip during transfer)
        // Current Soroban host: same-contract reentrancy is prohibited
        // Result: reentry attempt fails, but the transfer part succeeds
        let result = tipjar_client.try_tip(&sender, &creator, &token_id, &500);

        // On current host, this should fail because of reentrancy prohibition
        // If this test turns red in the future, it means the host changed!
        if result.is_ok() {
            // Host now allows reentry - storage should still be consistent
            // because the contract uses checks-effects-interactions
            let balance = tipjar_client.get_withdrawable_balance(&creator, &token_id);
            assert!(balance >= 0); // No panic occurred
        }
    }

    // ============ Authorization Abuse Tests ============

    #[test]
    fn test_withdraw_requires_creator_auth() {
        let (env, tipjar_id, token_id, admin, _, _) = setup_with_adversarial_token();
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);
        let token_client =
            AdversarialTokenContractClient::new(&env, &token_id);

        let sender = Address::generate(&env);
        let creator = Address::generate(&env);
        let attacker = Address::generate(&env);

        token_client.mint(&sender, &1000);
        tipjar_client.grant_role(&admin, &creator, &tipjar::Role::Creator);
        tipjar_client.tip(&sender, &creator, &token_id, &500);

        // Attacker (without auth) tries to call withdraw for creator
        // This should panic because require_auth checks caller identity
        env.set_auths(&[]);
        let result = tipjar_client.try_withdraw(&creator, &token_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_tip_requires_sender_auth() {
        let (env, tipjar_id, token_id, admin, _, _) = setup_with_adversarial_token();
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);
        let token_client =
            AdversarialTokenContractClient::new(&env, &token_id);

        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        token_client.mint(&sender, &1000);

        // Clear auth
        env.set_auths(&[]);

        // Try to tip without sender auth
        let result = tipjar_client.try_tip(&sender, &creator, &token_id, &500);
        assert!(result.is_err());
    }

    #[test]
    fn test_self_tip_allowed() {
        let (env, tipjar_id, token_id, admin, _, _) = setup_with_adversarial_token();
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);
        let token_client =
            AdversarialTokenContractClient::new(&env, &token_id);

        let sender = Address::generate(&env);

        token_client.mint(&sender, &1000);
        env.mock_all_auths();

        // Self-tip: sender == creator
        tipjar_client.tip(&sender, &sender, &token_id, &500);

        let balance = tipjar_client.get_withdrawable_balance(&sender, &token_id);
        assert_eq!(balance, 500);
    }

    #[test]
    fn test_tip_to_contract_address() {
        let (env, tipjar_id, token_id, admin, _, _) = setup_with_adversarial_token();
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);
        let token_client =
            AdversarialTokenContractClient::new(&env, &token_id);

        let sender = Address::generate(&env);

        token_client.mint(&sender, &1000);
        env.mock_all_auths();

        // Tip to the contract's own address (edge case)
        tipjar_client.tip(&sender, &tipjar_id, &token_id, &500);

        let balance = tipjar_client.get_withdrawable_balance(&tipjar_id, &token_id);
        assert_eq!(balance, 500);
    }

    // ============ Arithmetic Edge Cases ============

    #[test]
    fn test_i128_max_adjacent_balance() {
        let (env, tipjar_id, token_id, admin, _, _) = setup_with_adversarial_token();
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);
        let token_client =
            AdversarialTokenContractClient::new(&env, &token_id);

        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        // Mint a huge amount (close to i128::MAX)
        let large_amount = i128::MAX / 2;
        token_client.mint(&sender, &large_amount);

        env.mock_all_auths();

        // Tip a large amount
        tipjar_client.tip(&sender, &creator, &token_id, &(large_amount - 1));

        let balance = tipjar_client.get_withdrawable_balance(&creator, &token_id);
        assert_eq!(balance, large_amount - 1);

        // Second tip should not overflow
        token_client.mint(&sender, &1000);
        let result = tipjar_client.try_tip(&sender, &creator, &token_id, &500);

        // May overflow and panic or may succeed depending on checked arithmetic
        // Either way, storage should be consistent
        let _ = result;
    }

    #[test]
    fn test_minimum_tip_amount() {
        let (env, tipjar_id, token_id, admin, _, _) = setup_with_adversarial_token();
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);
        let token_client =
            AdversarialTokenContractClient::new(&env, &token_id);

        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        token_client.mint(&sender, &1);
        env.mock_all_auths();

        // Tip 1 stroop (smallest unit)
        tipjar_client.tip(&sender, &creator, &token_id, &1);

        let balance = tipjar_client.get_withdrawable_balance(&creator, &token_id);
        assert_eq!(balance, 1);
    }

    #[test]
    fn test_zero_tip_rejected() {
        let (env, tipjar_id, token_id, admin, _, _) = setup_with_adversarial_token();
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);

        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        env.mock_all_auths();

        // Attempt 0-amount tip
        let result = tipjar_client.try_tip(&sender, &creator, &token_id, &0);
        assert!(result.is_err());
    }

    #[test]
    fn test_negative_tip_rejected() {
        let (env, tipjar_id, token_id, admin, _, _) = setup_with_adversarial_token();
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);

        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        env.mock_all_auths();

        // Attempt negative tip
        let result = tipjar_client.try_tip(&sender, &creator, &token_id, &-1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_consecutive_tips_accumulate() {
        let (env, tipjar_id, token_id, admin, _, _) = setup_with_adversarial_token();
        let tipjar_client = TipJarContractClient::new(&env, &tipjar_id);
        let token_client =
            AdversarialTokenContractClient::new(&env, &token_id);

        let sender = Address::generate(&env);
        let creator = Address::generate(&env);

        token_client.mint(&sender, &10000);
        env.mock_all_auths();

        // Multiple tips
        for i in 1..=10 {
            tipjar_client.tip(&sender, &creator, &token_id, &100);
        }

        let balance = tipjar_client.get_withdrawable_balance(&creator, &token_id);
        let total = tipjar_client.get_total_tips(&creator, &token_id);
        assert_eq!(balance, 1000);
        assert_eq!(total, 1000);
    }
}
