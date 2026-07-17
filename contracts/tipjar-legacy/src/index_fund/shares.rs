//! Fund share issuance and redemption — deposits and withdrawals.

use soroban_sdk::{token, Address, Env};

use super::{
    get_creator_alloc, get_share, nav_per_share, save_fund, save_share, set_creator_alloc,
    INITIAL_SHARE_PRICE, MIN_DEPOSIT,
};

/// Deposit `amount` tokens into the fund and mint shares to `depositor`.
///
/// Shares minted = amount * INITIAL_SHARE_PRICE / nav_per_share
/// Creator allocations are updated proportionally.
pub fn deposit(env: &Env, fund_id: u64, depositor: &Address, amount: i128) -> i128 {
    depositor.require_auth();

    if amount < MIN_DEPOSIT {
        panic!("Deposit amount too small");
    }

    let mut fund = super::get_fund(env, fund_id).expect("Fund not found");
    if !fund.active {
        panic!("Fund is not active");
    }

    // Transfer tokens from depositor to this contract.
    let token_client = token::Client::new(env, &fund.token);
    token_client.transfer(depositor, &env.current_contract_address(), &amount);

    // Calculate shares to mint.
    let nav = nav_per_share(&fund);
    let shares_minted = amount * INITIAL_SHARE_PRICE / nav;

    // Update creator allocations based on deposit.
    for component in fund.components.iter() {
        let creator_amount = amount * (component.weight_bps as i128) / 10_000;
        let current = get_creator_alloc(env, fund_id, &component.creator);
        set_creator_alloc(env, fund_id, &component.creator, current + creator_amount);
    }

    // Update fund state.
    fund.total_value += amount;
    fund.total_shares += shares_minted;
    save_fund(env, &fund);

    // Update depositor's share position.
    let mut share = get_share(env, fund_id, depositor);
    share.shares += shares_minted;
    share.deposited += amount;
    save_share(env, &share);

    shares_minted
}

/// Withdraw by redeeming `shares` from the fund.
///
/// Token amount returned = shares * nav_per_share / INITIAL_SHARE_PRICE
/// Creator allocations are reduced proportionally.
pub fn withdraw(env: &Env, fund_id: u64, holder: &Address, shares: i128) -> i128 {
    holder.require_auth();

    if shares <= 0 {
        panic!("Shares must be positive");
    }

    let mut fund = super::get_fund(env, fund_id).expect("Fund not found");
    if !fund.active {
        panic!("Fund is not active");
    }

    let mut share_pos = get_share(env, fund_id, holder);
    if share_pos.shares < shares {
        panic!("Insufficient shares");
    }

    // Calculate token amount to return.
    let nav = nav_per_share(&fund);
    let amount_out = shares * nav / INITIAL_SHARE_PRICE;

    if amount_out > fund.total_value {
        panic!("Insufficient fund reserves");
    }

    // Reduce creator allocations proportionally.
    for component in fund.components.iter() {
        let creator_reduction = amount_out * (component.weight_bps as i128) / 10_000;
        let current = get_creator_alloc(env, fund_id, &component.creator);
        let new_alloc = if current > creator_reduction {
            current - creator_reduction
        } else {
            0
        };
        set_creator_alloc(env, fund_id, &component.creator, new_alloc);
    }

    // Update fund state.
    fund.total_value -= amount_out;
    fund.total_shares -= shares;
    save_fund(env, &fund);

    // Update holder's share position.
    share_pos.shares -= shares;
    save_share(env, &share_pos);

    // Transfer tokens back to holder.
    let token_client = token::Client::new(env, &fund.token);
    token_client.transfer(&env.current_contract_address(), holder, &amount_out);

    amount_out
}

/// Return the current NAV per share for a fund.
pub fn get_nav(env: &Env, fund_id: u64) -> i128 {
    let fund = super::get_fund(env, fund_id).expect("Fund not found");
    nav_per_share(&fund)
}

/// Return the share balance for a holder.
pub fn get_shares(env: &Env, fund_id: u64, holder: &Address) -> i128 {
    get_share(env, fund_id, holder).shares
}
