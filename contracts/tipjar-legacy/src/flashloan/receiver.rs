//! Flash loan receiver interface.

use soroban_sdk::{contractclient, Address, Bytes, Env};

/// Interface implemented by flash-loan receiver contracts.
#[contractclient(name = "FlashLoanReceiverClient")]
pub trait FlashLoanReceiver {
    /// Callback invoked after funds are transferred to receiver.
    fn execute_operation(
        env: Env,
        token: Address,
        amount: i128,
        fee: i128,
        params: Bytes,
    );
}
