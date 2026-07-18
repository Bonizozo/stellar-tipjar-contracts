//! Option Exercise Module
//!
//! Handles the exercise and settlement of option contracts.

use soroban_sdk::{token, Address, Env};

use super::pricing::calculate_payoff;
use super::{
    get_locked_collateral, get_option_or_panic, get_position, remove_active_option,
    remove_held_option, update_locked_collateral, update_option, update_position, OptionContract,
    OptionStatus, OptionType,
};

/// Exercise an option contract
///
/// # Arguments
/// * `env` - Soroban environment
/// * `holder` - Address exercising the option
/// * `option_id` - ID of the option to exercise
/// * `spot_price` - Current market price for settlement
///
/// # Returns
/// Payoff amount transferred to holder
pub fn exercise_option(env: &Env, holder: &Address, option_id: u64, spot_price: i128) -> i128 {
    // Get and validate option
    let mut option = get_option_or_panic(env, option_id);

    // Verify holder
    if let Some(ref opt_holder) = option.holder {
        if opt_holder != holder {
            panic!("Not the option holder");
        }
    } else {
        panic!("Option has no holder");
    }

    // Verify status
    if option.status != OptionStatus::Active {
        panic!("Option is not active");
    }

    // Verify not expired
    let now = env.ledger().timestamp();
    if now >= option.expiration {
        panic!("Option has expired");
    }

    // Calculate payoff
    let payoff = calculate_payoff(
        option.option_type,
        spot_price,
        option.strike_price,
        option.amount,
    );

    if payoff == 0 {
        panic!("Option is out of the money");
    }

    // Update option status
    option.status = OptionStatus::Exercised;
    update_option(env, &option);

    // Remove from active lists
    remove_active_option(env, option_id);
    remove_held_option(env, holder, option_id);

    // Update positions
    update_holder_position_on_exercise(env, holder, payoff);
    update_writer_position_on_exercise(env, &option.writer, payoff, option.collateral);

    // Settle the option
    settle_option(env, &option, payoff);

    payoff
}

/// Settle an exercised option by transferring funds
fn settle_option(env: &Env, option: &OptionContract, payoff: i128) {
    let contract_addr = env.current_contract_address();
    let token_client = token::Client::new(env, &option.token);

    match option.option_type {
        OptionType::Call => {
            // Call option: writer delivers tokens to holder
            // Holder pays strike price to writer

            // Transfer tokens from contract (writer's collateral) to holder
            if let Some(ref holder) = option.holder {
                token_client.transfer(&contract_addr, holder, &option.amount);

                // Holder pays strike price to writer
                let strike_payment = (option.strike_price * option.amount) / 1_000_000;
                token_client.transfer(holder, &option.writer, &strike_payment);
            }
        }
        OptionType::Put => {
            // Put option: holder delivers tokens to writer
            // Writer pays strike price to holder

            if let Some(ref holder) = option.holder {
                // Holder delivers tokens to writer
                token_client.transfer(holder, &option.writer, &option.amount);

                // Writer pays strike price from collateral
                let strike_payment = (option.strike_price * option.amount) / 1_000_000;
                token_client.transfer(&contract_addr, holder, &strike_payment);
            }
        }
    }

    // Release remaining collateral to writer
    release_collateral(
        env,
        &option.writer,
        &option.token,
        option.collateral,
        payoff,
    );
}

/// Release collateral back to writer after exercise
fn release_collateral(
    env: &Env,
    writer: &Address,
    token: &Address,
    collateral: i128,
    payoff: i128,
) {
    let remaining = collateral.saturating_sub(payoff);

    if remaining > 0 {
        let token_client = token::Client::new(env, token);
        token_client.transfer(&env.current_contract_address(), writer, &remaining);
    }

    // Update locked collateral tracking
    let current_locked = get_locked_collateral(env, writer, token);
    let new_locked = current_locked.saturating_sub(collateral);
    update_locked_collateral(env, writer, token, new_locked);
}

/// Update holder's position after exercise
fn update_holder_position_on_exercise(env: &Env, holder: &Address, _payoff: i128) {
    let mut position = get_position(env, holder);
    position.held_count = position.held_count.saturating_sub(1);
    update_position(env, &position);
}

/// Update writer's position after exercise
fn update_writer_position_on_exercise(
    env: &Env,
    writer: &Address,
    _payoff: i128,
    collateral: i128,
) {
    let mut position = get_position(env, writer);
    position.total_collateral = position.total_collateral.saturating_sub(collateral);
    update_position(env, &position);
}

/// Expire an option that has passed its expiration time
///
/// Returns collateral to writer and marks option as expired.
pub fn expire_option(env: &Env, option_id: u64) {
    let mut option = get_option_or_panic(env, option_id);

    // Verify status
    if option.status != OptionStatus::Active {
        panic!("Option is not active");
    }

    // Verify expiration
    let now = env.ledger().timestamp();
    if now < option.expiration {
        panic!("Option has not expired yet");
    }

    // Update status
    option.status = OptionStatus::Expired;
    update_option(env, &option);

    // Remove from active lists
    remove_active_option(env, option_id);
    if let Some(ref holder) = option.holder {
        remove_held_option(env, holder, option_id);
    }

    // Return collateral to writer
    let token_client = token::Client::new(env, &option.token);
    token_client.transfer(
        &env.current_contract_address(),
        &option.writer,
        &option.collateral,
    );

    // Update writer's position
    let mut writer_position = get_position(env, &option.writer);
    writer_position.total_collateral = writer_position
        .total_collateral
        .saturating_sub(option.collateral);
    update_position(env, &writer_position);

    // Update locked collateral
    let current_locked = get_locked_collateral(env, &option.writer, &option.token);
    let new_locked = current_locked.saturating_sub(option.collateral);
    update_locked_collateral(env, &option.writer, &option.token, new_locked);
}

/// Cancel an option before it's sold (writer only)
pub fn cancel_option(env: &Env, writer: &Address, option_id: u64) {
    let mut option = get_option_or_panic(env, option_id);

    // Verify writer
    if &option.writer != writer {
        panic!("Not the option writer");
    }

    // Verify no holder (not yet sold)
    if option.holder.is_some() {
        panic!("Option already has a holder");
    }

    // Verify status
    if option.status != OptionStatus::Active {
        panic!("Option is not active");
    }

    // Update status
    option.status = OptionStatus::Cancelled;
    update_option(env, &option);

    // Remove from active list
    remove_active_option(env, option_id);

    // Return collateral to writer
    let token_client = token::Client::new(env, &option.token);
    token_client.transfer(&env.current_contract_address(), writer, &option.collateral);

    // Update writer's position
    let mut writer_position = get_position(env, writer);
    writer_position.written_count = writer_position.written_count.saturating_sub(1);
    writer_position.total_collateral = writer_position
        .total_collateral
        .saturating_sub(option.collateral);
    update_position(env, &writer_position);

    // Update locked collateral
    let current_locked = get_locked_collateral(env, writer, &option.token);
    let new_locked = current_locked.saturating_sub(option.collateral);
    update_locked_collateral(env, writer, &option.token, new_locked);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payoff_calculation() {
        // Call option in the money
        let payoff = calculate_payoff(
            OptionType::Call,
            1_200_000,  // spot
            1_000_000,  // strike
            10_000_000, // amount
        );
        assert!(payoff > 0);

        // Put option in the money
        let payoff = calculate_payoff(OptionType::Put, 800_000, 1_000_000, 10_000_000);
        assert!(payoff > 0);
    }
}
