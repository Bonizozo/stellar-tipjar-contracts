use soroban_sdk::{Address, Env};

use crate::interfaces::dex::DexClient;
use crate::DataKey;

/// Swaps `input_token` for the contract's configured tip token via the stored DEX.
/// Returns the output amount received.
pub fn swap_to_tip_token(
    env: &Env,
    from: &Address,
    input_token: &Address,
    tip_token: &Address,
    input_amount: i128,
    min_output: i128,
) -> i128 {
    let dex_address: Address = env
        .storage()
        .instance()
        .get(&DataKey::DexContract)
        .unwrap_or_else(|| panic!("DEX not configured"));

    // Transfer input tokens from caller into this contract first.
    let input_client = soroban_sdk::token::Client::new(env, input_token);
    input_client.transfer(from, &env.current_contract_address(), &input_amount);

    // Approve DEX to pull the input tokens from this contract.
    input_client.approve(
        &env.current_contract_address(),
        &dex_address,
        &input_amount,
        &(env.ledger().sequence() + 1),
    );

    let dex = DexClient::new(env, &dex_address);
    dex.swap(input_token, tip_token, &input_amount, &min_output)
}
