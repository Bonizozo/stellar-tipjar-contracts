use soroban_sdk::{contracttype, Address, BytesN};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateTip {
    pub id: u64,
    pub creator: Address,
    pub amount_hash: BytesN<32>,
    pub is_anonymous: bool,
    pub tipper: Option<Address>,
    pub created_at: u64,
    pub revealed: bool,
}
