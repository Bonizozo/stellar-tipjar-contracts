use soroban_sdk::{symbol_short, token, Address, Env};

use crate::bridge::{validator, BridgeDataKey, BridgeTip};
use crate::{
    AuctionError, AuctionKey, BridgeKey, CircuitBreakerKey, CoreError, CreditError, DataKey,
    DelegationKey, DisputeKey, FeatureError, FeeKey, InsuranceKey, LimitKey, LockedTipKey,
    MatchingKey, MilestoneKey, MultiSigKey, OptionKey, OtherError, PrivateTipKey, RoleKey,
    SnapshotKey, StatsKey, StreamError, StreamKey, SyntheticKey, SystemError, TipJarError,
    VestingError, VestingKey,
};

/// Processes a bridged tip submitted by an authorised relayer.
///
/// The relayer must match the stored `DataKey::BridgeRelayer` address.
/// Validates the tip, transfers funds from the relayer into contract escrow,
/// deducts bridge fees, credits the creator's balance, emits bridge events, and
/// marks the source transaction as processed (replay protection).
pub fn process_bridge_tip(
    env: &Env,
    relayer: &Address,
    tip: &BridgeTip,
) -> Result<(), TipJarError> {
    // 1. Check bridge is enabled.
    let enabled: bool = env
        .storage()
        .instance()
        .get(&BridgeDataKey::BridgeEnabled)
        .unwrap_or(false);
    if !enabled {
        return Err(TipJarError::BridgeDisabled.into());
    }

    // 2. Authenticate relayer.
    relayer.require_auth();
    let stored_relayer: Address = env
        .storage()
        .instance()
        .get(&BridgeDataKey::BridgeRelayer)
        .ok_or(CoreError::Unauthorized)?;
    if *relayer != stored_relayer {
        return Err(CoreError::Unauthorized.into());
    }

    // 3. Validate amount and replay guard.
    validator::validate_bridge_tip(env, &tip.source_chain, &tip.source_tx_hash, tip.amount)
        .map_err(|_| CoreError::InvalidAmount)?;

    // 4. Validate chain is supported.
    validator::validate_chain_supported(env, &tip.source_chain)
        .map_err(|_| CoreError::InvalidAmount)?;

    // 5. Resolve bridge token.
    let token_address: Address = env
        .storage()
        .instance()
        .get(&BridgeDataKey::BridgeToken)
        .ok_or(CoreError::TokenNotWhitelisted)?;

    // 6. Calculate bridge fee.
    let (fee, net_amount) = validator::calculate_bridge_fee(env, tip.amount);

    // 7. Pull funds from relayer into contract escrow.
    token::Client::new(env, &token_address).transfer(
        relayer,
        &env.current_contract_address(),
        &tip.amount,
    );

    // 8. Credit creator balance and historical total.
    let bal_key = DataKey::CreatorBalance(tip.creator.clone(), token_address.clone());
    let balance: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&bal_key, &(balance + net_amount));

    let tot_key = DataKey::CreatorTotal(tip.creator.clone(), token_address.clone());
    let total: i128 = env.storage().persistent().get(&tot_key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&tot_key, &(total + tip.amount));

    // 9. Accumulate bridge fees if any.
    if fee > 0 {
        let fee_key = DataKey::Bridge(BridgeKey::PlatformFeeBalance(token_address.clone()));
        let current_fee: i128 = env.storage().instance().get(&fee_key).unwrap_or(0);
        env.storage().instance().set(&fee_key, &(current_fee + fee));
    }

    // 10. Mark source tx as processed (replay protection).
    validator::mark_processed(env, &tip.source_tx_hash);

    // 11. Emit bridge events.
    env.events().publish(
        (symbol_short!("bridge"), tip.creator.clone()),
        (
            tip.source_chain.clone(),
            tip.source_tx_hash.clone(),
            tip.amount,
            fee,
        ),
    );

    if fee > 0 {
        env.events().publish(
            (symbol_short!("br_fee"), tip.creator.clone()),
            (fee, token_address.clone()),
        );
    }

    Ok(())
}
