use soroban_sdk::{xdr::ToXdr, Address, Bytes, BytesN, Env};

use crate::privacy::CommitmentOpening;

/// Computes the commitment: SHA256(creator_xdr || amount_xdr || blinding_factor).
pub fn compute_commitment(
    env: &Env,
    creator: &Address,
    amount: i128,
    blinding_factor: &BytesN<32>,
) -> BytesN<32> {
    let mut data = Bytes::new(env);
    data.append(&creator.to_xdr(env));
    data.append(&amount.to_xdr(env));
    data.append(&Bytes::from(blinding_factor));
    env.crypto().sha256(&data).into()
}

/// Returns true if the opening's preimage matches the stored commitment.
pub fn verify_opening(env: &Env, commitment: &BytesN<32>, opening: &CommitmentOpening) -> bool {
    compute_commitment(
        env,
        &opening.creator,
        opening.amount,
        &opening.blinding_factor,
    ) == *commitment
}
