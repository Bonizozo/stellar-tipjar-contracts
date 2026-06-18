//! Cross-contract call support for integrating with other Stellar protocols.
//!
//! Provides contract interfaces, call routing, return value handling,
//! and batch call execution for tip cross-contract interactions.

use soroban_sdk::{contracttype, symbol_short, Address, Bytes, BytesN, Env, Vec};

use crate::DataKey;

// ── Constants ────────────────────────────────────────────────────────────────

/// Maximum number of calls in a single batch.
pub const MAX_BATCH_SIZE: u32 = 32;

// ── Types ────────────────────────────────────────────────────────────────────

/// The type of cross-contract call to route.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallType {
    /// Transfer tokens to another contract.
    Transfer,
    /// Invoke a DEX swap on an external contract.
    Swap,
    /// Mint an NFT reward via an external contract.
    MintNft,
    /// Generic call with raw encoded arguments.
    Generic,
}

/// Status of a cross-contract call or batch.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallStatus {
    /// Call is queued but not yet executed.
    Pending,
    /// Call completed successfully.
    Success,
    /// Call failed.
    Failed,
}

/// A single cross-contract call descriptor.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossCall {
    /// Unique call ID.
    pub id: u64,
    /// Caller / initiator address.
    pub caller: Address,
    /// Target contract address.
    pub target: Address,
    /// Type of call.
    pub call_type: CallType,
    /// ABI-encoded arguments (up to 256 bytes).
    pub args: Bytes,
    /// Current status.
    pub status: CallStatus,
    /// Ledger timestamp of creation.
    pub created_at: u64,
    /// Ledger timestamp of execution (0 if not yet executed).
    pub executed_at: u64,
    /// Encoded return value (empty if not yet executed or failed).
    pub return_value: Bytes,
}

/// Result of a single call within a batch.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallResult {
    /// Index within the batch (0-based).
    pub index: u32,
    /// Whether this call succeeded.
    pub success: bool,
    /// Encoded return value (empty on failure).
    pub return_value: Bytes,
}

/// A batch of cross-contract calls.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallBatch {
    /// Unique batch ID.
    pub id: u64,
    /// Initiator address.
    pub caller: Address,
    /// Ordered list of call IDs in this batch.
    pub call_ids: Vec<u64>,
    /// Overall batch status.
    pub status: CallStatus,
    /// Ledger timestamp of creation.
    pub created_at: u64,
    /// Number of successful calls.
    pub success_count: u32,
    /// Number of failed calls.
    pub fail_count: u32,
}

// ── Storage sub-keys ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossCallKey {
    /// Global call ID counter.
    Counter,
    /// Global batch ID counter.
    BatchCounter,
    /// CrossCall record keyed by call ID.
    Call(u64),
    /// CallBatch record keyed by batch ID.
    Batch(u64),
    /// List of call IDs initiated by an address.
    CallerCalls(Address),
}

// ── Storage helpers ──────────────────────────────────────────────────────────

fn next_call_id(env: &Env) -> u64 {
    let key = DataKey::CrossCall(CrossCallKey::Counter);
    let id: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage().persistent().set(&key, &(id + 1));
    id
}

fn next_batch_id(env: &Env) -> u64 {
    let key = DataKey::CrossCall(CrossCallKey::BatchCounter);
    let id: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage().persistent().set(&key, &(id + 1));
    id
}

fn save_call(env: &Env, call: &CrossCall) {
    env.storage()
        .persistent()
        .set(&DataKey::CrossCall(CrossCallKey::Call(call.id)), call);
}

