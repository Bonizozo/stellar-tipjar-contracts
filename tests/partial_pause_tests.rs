//! Full (flag, entrypoint) test matrix for the granular circuit breaker,
//! plus the acceptance-criteria tests for independent pause scopes:
//! withdrawals stay live while tips are paused, and vice versa.

mod common;

use common::{expect_error, TestContext};
use tipjar::{Error, PAUSE_FLAG_TIPS, PAUSE_FLAG_WITHDRAWALS};

#[derive(Clone, Copy, Debug)]
enum Entrypoint {
    Tip,
    Withdraw,
    SetPayoutAddress,
    CancelPayoutAddress,
    AuthorizeOperator,
    RevokeOperator,
}

impl Entrypoint {
    const ALL: [Entrypoint; 6] = [
        Entrypoint::Tip,
        Entrypoint::Withdraw,
        Entrypoint::SetPayoutAddress,
        Entrypoint::CancelPayoutAddress,
        Entrypoint::AuthorizeOperator,
        Entrypoint::RevokeOperator,
    ];

    fn name(self) -> &'static str {
        match self {
            Entrypoint::Tip => "tip",
            Entrypoint::Withdraw => "withdraw",
            Entrypoint::SetPayoutAddress => "set_payout_address",
            Entrypoint::CancelPayoutAddress => "cancel_payout_address",
            Entrypoint::AuthorizeOperator => "authorize_operator",
            Entrypoint::RevokeOperator => "revoke_operator",
        }
    }

    /// The flag this entrypoint is gated on.
    fn gate(self) -> u32 {
        match self {
            Entrypoint::Tip => PAUSE_FLAG_TIPS,
            _ => PAUSE_FLAG_WITHDRAWALS,
        }
    }
}

/// Runs a single (entrypoint, pause_flag) case against a fresh contract and
/// returns the typed error the entrypoint raised, if any.
///
/// All preconditions (funding, an existing tip, an existing pending payout
/// change, an existing operator grant) are set up *before* the pause is
/// applied, since several of those setup calls are themselves gated by
/// `PAUSE_FLAG_WITHDRAWALS`.
fn run_case(entry: Entrypoint, pause_flag: Option<u32>) -> Option<soroban_sdk::Error> {
    let ctx = TestContext::new();
    let actor = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&actor, 1_000);

    match entry {
        Entrypoint::Tip | Entrypoint::SetPayoutAddress | Entrypoint::AuthorizeOperator => {}
        Entrypoint::Withdraw => {
            ctx.client().tip(&actor, &creator, &200);
        }
        Entrypoint::CancelPayoutAddress => {
            ctx.client().set_payout_address(&creator, &actor);
        }
        Entrypoint::RevokeOperator => {
            let expiry = ctx.ledger_sequence() + 1_000;
            ctx.client()
                .authorize_operator(&creator, &actor, &10, &expiry);
        }
    }

    if let Some(flag) = pause_flag {
        if flag == PAUSE_FLAG_TIPS {
            ctx.client().pause_tips(&ctx.admin);
        } else {
            ctx.client().pause_withdrawals(&ctx.admin);
        }
    }

    let result = match entry {
        Entrypoint::Tip => ctx.client().try_tip(&actor, &creator, &50),
        Entrypoint::Withdraw => ctx
            .client()
            .try_withdraw(&creator, &creator, &creator, &None),
        Entrypoint::SetPayoutAddress => ctx.client().try_set_payout_address(&creator, &actor),
        Entrypoint::CancelPayoutAddress => ctx.client().try_cancel_payout_address(&creator),
        Entrypoint::AuthorizeOperator => {
            let expiry = ctx.ledger_sequence() + 1_000;
            ctx.client()
                .try_authorize_operator(&creator, &actor, &10, &expiry)
        }
        Entrypoint::RevokeOperator => ctx.client().try_revoke_operator(&creator, &actor),
    };

    match result {
        Ok(_) => None,
        Err(Ok(e)) => Some(e),
        Err(Err(host_err)) => panic!("unexpected host-level error: {:?}", host_err),
    }
}

