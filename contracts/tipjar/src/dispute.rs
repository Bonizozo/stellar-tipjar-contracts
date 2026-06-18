use soroban_sdk::{contracttype, Address, String};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeStatus {
    Open,
    UnderReview,
    Resolved,
    Rejected,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dispute {
    pub id: u64,
    pub tip_id: u64,
    pub initiator: Address,
    pub reason: String,
    pub status: DisputeStatus,
    pub arbitrator: Option<Address>,
    pub resolution: Option<String>,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeEvidence {
    pub dispute_id: u64,
    pub submitter: Address,
    pub evidence: String,
    pub submitted_at: u64,
}