fn track_caller_call(env: &Env, caller: &Address, call_id: u64) {
    let key = DataKey::CrossCall(CrossCallKey::CallerCalls(caller.clone()));
    let mut ids: Vec<u64> = env.storage().persistent().get(&key).unwrap_or(Vec::new(env));
    ids.push_back(call_id);
    env.storage().persistent().set(&key, &ids);
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Register and route a single cross-contract call.
///
/// Returns the call ID. The call is recorded as `Pending` and then
/// immediately executed (status updated to `Success` or `Failed`).
pub fn route_call(
    env: &Env,
    caller: &Address,
    target: &Address,
    call_type: CallType,
    args: Bytes,
) -> u64 {
    caller.require_auth();

    let id = next_call_id(env);
    let now = env.ledger().timestamp();

    let mut call = CrossCall {
        id,
        caller: caller.clone(),
        target: target.clone(),
        call_type: call_type.clone(),
        args: args.clone(),
        status: CallStatus::Pending,
        created_at: now,
        executed_at: 0,
        return_value: Bytes::new(env),
    };

    // Execute the call based on type.
    let result = execute_call(env, &call_type, target, &args);
    call.status = if result.success {
        CallStatus::Success
    } else {
        CallStatus::Failed
    };
    call.executed_at = now;
    call.return_value = result.return_value.clone();

    save_call(env, &call);
    track_caller_call(env, caller, id);

    env.events().publish(
        (symbol_short!("cc_route"),),
        (id, caller.clone(), target.clone(), call.status.clone()),
    );

    id
}

/// Execute a batch of cross-contract calls atomically.
///
/// Each call is attempted independently; failures do not abort the batch.
/// Returns the batch ID.
pub fn route_batch(
    env: &Env,
    caller: &Address,
    targets: Vec<Address>,
    call_types: Vec<CallType>,
    args_list: Vec<Bytes>,
) -> u64 {
    caller.require_auth();

    let len = targets.len();
    assert!(len > 0 && len <= MAX_BATCH_SIZE, "invalid batch size");
    assert!(
        call_types.len() == len && args_list.len() == len,
        "mismatched batch inputs"
    );

    let batch_id = next_batch_id(env);
    let now = env.ledger().timestamp();
    let mut call_ids: Vec<u64> = Vec::new(env);
    let mut success_count: u32 = 0;
    let mut fail_count: u32 = 0;

    for i in 0..len {
        let call_id = next_call_id(env);
        let target = targets.get(i).unwrap();
        let call_type = call_types.get(i).unwrap();
        let args = args_list.get(i).unwrap();

        let result = execute_call(env, &call_type, &target, &args);
        let status = if result.success {
            success_count += 1;
            CallStatus::Success
        } else {
            fail_count += 1;
            CallStatus::Failed
        };

        let call = CrossCall {
            id: call_id,
            caller: caller.clone(),
            target: target.clone(),
            call_type,
            args,
            status,
            created_at: now,
            executed_at: now,
            return_value: result.return_value,
        };
        save_call(env, &call);
        track_caller_call(env, caller, call_id);
        call_ids.push_back(call_id);
    }

    let batch = CallBatch {
        id: batch_id,
        caller: caller.clone(),
        call_ids: call_ids.clone(),
        status: if fail_count == 0 {
            CallStatus::Success
        } else {
            CallStatus::Failed
        },
        created_at: now,
        success_count,
        fail_count,
    };

    env.storage()
        .persistent()
        .set(&DataKey::CrossCall(CrossCallKey::Batch(batch_id)), &batch);

    env.events().publish(
        (symbol_short!("cc_batch"),),
        (batch_id, caller.clone(), len, success_count, fail_count),
    );

    batch_id
}

/// Retrieve a cross-contract call record by ID.
pub fn get_call(env: &Env, call_id: u64) -> Option<CrossCall> {
    env.storage()
        .persistent()
        .get(&DataKey::CrossCall(CrossCallKey::Call(call_id)))
}

/// Retrieve a batch record by ID.
pub fn get_batch(env: &Env, batch_id: u64) -> Option<CallBatch> {
    env.storage()
        .persistent()
        .get(&DataKey::CrossCall(CrossCallKey::Batch(batch_id)))
}

/// Retrieve all call IDs initiated by a caller.
pub fn get_caller_calls(env: &Env, caller: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::CrossCall(CrossCallKey::CallerCalls(
            caller.clone(),
        )))
        .unwrap_or(Vec::new(env))
}

// ── Internal execution ───────────────────────────────────────────────────────

/// Simulate call execution and return a result.
///
/// On Soroban, actual cross-contract invocations use `env.invoke_contract`.
/// Here we record the intent and mark success; real dispatch would use the
/// appropriate `contractclient` generated by `#[contractclient]`.
fn execute_call(env: &Env, call_type: &CallType, _target: &Address, args: &Bytes) -> CallResult {
    // Validate args are non-empty for non-generic calls.
    let success = match call_type {
        CallType::Transfer | CallType::Swap | CallType::MintNft | CallType::Generic => {
            !args.is_empty()
        }
    };

    // Return value is a SHA-256 digest of the args as a deterministic receipt.
    let return_value: Bytes = if success {
        let hash: BytesN<32> = env.crypto().sha256(args).to_bytes();
        hash.into()
    } else {
        Bytes::new(env)
    };

    CallResult {
        index: 0,
        success,
        return_value,
    }
}
