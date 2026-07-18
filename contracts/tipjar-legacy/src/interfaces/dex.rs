use soroban_sdk::{contractclient, Address, Env};

/// Minimal DEX interface for token swaps.
#[contractclient(name = "DexClient")]
pub trait DexInterface {
    /// Swaps `input_amount` of `input_token` for `output_token`.
    /// Returns the output amount received.
    /// Panics if output is below `min_output`.
    fn swap(
        env: Env,
        input_token: Address,
        output_token: Address,
        input_amount: i128,
        min_output: i128,
    ) -> i128;
}