/// Exhaustive (flag, entrypoint) matrix: every entrypoint x {unpaused, tips
/// paused, withdrawals paused}. An entrypoint must be blocked by its own
/// gate flag and unaffected by the other one.
#[test]
fn flag_entrypoint_matrix() {
    let scenarios: [(&str, Option<u32>); 3] = [
        ("none", None),
        ("tips", Some(PAUSE_FLAG_TIPS)),
        ("withdrawals", Some(PAUSE_FLAG_WITHDRAWALS)),
    ];

    println!();
    println!(
        "{:<24}{:<16}{:<10}result",
        "entrypoint", "flag paused", "blocked"
    );
    println!("{}", "-".repeat(70));

    let mut failures = std::vec::Vec::new();

    for entry in Entrypoint::ALL {
        for (scenario_name, pause_flag) in scenarios {
            let result = run_case(entry, pause_flag);
            let expected_blocked = pause_flag == Some(entry.gate());
            let expected_error: Option<soroban_sdk::Error> = if expected_blocked {
                Some(
                    if entry.gate() == PAUSE_FLAG_TIPS {
                        Error::TipsPaused
                    } else {
                        Error::WithdrawalsPaused
                    }
                    .into(),
                )
            } else {
                None
            };

            let ok = result == expected_error;
            println!(
                "{:<24}{:<16}{:<10}{}",
                entry.name(),
                scenario_name,
                expected_blocked,
                if ok {
                    std::format!("{:?}", result)
                } else {
                    std::format!("FAIL (got {:?}, want {:?})", result, expected_error)
                }
            );
            if !ok {
                failures.push((entry.name(), scenario_name, result, expected_error));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "flag/entrypoint matrix mismatches: {:?}",
        failures
    );
}

// ── independent flags: acceptance criteria ──────────────────────────────

#[test]
fn withdrawals_remain_live_while_tips_paused() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, 1_000);
    ctx.client().tip(&sender, &creator, &300);

    ctx.client().pause_tips(&ctx.admin);

    // Withdrawals must be entirely unaffected.
    ctx.client().withdraw(&creator, &creator, &creator, &None);
    assert_eq!(ctx.token_client().balance(&creator), 300);

    // Tips remain blocked.
    let err = expect_error(ctx.client().try_tip(&sender, &creator, &10));
    assert_eq!(err, Error::TipsPaused.into());
}

#[test]
fn tips_remain_live_while_withdrawals_paused() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, 1_000);

    ctx.client().pause_withdrawals(&ctx.admin);

    // Tips must be entirely unaffected.
    ctx.client().tip(&sender, &creator, &300);
    assert_eq!(ctx.client().get_total_tips(&creator), 300);

    // Withdrawals remain blocked.
    let err = expect_error(
        ctx.client()
            .try_withdraw(&creator, &creator, &creator, &None),
    );
    assert_eq!(err, Error::WithdrawalsPaused.into());
}

#[test]
fn pause_all_blocks_both_scopes() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, 1_000);
    ctx.client().tip(&sender, &creator, &300);

    ctx.client().pause_all(&ctx.admin);

    assert_eq!(
        expect_error(ctx.client().try_tip(&sender, &creator, &10)),
        Error::TipsPaused.into()
    );
    assert_eq!(
        expect_error(
            ctx.client()
                .try_withdraw(&creator, &creator, &creator, &None)
        ),
        Error::WithdrawalsPaused.into()
    );
}

#[test]
fn view_functions_are_unaffected_by_pause() {
    let ctx = TestContext::new();
    let sender = ctx.create_user();
    let creator = ctx.create_creator();
    ctx.mint_tokens(&sender, 1_000);
    ctx.client().tip(&sender, &creator, &300);

    ctx.client().pause_all(&ctx.admin);

    assert_eq!(ctx.client().get_total_tips(&creator), 300);
    assert!(ctx.client().is_feature_paused(&PAUSE_FLAG_TIPS));
    assert!(ctx.client().is_feature_paused(&PAUSE_FLAG_WITHDRAWALS));
}
