use soroban_sdk::{symbol_short, Address, BytesN, Env, Vec};

use crate::plasma::{ExitStatus, PlasmaBlockStatus, PlasmaExit, PlasmaKey, EXIT_CHALLENGE_PERIOD};
use crate::DataKey;

/// Initiates a Plasma exit, allowing a user to withdraw funds from the Plasma chain.
///
/// The caller must provide a Merkle proof that their tip transaction is included
/// in a finalized Plasma block. The exit enters `Pending` status and can be
/// challenged during the challenge window.
///
/// Returns the new exit ID.
pub fn initiate_exit(
    env: &Env,
    exitor: &Address,
    block_number: u64,
    token: Address,
    amount: i128,
    tx_hash: BytesN<32>,
    proof: Vec<BytesN<32>>,
) -> u64 {
    exitor.require_auth();

    if amount <= 0 {
        panic!("invalid exit amount");
    }

    // Verify the referenced block is finalized
    let block: crate::plasma::PlasmaBlock = env
        .storage()
        .persistent()
        .get(&DataKey::PlasmaBlock(block_number))
        .expect("block not found");

    if !matches!(block.status, PlasmaBlockStatus::Finalized) {
        panic!("block not finalized");
    }

    // Verify the Merkle proof: the tx_hash must be included in block.tx_root
    if !verify_merkle_proof(env, &block.tx_root, &tx_hash, &proof) {
        panic!("invalid Merkle proof");
    }

    let exit_id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::PlasmaExitCounter)
        .unwrap_or(0)
        + 1;

    let exit = PlasmaExit {
        exit_id,
        block_number,
        exitor: exitor.clone(),
        token: token.clone(),
        amount,
        tx_hash,
        proof,
        initiated_at: env.ledger().timestamp(),
        status: ExitStatus::Pending,
    };

    env.storage()
        .persistent()
        .set(&DataKey::PlasmaExit(exit_id), &exit);
    env.storage()
        .instance()
        .set(&DataKey::PlasmaExitCounter, &exit_id);

    // Track user's exits
    let mut user_exits: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::PlasmaUserExits(exitor.clone()))
        .unwrap_or_else(|| Vec::new(env));
    user_exits.push_back(exit_id);
    env.storage()
        .persistent()
        .set(&DataKey::PlasmaUserExits(exitor.clone()), &user_exits);

    env.events().publish(
        (symbol_short!("pl_exit"), exit_id),
        (exitor.clone(), token, amount, block_number),
    );

    exit_id
}

/// Processes a pending exit after the challenge window has elapsed.
///
/// Credits the exitor's on-chain balance. Anyone may call this.
pub fn process_exit(env: &Env, exit_id: u64) {
    let mut exit: PlasmaExit = env
        .storage()
        .persistent()
        .get(&DataKey::PlasmaExit(exit_id))
        .expect("exit not found");

    if !matches!(exit.status, ExitStatus::Pending) {
        panic!("exit not pending");
    }

    let now = env.ledger().timestamp();
    if now < exit.initiated_at + EXIT_CHALLENGE_PERIOD {
        panic!("challenge period not elapsed");
    }

    // Credit the exitor's balance
    let bal_key = DataKey::CreatorBalance(exit.exitor.clone(), exit.token.clone());
    let balance: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&bal_key, &(balance + exit.amount));

    // Update finalized volume tracking
    let vol_key = DataKey::PlasmaFinalizedVolume(exit.exitor.clone(), exit.token.clone());
    let vol: i128 = env.storage().persistent().get(&vol_key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&vol_key, &(vol + exit.amount));

    // Update global exit counter
    let total_exits: u64 = env
        .storage()
        .instance()
        .get(&DataKey::PlasmaTotalExits)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::PlasmaTotalExits, &(total_exits + 1));

    exit.status = ExitStatus::Processed;
    env.storage()
        .persistent()
        .set(&DataKey::PlasmaExit(exit_id), &exit);

    env.events().publish(
        (symbol_short!("pl_proc"), exit_id),
        (exit.exitor.clone(), exit.token.clone(), exit.amount),
    );
}

/// Returns a Plasma exit by ID.
pub fn get_exit(env: &Env, exit_id: u64) -> Option<PlasmaExit> {
    env.storage()
        .persistent()
        .get(&DataKey::PlasmaExit(exit_id))
}

/// Returns all exit IDs for a given user.
pub fn get_user_exits(env: &Env, user: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::PlasmaUserExits(user.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Verifies a Merkle inclusion proof.
///
/// Checks that `leaf` is included in the tree with root `root` using the
/// provided `proof` (sibling hashes from leaf to root). Uses SHA-256 via
/// the Soroban crypto API.
fn verify_merkle_proof(
    env: &Env,
    root: &BytesN<32>,
    leaf: &BytesN<32>,
    proof: &Vec<BytesN<32>>,
) -> bool {
    let mut current = leaf.clone();

    for sibling in proof.iter() {
        // Combine current and sibling: hash(current || sibling)
        let mut combined = soroban_sdk::Bytes::new(env);
        combined.append(&current.into());
        combined.append(&sibling.into());
        current = env.crypto().sha256(&combined);
    }

    current == *root
}
