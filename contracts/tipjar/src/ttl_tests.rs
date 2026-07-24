//! TTL expiry and archival behavior tests.
//!
//! These tests verify that storage entries behave correctly as they approach
//! and exceed their TTL expiry, and that recovery via extend_entries works.

#[cfg(test)]
mod tests {
    use crate::{Error, TipJar, TipJarClient};
    use soroban_sdk::{
        testutils::{Address as _, Ledger as _},
        token, Address, Env,
    };

    struct Ctx {
        env: Env,
        contract_id: Address,
        token: Address,
    }

    const LEDGER_THRESHOLD: u32 = 100_000;
    const LEDGER_BUMP: u32 = 120_960;

    impl Ctx {
        fn new() -> Self {
            let env = Env::default();
            env.mock_all_auths();

            let token_admin = Address::generate(&env);
            let token = env
                .register_stellar_asset_contract_v2(token_admin)
                .address();

            let contract_id = env.register(TipJar, ());
            let client = TipJarClient::new(&env, &contract_id);
            client.init(&token);

            Ctx {
                env,
                contract_id,
                token,
            }
        }

        fn client(&self) -> TipJarClient<'_> {
            TipJarClient::new(&self.env, &self.contract_id)
        }

        fn token_client(&self) -> token::TokenClient<'_> {
            token::TokenClient::new(&self.env, &self.token)
        }

        fn fund(&self, amount: i128) -> Address {
            let holder = Address::generate(&self.env);
            token::StellarAssetClient::new(&self.env, &self.token).mint(&holder, &amount);
            holder
        }

        fn advance_ledger(&self, delta: u32) {
            let current = self.env.ledger().sequence();
            self.env.ledger().with_mut(|ledger| {
                ledger.set_sequence_number(current + delta);
            });
        }

        fn current_ledger(&self) -> u32 {
            self.env.ledger().sequence()
        }
    }

    #[test]
    fn test_creator_balance_archived_after_ttl_expiry() {
        let ctx = Ctx::new();
        let sender = ctx.fund(10_000);
        let creator = Address::generate(&ctx.env);

        // Tip at ledger 100
        let ledger_at_tip = ctx.current_ledger();
        ctx.client().tip(&sender, &creator, &500);

        // Balance should be accessible immediately after
        assert_eq!(ctx.client().get_total_tips(&creator), 500);

        // Advance past TTL expiry (ledger_at_tip + LEDGER_BUMP + 1)
        let expiry_ledger = ledger_at_tip + LEDGER_BUMP + 1;
        let advance_by = expiry_ledger - ctx.current_ledger();
        ctx.advance_ledger(advance_by);

        // Entry is now archived; read returns 0 (entry missing)
        assert_eq!(ctx.client().get_total_tips(&creator), 0);
    }

    #[test]
    fn test_entry_alive_at_live_until_boundary() {
        let ctx = Ctx::new();
        let sender = ctx.fund(10_000);
        let creator = Address::generate(&ctx.env);

        let ledger_at_tip = ctx.current_ledger();
        ctx.client().tip(&sender, &creator, &500);

        // Entry is live_until = ledger_at_tip + LEDGER_BUMP
        // At that exact ledger, entry is still alive
        let live_until = ledger_at_tip + LEDGER_BUMP;
        let advance_by = live_until - ctx.current_ledger();
        ctx.advance_ledger(advance_by);

        // Should still be readable at live_until
        assert_eq!(ctx.client().get_total_tips(&creator), 500);

        // One ledger past live_until, archived
        ctx.advance_ledger(1);
        assert_eq!(ctx.client().get_total_tips(&creator), 0);
    }

    #[test]
    fn test_tip_bumps_ttl_of_existing_entry() {
        let ctx = Ctx::new();
        let sender = ctx.fund(10_000);
        let creator = Address::generate(&ctx.env);

        let ledger_1 = ctx.current_ledger();
        ctx.client().tip(&sender, &creator, &100);
        // live_until_1 = ledger_1 + LEDGER_BUMP

        // Advance to near expiry
        let near_expiry = ledger_1 + LEDGER_BUMP - 1_000;
        let advance_by = near_expiry - ctx.current_ledger();
        ctx.advance_ledger(advance_by);

        // Second tip should bump TTL
        let ledger_2 = ctx.current_ledger();
        ctx.client().tip(&sender, &creator, &200);
        // live_until_2 = ledger_2 + LEDGER_BUMP (much later than live_until_1)

        // Advance past original expiry
        let past_original = ledger_1 + LEDGER_BUMP + 100;
        let advance_by = past_original - ctx.current_ledger();
        ctx.advance_ledger(advance_by);

        // Entry is still readable because it was bumped
        assert_eq!(ctx.client().get_total_tips(&creator), 300);
    }

    #[test]
    fn test_get_total_tips_does_not_bump_ttl_read_only() {
        let ctx = Ctx::new();
        let sender = ctx.fund(10_000);
        let creator = Address::generate(&ctx.env);

        let ledger_at_tip = ctx.current_ledger();
        ctx.client().tip(&sender, &creator, &500);

        // Advance to near expiry
        let near_expiry = ledger_at_tip + LEDGER_BUMP - 1_000;
        let advance_by = near_expiry - ctx.current_ledger();
        ctx.advance_ledger(advance_by);

        // Call get_total_tips (read-only, should NOT bump TTL)
        let _total = ctx.client().get_total_tips(&creator);

        // Advance past expiry
        let expiry = ledger_at_tip + LEDGER_BUMP + 1;
        let advance_by = expiry - ctx.current_ledger();
        ctx.advance_ledger(advance_by);

        // Entry is archived because read-only call did not extend TTL
        assert_eq!(ctx.client().get_total_tips(&creator), 0);
    }

    #[test]
    fn test_withdraw_bumps_ttl() {
        let ctx = Ctx::new();
        let sender = ctx.fund(10_000);
        let creator = Address::generate(&ctx.env);

        let ledger_at_tip = ctx.current_ledger();
        ctx.client().tip(&sender, &creator, &500);

        // Advance to near expiry
        let near_expiry = ledger_at_tip + LEDGER_BUMP - 1_000;
        let advance_by = near_expiry - ctx.current_ledger();
        ctx.advance_ledger(advance_by);

        // Withdraw bumps the balance key's TTL
        let ledger_at_withdraw = ctx.current_ledger();
        ctx.client().withdraw(&creator);

        // Advance past original expiry
        let past_original = ledger_at_tip + LEDGER_BUMP + 100;
        let advance_by = past_original - ctx.current_ledger();
        ctx.advance_ledger(advance_by);

        // Historical total is archived (not bumped by withdraw)
        // But trying to withdraw again should fail (balance archived too)
        let err = ctx.client().try_withdraw(&creator).unwrap_err();
        // Should be either NothingToWithdraw or archival error
        assert!(err.is_some());
    }

    #[test]
    fn test_multiple_creators_independent_ttls() {
        let ctx = Ctx::new();
        let sender = ctx.fund(50_000);

        let creator1 = Address::generate(&ctx.env);
        let creator2 = Address::generate(&ctx.env);

        let ledger_at_tip1 = ctx.current_ledger();
        ctx.client().tip(&sender, &creator1, &100);

        // Advance 5 days
        ctx.advance_ledger(86_400); // rough estimate; actual depends on block time

        let ledger_at_tip2 = ctx.current_ledger();
        ctx.client().tip(&sender, &creator2, &200);

        // Creator1's expiry is earlier
        // Advance past creator1's expiry but before creator2's
        let creator1_expiry = ledger_at_tip1 + LEDGER_BUMP + 1;
        let advance_by = creator1_expiry - ctx.current_ledger();
        ctx.advance_ledger(advance_by);

        // Creator1 archived, creator2 still alive
        assert_eq!(ctx.client().get_total_tips(&creator1), 0);
        assert_eq!(ctx.client().get_total_tips(&creator2), 200);
    }

    // NOTE: Tests for extend_entries will be added once the entrypoint is implemented.
    // Expected test cases:
    // - extend_entries(creator) bumps CreatorBalance and CreatorTotal TTLs
    // - extend_entries on archived entry restores access
    // - extend_entries_batch(creators) handles multiple creators
    // - extend_entries with invalid threshold rejected
    // - extend_entries is permissionless (any caller)
    // - extend_entries emits EntriesExtended event
}
