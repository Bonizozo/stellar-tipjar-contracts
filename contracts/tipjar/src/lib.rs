#![no_std]
#![deny(unsafe_code)]
#![deny(missing_docs)]

pub mod bridge;
pub mod integrations;
pub mod interfaces;
pub mod privacy;
pub mod security;
pub mod synthetic;

/// Polynomial commitment scheme for efficient tip data verification.
pub mod poly_commit;

/// Sidechain integration for scalable tip processing.
pub mod sidechain;

/// Optimistic rollup for scalable tip processing with fraud proofs.
pub mod rollup;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, token,
    Address, BytesN, Env, Map, String, Vec,
};

use circuit_breaker::{
    CircuitBreakerError, CooldownConfig, EnhancedCircuitBreakerConfig, VolumeThresholds,
};

pub mod circuit_breaker;
pub mod storage;
pub mod upgrade;

#[cfg(test)]
extern crate std;

// Advanced Event System
pub mod events;

// Automated Market Maker
pub mod amm;

// Governance System
pub mod governance;

// Staking and Rewards
pub mod staking;

// Conditional tip execution
pub mod conditions;

// Dynamic fee adjustment
pub mod fees;

// Dispute resolution
pub mod dispute;

// Privacy features
pub mod privacy_tip;

// Options trading
pub mod options;

// Prediction markets
pub mod prediction_market;

// Tip futures contracts
pub mod futures;

// Tip Volatility Index
pub mod volatility;

// Tip Index Funds
pub mod index_fund;

// Tip Time-Lock Puzzles
pub mod time_lock_puzzle;
// Bonding curves
pub mod bonding_curve;

// Quadratic funding
pub mod quadratic_funding;

// Retroactive public goods funding
pub mod retroactive_funding;

// TWAP oracle
pub mod twap_oracle;

// Tip payment channels
pub mod payment_channel;

// Tip state channels (off-chain tips with on-chain settlement)
pub mod state_channel;

// Meta-transactions for gasless tipping
pub mod meta_tx;

// Royalty splits for collaborative content and team tips
pub mod royalty;

// Zero-knowledge proofs for private tip verification
pub mod zk_proof;

// Cross-contract call routing and batch execution
pub mod cross_contract;

// Tip sharding for parallel processing
pub mod sharding;

// Tip validity proofs for lightweight verification
pub mod validity_proof;

// Verkle tree for efficient tip state proofs
pub mod verkle_tree;

// On-chain reputation system
pub mod reputation;

// Decentralized identity (DID) for creator verification
pub mod did;

// Verifiable credentials for creator achievements
pub mod verifiable_credentials;

// Non-transferable reputation tokens
pub mod reputation_tokens;

// Composable NFTs for tip receipts
pub mod composable_nft;

// Automated strategy execution for tip investments and yield optimization
pub mod strategy;

/// A tip record that includes an optional memo and timestamp.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipWithMemo {
    pub sender: Address,
    pub amount: i128,
    pub memo: Option<String>,
    pub timestamp: u64,
}

/// Combined creator stats stored in a single persistent entry to reduce storage reads/writes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatorStats {
    pub balance: i128,
    pub total: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipWithMessage {
    pub sender: Address,
    pub creator: Address,
    pub amount: i128,
    pub message: String,
    pub metadata: Map<String, String>,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipWithExpiry {
    pub tipper: Address,
    pub creator: Address,
    pub amount: i128,
    pub created_at: u64,
    pub expires_at: u64,
    pub claimed: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Delegation {
    pub creator: Address,
    pub delegate: Address,
    pub max_amount: i128,
    pub used_amount: i128,
    pub expires_at: u64,
    pub active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VestingSchedule {
    pub id: u64,
    pub creator: Address,
    pub tipper: Address,
    pub token: Address,
    pub total_amount: i128,
    pub start_time: u64,
    pub cliff_duration: u64,
    pub vesting_duration: u64,
    pub withdrawn: i128,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Milestone {
    pub id: u64,
    pub creator: Address,
    pub goal_amount: i128,
    pub current_amount: i128,
    pub description: String,
    pub deadline: Option<u64>,
    pub completed: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchTip {
    pub creator: Address,
    pub token: Address,
    pub amount: i128,
}

/// A single tip operation used in `batch_tip_v2`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipOperation {
    pub creator: Address,
    pub token: Address,
    pub amount: i128,
}

/// A single withdrawal operation used in `batch_withdraw`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawOperation {
    pub token: Address,
    pub amount: i128,
}

/// Result for a single operation within a batch call.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchResult {
    /// Whether this individual operation succeeded.
    pub success: bool,
    /// Zero-based index of this operation in the input vector.
    pub index: u32,
}

/// Cumulative statistics for all `batch_tip_multi` calls.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchStats {
    /// Total number of `batch_tip_multi` invocations.
    pub total_batches: u64,
    /// Total number of individual tip operations that succeeded.
    pub total_tips: u64,
    /// Total token amount successfully tipped across all batches.
    pub total_amount: i128,
    /// Total number of operations that were skipped due to validation failure.
    pub total_skipped: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockedTip {
    pub sender: Address,
    pub creator: Address,
    pub token: Address,
    pub amount: i128,
    pub unlock_timestamp: u64,
}

/// Metadata stored on-chain for each tip with an optional message.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipMetadata {
    pub sender: Address,
    pub amount: i128,
    pub message: Option<String>,
    pub timestamp: u64,
}

/// Internal record of a tip for refund tracking.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipRecord {
    pub id: u64,
    pub sender: Address,
    pub creator: Address,
    pub token: Address,
    pub amount: i128,
    pub timestamp: u64,
    pub refunded: bool,
    pub refund_requested: bool,
    pub refund_approved: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimePeriod {
    AllTime,
    Monthly,
    Weekly,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaderboardEntry {
    pub address: Address,
    pub total_amount: i128,
    pub tip_count: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParticipantKind {
    Tipper,
    Creator,
}

/// Query parameters for tip history retrieval.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipHistoryQuery {
    pub creator: Option<Address>,
    pub sender: Option<Address>,
    pub min_amount: Option<i128>,
    pub max_amount: Option<i128>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub limit: u32,
    pub offset: u32,
}

/// Insurance pool configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsurancePoolConfig {
    pub min_contribution: i128,
    pub max_contribution: i128,
    pub premium_rate_bps: u32,
    pub payout_ratio_bps: u32,
    pub claim_cooldown: u64,
    pub admin_fee_bps: u32,
    pub tip_premium_bps: u32,
}

/// Current state of the insurance pool for a specific token.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsurancePool {
    pub token: Address,
    pub total_reserves: i128,
    pub total_contributions: i128,
    pub total_claims_paid: i128,
    pub active_claims: u32,
    pub total_claims: u32,
    pub last_payout_time: u64,
}

/// An insurance claim submitted by a creator for a failed transaction.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsuranceClaim {
    pub claim_id: u64,
    pub creator: Address,
    pub token: Address,
    pub amount: i128,
    pub tx_hash: BytesN<32>,
    pub status: ClaimStatus,
    pub created_at: u64,
    pub updated_at: u64,
    pub last_claim_at: u64,
}

/// Status of an insurance claim.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ClaimStatus {
    Pending,
    Approved,
    Rejected,
    Paid,
}

/// Premium information for a creator's contribution.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PremiumInfo {
    pub creator: Address,
    pub token: Address,
    pub total_contributed: i128,
    pub coverage_amount: i128,
    pub last_claim_at: u64,
    pub active_claims: u32,
}

/// Role enum for role-based access control.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Role {
    Admin,
    Moderator,
    Creator,
}

/// A sponsor-funded tip matching program.
///
/// `match_ratio` is in basis points: 100 = 1:1, 200 = 2:1.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchingProgram {
    pub id: u64,
    pub sponsor: Address,
    pub creator: Address,
    pub token: Address,
    pub match_ratio: u32,
    pub max_match_amount: i128,
    pub current_matched: i128,
    pub active: bool,
}

/// A time-boxed sponsor matching campaign with a budget and expiry.
///
/// `match_ratio` is in basis points: 100 = 1:1, 200 = 2:1.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchingCampaign {
    pub sponsor: Address,
    pub creator: Address,
    pub token: Address,
    /// Match ratio in basis points (100 = 1:1, 200 = 2:1).
    pub match_ratio: u32,
    pub total_budget: i128,
    pub remaining_budget: i128,
    pub start_time: u64,
    pub end_time: u64,
    pub active: bool,
}

/// Per-creator withdrawal rate-limit state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawalLimits {
    /// Maximum amount withdrawable within a 24-hour window (0 = unlimited).
    pub daily_limit: i128,
    /// Minimum seconds that must elapse between withdrawals (0 = no cooldown).
    pub cooldown_seconds: u64,
    /// Ledger timestamp of the last successful withdrawal.
    pub last_withdrawal: u64,
    /// Amount already withdrawn in the current 24-hour window.
    pub withdrawn_today: i128,
    /// Ledger timestamp when the current 24-hour window started.
    pub day_start: u64,
}

/// A single recipient in a split tip, with share in basis points (10 000 = 100%).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TipRecipient {
    pub creator: Address,
    /// Share in basis points; must be > 0. All shares must sum to 10 000.
    pub percentage: u32,
}

/// Subscription status.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubscriptionStatus {
    Active,
    Paused,
    Cancelled,
}

/// Subscription tier level.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubscriptionTier {
    Bronze,
    Silver,
    Gold,
}

/// Configuration for a subscription tier set by admin.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TierConfig {
    /// Price per payment interval in token units.
    pub price: i128,
    /// Human-readable description of benefits for this tier.
    pub benefits: String,
}

/// A recurring tip subscription from a subscriber to a creator.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subscription {
    pub subscriber: Address,
    pub creator: Address,
    pub token: Address,
    pub amount: i128,
    /// Minimum seconds between payments.
    pub interval_seconds: u64,
    pub last_payment: u64,
    pub next_payment: u64,
    pub status: SubscriptionStatus,
    /// The tier this subscription is on.
    pub tier: SubscriptionTier,
    /// Pending tier change to apply at next payment cycle (None = no pending change).
    pub pending_tier: Option<SubscriptionTier>,
}

/// Stream status.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamStatus {
    Active,
    Paused,
    Cancelled,
    Completed,
}

/// A continuous tip stream where funds flow in real-time based on time elapsed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stream {
    pub stream_id: u64,
    pub sender: Address,
    pub creator: Address,
    pub token: Address,
    pub amount_per_second: i128,
    pub start_time: u64,
    pub end_time: u64,
    pub withdrawn: i128,
    pub status: StreamStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

/// A time-locked tip that can only be withdrawn after `unlock_time`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimeLock {
    pub sender: Address,
    pub creator: Address,
    pub token: Address,
    pub amount: i128,
    pub unlock_time: u64,
    pub created_at: u64,
    pub expires_at: u64,
    pub cancelled: bool,
}

/// An auction for exclusive creator content or experiences.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Auction {
    pub auction_id: u64,
    pub creator: Address,
    pub token: Address,
    pub reserve_price: i128,
    pub highest_bid: i128,
    pub highest_bidder: Option<Address>,
    pub ends_at: u64,
    pub created_at: u64,
    pub settled: bool,
}

/// A pending multi-signature withdrawal request.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiSigWithdrawal {
    pub request_id: u64,
    pub creator: Address,
    pub token: Address,
    pub amount: i128,
    pub approvals: Vec<Address>,
    pub required_approvals: u32,
    pub expires_at: u64,
    pub executed: bool,
    pub cancelled: bool,
}

/// Per-contract multi-sig configuration set by admin.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiSigConfig {
    /// Withdrawal amount above which multi-sig is required (0 = always require).
    pub threshold: i128,
    /// Number of approvals needed to execute.
    pub required_approvals: u32,
    /// Seconds until a pending request expires.
    pub expiry_seconds: u64,
    /// Authorised signers.
    pub signers: Vec<Address>,
}

/// Leaderboard category selector.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LeaderboardType {
    TopTippers,
    TopCreators,
}

/// State snapshot keyed by snapshot_id.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateSnapshot {
    pub snapshot_id: u64,
    pub timestamp: u64,
    pub metadata: soroban_sdk::String,
}

/// Circuit breaker configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitBreakerConfig {
    /// Maximum amount for a single tip before triggering breaker.
    pub max_single_tip: i128,
    /// Maximum total volume in a sliding window before triggering breaker.
    pub max_volume_window: i128,
    /// Window duration in seconds.
    pub window_seconds: u64,
    /// Cooldown duration in seconds when halted.
    pub cooldown_seconds: u64,
    /// Whether the circuit breaker is active.
    pub enabled: bool,
}

/// Current state of the circuit breaker.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitBreakerState {
    /// Start time of the current volume window.
    pub window_start: u64,
    /// Total volume processed in the current window.
    pub current_volume: i128,
    /// Timestamp until which the contract is halted (0 if not halted).
    pub halted_until: u64,
    /// Number of times the breaker has been triggered.
    pub trigger_count: u32,
}

/// Credit configuration for the tipping platform.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreditConfig {
    /// Maximum credit ratio in basis points of historical total tips (e.g., 2000 = 20%).
    pub max_credit_ratio_bps: u32,
    /// Interest rate in basis points per 30-day period (e.g., 100 = 1%).
    pub interest_rate_bps: u32,
    /// Minimum historical tips required in a token to be eligible for credit.
    pub min_total_tips: i128,
    /// Share of incoming tips automatically diverted for repayment (e.g., 5000 = 50%).
    pub repayment_share_bps: u32,
    /// Whether the credit system is enabled.
    pub enabled: bool,
}

/// Represents a creator's credit account for a specific token.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreditAccount {
    /// Outstanding principal amount borrowed.
    pub principal: i128,
    /// Interest accrued but not yet paid.
    pub interest_accrued: i128,
    /// Ledger timestamp of the last interest accrual or repayment.
    pub last_update: u64,
    /// Total amount ever borrowed.
    pub total_borrowed: i128,
    /// Total amount ever repaid (principal + interest).
    pub total_repaid: i128,
}

/// A record of a credit event (borrowing or repayment).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreditRecord {
    pub id: u64,
    pub amount: i128,
    pub interest_portion: i128,
    pub timestamp: u64,
    pub is_repayment: bool,
}

/// Storage layout for persistent contract data.
#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    /// Token contract address whitelist state (bool).
    TokenWhitelist(Address),
    /// Creator's currently withdrawable balance held by this contract per token.
    CreatorBalance(Address, Address), // (creator, token)
    /// Historical total tips ever received by creator per token.
    CreatorTotal(Address, Address), // (creator, token)
    /// List of token addresses a creator has ever received tips in.
    CreatorTokens(Address),
    /// Emergency pause state (bool).
    Paused,
    /// Contract administrator (Address).
    Admin,
    /// Messages appended for a creator.
    CreatorMessages(Address),
    /// Milestone related keys.
    Milestone(MilestoneKey),
    /// Role related keys.
    Role(RoleKey),
    /// Stats related keys.
    Stats(StatsKey),
    /// Locked tip related keys.
    LockedTip(LockedTipKey),
    /// Matching related keys.
    Matching(MatchingKey),
    /// Tip related keys.
    Tip(TipKey),
    /// Off-chain oracle approval flag keyed by condition ID.
    OffchainCondition(BytesN<32>),
    /// Dynamic fee related keys.
    Fee(FeeKey),
    /// Monotonically increasing contract version, incremented on each upgrade.
    ContractVersion,
    /// Subscription keyed by (subscriber, creator).
    Subscription(Address, Address),
    /// Human-readable reason stored when the contract is paused.
    PauseReason,
    /// Optional timestamp (unix seconds) after which the contract auto-unpauses.
    PauseUntil,
    /// TipMetadata keyed by (creator, tip_index).
    TipHistory(Address, u64),
    /// Total number of tips with metadata stored for a creator.
    TipCount(Address),
    /// Platform fee in basis points (u32).
    FeeBasisPoints,
    /// Accumulated platform fee balance per token.
    PlatformFeeBalance(Address),
    /// Refund window in seconds (u64).
    RefundWindow,
    /// Leaderboard related keys.
    Leaderboard(LeaderboardType),
    /// Tipper total tips sent (i128).
    TipperTotal(Address),
    /// Snapshot related keys.
    Snapshot(SnapshotKey),
    /// Limit related keys.
    Limit(LimitKey),
    /// Next time-lock ID counter (u64).
    NextLockId,
    /// List of lock IDs belonging to a creator.
    CreatorLocks(Address),
    /// Active time-lock IDs for expiration processing.
    ActiveTimeLocks,
    /// Delegation related keys.
    Delegation(DelegationKey),
    /// Vesting related keys.
    Vesting(VestingKey),
    /// Stream related keys.
    Stream(StreamKey),
    /// Auction related keys.
    Auction(AuctionKey),
    /// Time-lock record keyed by lock ID.
    TimeLock(u64),
    /// Multi-sig withdrawal request keyed by request ID.
    MultiSigRequest(u64),
    /// Global counter for multi-sig request IDs.
    MultiSigCounter,
    /// Multi-sig configuration (threshold, signers, required approvals).
    MultiSigConfig,
    /// Dispute record keyed by dispute_id.
    Dispute(u64),
    /// Global counter for dispute IDs.
    DisputeCounter,
    /// List of dispute IDs for a creator.
    CreatorDisputes(Address),
    /// Evidence records keyed by (dispute_id, evidence_index).
    DisputeEvidence(u64, u64),
    /// Evidence counter for a dispute.
    DisputeEvidenceCounter(u64),
    /// Private tip record keyed by tip_id.
    PrivateTip(u64),
    /// Global counter for private tip IDs.
    PrivateTipCounter,
    /// Revealed amount for a private tip keyed by tip_id.
    PrivateTipAmount(u64),
    /// Insurance pool configuration.
    InsPoolCfg,
    /// Insurance pool state per token.
    InsPoolToken(Address),
    /// Insurance claim record keyed by claim ID.
    InsClaim(u64),
    /// Global counter for insurance claim IDs.
    InsClaimCtr,
    /// Creator's insurance contribution per token.
    InsContrib(Address, Address),
    /// Creator's last claim timestamp per token.
    InsLastClm(Address, Address),
    /// Creator's active claim count per token.
    InsActiveClms(Address, Address),
    /// Total number of claims for a creator per token.
    InsTotalClms(Address, Address),
    /// Insurance feature enabled flag.
    InsEnabled,
    /// Max active claims per creator.
    InsMaxClms,
    /// Insurance admin address.
    InsAdmin,
    /// List of claim IDs for a creator per token.
    InsClms(Address, Address),
    /// Option contract by ID.
    Option(u64),
    /// Option counter for ID generation.
    OptionCounter,
    /// Options written by address.
    WrittenOptions(Address),
    /// Options held by address.
    HeldOptions(Address),
    /// Option position tracking for address.
    OptionPosition(Address),
    /// Option pricing parameters.
    OptionPricingParams,
    /// Active options list.
    ActiveOptions,
    /// Collateral locked per token per address for options.
    OptionCollateral(Address, Address),
    /// Bridge relayer address.
    BridgeRelayer,
    /// Bridge token address.
    BridgeToken,
    /// Bridge enabled flag.
    BridgeEnabled,
    /// Bridge fee in basis points.
    BridgeFeeBps,
    /// Index fund record by ID.
    IndexFund(u64),
    /// Global index fund counter.
    IndexFundCounter,
    /// User share position in a fund keyed by (fund_id, holder).
    IndexFundShare(u64, Address),
    /// Creator allocation within a fund keyed by (fund_id, creator).
    IndexCreatorAlloc(u64, Address),
    /// Time-lock puzzle record by ID.
    TimeLockPuzzle(u64),
    /// Global time-lock puzzle counter.
    TimeLockPuzzleCounter,
    /// List of puzzle IDs created by an address.
    CreatorPuzzles(Address),
    /// List of puzzle IDs targeting a recipient.
    RecipientPuzzles(Address),
    /// TWAP oracle config and state keyed by oracle_id.
    TwapOracle(u64),
    /// Individual ring-buffer observation keyed by (oracle_id, slot_index).
    TwapObservation(u64, u32),
    /// Global TWAP oracle ID counter.
    TwapOracleCounter,
    /// Prediction market record by ID.
    PredMarket(u64),
    /// Global prediction market counter.
    PredMarketCounter,
    /// Bettor position keyed by (market_id, bettor).
    PredBettorPosition(u64, Address),
    /// List of market IDs a bettor has participated in.
    PredBettorMarkets(Address),
    /// List of market IDs for a creator.
    PredCreatorMarkets(Address),
    /// List of all open prediction market IDs.
    PredActiveMarkets,
    /// Platform fee for prediction markets in basis points.
    PredMarketFeeBps,
    /// Futures contract record by ID.
    FuturesContract(u64),
    /// Global futures contract counter.
    FuturesCounter,
    /// Aggregated position for a trader.
    FuturesPosition(Address),
    /// List of contract IDs for a trader.
    FuturesTraderContracts(Address),
    /// List of all active futures contract IDs.
    FuturesActiveContracts,
    /// Global futures configuration (margins, penalties).
    FuturesConfig,
    /// Volatility index state by ID.
    VolIndex(u64),
    /// Global volatility index counter.
    VolCounter,
    /// Ring-buffer observation slot keyed by (index_id, slot).
    VolObservation(u64, u32),
    /// Volatility snapshot keyed by (index_id, seq).
    VolSnapshot(u64, u64),
    /// Total snapshot count for an index.
    VolSnapshotCount(u64),
    /// List of index IDs for a creator.
    VolCreatorIndices(Address),
    /// Global volatility module configuration.
    VolConfig,
    /// AMM liquidity pool state by pool ID.
    AmmPool(u64),
    /// Global AMM pool counter.
    AmmPoolCounter,
    /// AMM pool ID lookup by token pair.
    AmmPoolByTokens(Address, Address),
    /// LP share balance keyed by (pool_id, provider).
    AmmLpShares(u64, Address),
    /// Fee-per-share debt snapshot keyed by (pool_id, provider).
    AmmProviderDebt(u64, Address),
    /// Homomorphic encryption configuration.
    HomomorphicConfig,
    /// Key management configuration for homomorphic encryption.
    KeyManagementConfig,
    /// History of public keys for homomorphic encryption.
    KeyHistory,
    /// Encrypted balance keyed by (creator, token).
    EncryptedBalance(Address, Address),
    /// Encrypted tip record keyed by tip_id.
    EncryptedTip(u64),
    /// Global counter for encrypted tip IDs.
    EncryptedTipCounter,
    /// Nullifier for encrypted tips (prevents double-spend).
    PrivacyNullifier(BytesN<32>),
    /// Payment channel record keyed by (party_a, party_b, token).
    PaymentChannel(Address, Address, Address),
    /// Global counter for payment channel IDs.
    ChannelCounter,
    /// Tip state channel record keyed by (tipper, creator, token).
    TipChannel(Address, Address, Address),
    /// Individual tip entry within a state channel keyed by (tipper, creator, token, index).
    TipChannelEntry(Address, Address, Address, u64),
    /// Per-sender meta-transaction nonce keyed by sender address.
    MetaTxNonce(Address),
    /// Trusted relayer flag keyed by relayer address.
    MetaTxRelayer(Address),
    /// Consumed request nullifier keyed by request hash (replay protection).
    MetaTxNullifier(BytesN<32>),
    /// Meta-transaction execution record keyed by record ID.
    MetaTxRecord(u64),
    /// Global meta-transaction record counter.
    MetaTxCounter,
    /// Commit-reveal round keyed by round ID.
    CommitRevealRound(u64),
    /// Commitment keyed by (round_id, participant).
    CommitRevealCommitment(u64, Address),
    /// Reveal keyed by (round_id, participant).
    CommitRevealReveal(u64, Address),
    /// List of round IDs created by a creator.
    CommitRevealCreatorRounds(Address),
    /// Global commit-reveal round counter.
    CommitRevealCounter,
    /// ZK circuit verification key keyed by circuit ID.
    ZkCircuit(u64),
    /// ZK proof keyed by proof ID.
    ZkProof(u64),
    /// Private tip with ZK proof keyed by tip ID.
    ZkPrivateTip(u64),
    /// Nullifier used flag keyed by nullifier hash.
    ZkNullifier(BytesN<32>),
    /// List of circuit IDs owned by an address.
    ZkOwnerCircuits(Address),
    /// List of proof IDs submitted by a prover.
    ZkProverProofs(Address),
    /// List of private tip IDs for a creator.
    ZkCreatorPrivateTips(Address),
    /// Global ZK circuit counter.
    ZkCircuitCounter,
    /// Global ZK proof counter.
    ZkProofCounter,
    /// Global ZK private tip counter.
    ZkPrivateTipCounter,
    /// Cross-contract call sub-keys.
    CrossCall(cross_contract::CrossCallKey),
    /// Sharding sub-keys.
    Shard(sharding::ShardKey),
    /// Validity proof sub-keys.
    ValidityProof(validity_proof::ValidityProofKey),
    /// Verkle tree sub-keys.
    Verkle(verkle_tree::VerkleKey),
    /// Retroactive public goods funding sub-keys.
    Retro(retroactive_funding::RetroKey),
    /// Decentralized identity sub-keys.
    Did(did::DidKey),
    /// Verifiable credential sub-keys.
    Vc(verifiable_credentials::VcKey),
    /// Reputation token sub-keys.
    RepToken(reputation_tokens::RepTokenKey),
    /// Composable NFT sub-keys.
    Nft(composable_nft::NftKey),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TipJarError {
    AlreadyInitialized = 1,
    TokenNotWhitelisted = 2,
    InvalidAmount = 3,
    NothingToWithdraw = 4,
    MessageTooLong = 5,
    MilestoneNotFound = 6,
    MilestoneAlreadyCompleted = 7,
    InvalidGoalAmount = 8,
    Unauthorized = 9,
    RoleNotFound = 10,
    InsufficientBalance = 11,
    AmountTooSmall = 12,
    /// Farming pool not found.
    FarmingPoolNotFound = 13,
    /// Farming position not found.
    FarmingPositionNotFound = 14,
    /// Farming lock period has not expired yet.
    FarmingLockNotExpired = 15,
    /// Liquidity mining program not found.
    LmProgramNotFound = 16,
    /// Liquidity mining position not found.
    LmPositionNotFound = 17,
    /// Invalid reward rate for LM program.
    LmInvalidRate = 18,
    /// Invalid vesting parameters for LM program.
    LmInvalidVesting = 19,
    /// Invalid end time for LM program.
    LmInvalidEndTime = 20,
    /// LM program is inactive.
    LmProgramInactive = 21,
    /// LM program has ended.
    LmProgramEnded = 22,
    /// No rewards available to claim.
    LmNothingToClaim = 23,
    /// All rewards have been distributed.
    LmRewardsExhausted = 24,
    /// Boost multiplier is not higher than current.
    LmBoostTooLow = 25,
    /// Invalid duration parameter.
    InvalidDuration = 26,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CoreError {
    AlreadyInitialized = 1,
    TokenNotWhitelisted = 2,
    InvalidAmount = 3,
    NothingToWithdraw = 4,
    MessageTooLong = 5,
    MilestoneNotFound = 6,
    MilestoneAlreadyCompleted = 7,
    InvalidGoalAmount = 8,
    Unauthorized = 9,
    RoleNotFound = 10,
    BatchTooLarge = 11,
    InsufficientBalance = 12,
    InvalidUnlockTime = 13,
    TipStillLocked = 14,
    LockedTipNotFound = 15,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SystemError {
    MatchProgNotFound = 16,
    MatchProgInactive = 17,
    InvalidMatchRatio = 18,
    DexNotConfigured = 19,
    NftNotConfigured = 20,
    SwapFailed = 21,
    ConditionFailed = 22,
    UpgradeUnauthorized = 23,
    SubscriptionNotFound = 24,
    SubscriptionNotActive = 25,
    PaymentNotDue = 26,
    InvalidInterval = 27,
    InvalidRecipientCount = 28,
    InvalidPercentageSum = 29,
    InvalidPercentage = 30,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FeatureError {
    ContractPaused = 31,
    FeeExceedsMaximum = 32,
    LockNotFound = 33,
    NotUnlocked = 34,
    LockCancelled = 35,
    WithdrawalCooldown = 36,
    DailyLimitExceeded = 37,
    MsigReqNotFound = 38,
    MultiSigReqExpired = 39,
    MultiSigReqClosed = 40,
    NotASigner = 41,
    AlreadyApproved = 42,
    MultiSigNotConfigured = 43,
    DelegationNotFound = 44,
    DelegationExpired = 45,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VestingError {
    DelegationInactive = 46,
    DelegationLimitExceeded = 47,
    InvalidDuration = 48,
    DisputeNotFound = 49,
    DisputeNotOpen = 50,
    DisputeUnauthorized = 51,
    InsPoolNotCfg = 52,
    ContributionTooLow = 53,
    ContributionTooHigh = 54,
    NoCoverage = 55,
    ClaimNotApproved = 56,
    ClaimAlreadyPaid = 57,
    InsufficientReserves = 58,
    ClaimCooldownActive = 59,
    TooManyActiveClaims = 60,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StreamError {
    ClaimNotFound = 61,
    AlreadyContributed = 62,
    InsuranceDisabled = 63,
    PendingClaimExists = 64,
    PayoutExceedsReserves = 65,
    InvalidClaimAmount = 66,
    AdmAppReq = 67,
    PrivateTipNotFound = 68,
    InvalidReveal = 69,
    StreamNotFound = 70,
    StreamAlreadyCancelled = 71,
    StreamNotStarted = 72,
    StreamAlreadyCompleted = 73,
    InvalidStreamAmount = 74,
    InvalidStreamRate = 75,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AuctionError {
    NoStreamedAmount = 76,
    StrmRateMax = 77,
    AuctionNotFound = 78,
    AuctionAlreadySettled = 79,
    AuctionNotEnded = 80,
    AuctionReserveNotMet = 81,
    AuctionBidTooLow = 82,
    AuctionUnauthorized = 83,
    InvalidAuctionDuration = 84,
    InvalidReservePrice = 85,
    AuctionEnded = 86,
    OptionNotFound = 87,
    OptionNotActive = 88,
    OptionExpired = 89,
    OptionOutOfMoney = 90,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CreditError {
    NotOptionHolder = 91,
    NotOptionWriter = 92,
    OptionAlreadySold = 93,
    InsufficientCollateral = 94,
    InvalidOptionParams = 95,
    OptionNotExpired = 96,
    InvalidBridgeFee = 97,
    /// Index fund not found.
    IndexFundNotFound = 98,
    /// Index fund is not active.
    IndexFundNotActive = 99,
    /// Component weights do not sum to 10000 bps.
    InvalidIndexWeights = 100,
    /// Index fund requires at least 2 creators.
    IndexFundTooFewCreators = 101,
    /// Insufficient fund shares for withdrawal.
    InsufficientFundShares = 102,
    /// Deposit amount is below the minimum.
    IndexDepositTooSmall = 103,
    /// Attempted to unpause a contract that is not currently paused.
    NotPaused = 104,
    /// Time-lock puzzle not found.
    PuzzleNotFound = 105,
    /// Puzzle is not in Active status.
    PuzzleNotActive = 106,
    /// Puzzle unlock time has not been reached yet.
    PuzzleTooEarly = 107,
    /// Supplied secret/nonce does not match the puzzle commitment.
    PuzzleWrongSolution = 108,
    /// Puzzle has already been solved.
    PuzzleAlreadySolved = 109,
    /// Puzzle has been cancelled.
    PuzzleCancelled = 110,
    /// Caller is not the puzzle creator.
    PuzzleUnauthorized = 111,
    /// TWAP oracle not found.
    TwapOracleNotFound = 105,
    /// TWAP oracle is inactive.
    TwapOracleInactive = 106,
    /// TWAP price update is too frequent (below MIN_UPDATE_INTERVAL).
    TwapUpdateTooFrequent = 107,
    /// TWAP observation window is outside the allowed range.
    TwapInvalidWindow = 108,
    /// TWAP oracle parameters (e.g. capacity) are invalid.
    TwapInvalidParams = 109,
    /// TWAP price value is invalid (must be > 0).
    TwapInvalidPrice = 110,
    /// Prediction market not found.
    PredMarketNotFound = 111,
    /// Prediction market is not open for betting.
    PredMarketNotOpen = 112,
    /// Betting window for this market has closed.
    PredMarketClosed = 113,
    /// Prediction market has already been resolved or cancelled.
    PredMarketAlreadySettled = 114,
    /// Caller is not the designated market resolver.
    PredMarketNotResolver = 115,
    /// Bet amount is below the minimum allowed.
    PredBetTooSmall = 116,
    /// Bettor has no position in this market.
    PredNoPosition = 117,
    /// Winnings for this market have already been claimed.
    PredAlreadyClaimed = 118,
    /// Market is not yet resolved or cancelled; cannot claim.
    PredMarketNotSettled = 119,
    /// Futures contract not found.
    FuturesNotFound = 120,
    /// Futures contract is not in an active state.
    FuturesNotActive = 121,
    /// Futures contract has already been matched by a short party.
    FuturesAlreadyMatched = 122,
    /// Caller is not a party to this futures contract.
    FuturesUnauthorized = 123,
    /// Settlement date has not been reached yet.
    FuturesNotDue = 124,
    /// Contract size is below the minimum allowed.
    FuturesSizeTooSmall = 125,
    /// Contract price must be greater than zero.
    FuturesInvalidPrice = 126,
    /// Position is not under-margined; liquidation not allowed.
    FuturesPositionHealthy = 127,
    /// Contract is not matched; cannot settle.
    FuturesNotMatched = 128,
    /// Futures contract has already been settled or liquidated.
    FuturesAlreadyClosed = 129,
    /// Volatility index not found.
    VolIndexNotFound = 130,
    /// Volatility index is not active.
    VolIndexNotActive = 131,
    /// Observation is too frequent (below min interval).
    VolObsTooFrequent = 132,
    /// Window size is outside the allowed range.
    VolInvalidWindow = 133,
    /// Caller is not the index creator.
    VolUnauthorized = 134,
    /// AMM pool not found.
    AmmPoolNotFound = 135,
    /// AMM pool already exists for this token pair.
    AmmPoolExists = 136,
    /// Both tokens in a pool must be different.
    AmmIdenticalTokens = 137,
    /// Token is not part of this pool.
    AmmTokenNotInPool = 138,
    /// Swap or withdrawal would exceed slippage tolerance.
    AmmSlippageExceeded = 139,
    /// Pool fee exceeds the maximum allowed (10 %).
    AmmFeeTooHigh = 140,
    /// Insufficient liquidity in the pool for this operation.
    AmmInsufficientLiquidity = 141,
    /// Provider has insufficient LP shares.
    AmmInsufficientShares = 142,
    /// Sidechain feature is not initialized.
    SidechainNotInitialized = 143,
    /// Sidechain feature is disabled.
    SidechainDisabled = 144,
    /// Caller is not the sidechain operator.
    SidechainUnauthorized = 145,
    /// Checkpoint with this sequence number was not found.
    SidechainCheckpointNotFound = 146,
    /// Checkpoint has already been finalized.
    SidechainAlreadyFinalized = 147,
    /// Tip batch was not found.
    SidechainBatchNotFound = 148,
    /// Tip batch has already been settled.
    SidechainBatchAlreadySettled = 149,
    /// Checkpoint must be finalized before settling batches.
    SidechainCheckpointNotFinalized = 150,
    /// Rollup is not initialized.
    RollupNotInitialized = 151,
    /// Caller is not the rollup sequencer.
    RollupUnauthorized = 152,
    /// Rollup batch not found.
    RollupBatchNotFound = 153,
    /// Batch is not in pending status.
    RollupBatchNotPending = 154,
    /// Challenge period has not elapsed yet.
    RollupChallengeActive = 155,
    /// Challenge period has already elapsed; fraud proof too late.
    RollupChallengeExpired = 156,
    /// Fraud proof roots match; no fraud detected.
    RollupNoFraudDetected = 157,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum OtherError {
    CreditDisabled = 107,
    IneligibleForCredit = 108,
    CreditLimitExceeded = 109,
    ActiveLoanExists = 110,
    InsufficientLendingLiquidity = 111,
    NoActiveCredit = 112,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ChannelError {
    /// No channel exists for the given parties and token.
    ChannelNotFound = 200,
    /// Channel is not in the Open state.
    ChannelNotOpen = 201,
    /// Provided nonce is not strictly greater than the current nonce.
    StaleNonce = 202,
    /// Caller is not a party to this channel.
    NotChannelParty = 203,
    /// Channel is not in the Disputed state.
    ChannelNotDisputed = 204,
    /// Dispute window has not yet elapsed.
    DisputeWindowActive = 205,
    /// Proposed balance exceeds total channel deposit.
    InvalidChannelBalance = 206,
    /// Deposit amount must be greater than zero.
    InvalidDeposit = 207,
    /// Dispute window must be greater than zero.
    InvalidDisputeWindow = 208,
}

/// Errors specific to tip state channels.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TipChannelError {
    /// No tip channel exists for the given parties and token.
    ChannelNotFound = 300,
    /// Channel is not in the Open state.
    ChannelNotOpen = 301,
    /// Provided nonce is not strictly greater than the current nonce.
    StaleNonce = 302,
    /// Caller is not a party to this channel.
    NotChannelParty = 303,
    /// Deposit amount must be greater than zero.
    InvalidDeposit = 304,
    /// Dispute window must be greater than zero.
    InvalidDisputeWindow = 305,
    /// Tipped amount exceeds the channel deposit.
    ExceedsDeposit = 306,
    /// Tipped amount cannot decrease.
    TippedAmountDecreased = 307,
    /// Channel is already settled.
    AlreadySettled = 308,
}

/// Errors for meta-transaction (gasless tip) operations.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MetaTxError {
    /// Relayer is not in the trusted relayer set.
    UntrustedRelayer = 400,
    /// Signed request has passed its `valid_until` timestamp.
    RequestExpired = 401,
    /// Provided nonce does not match the stored per-sender nonce.
    InvalidNonce = 402,
    /// This request hash has already been executed (replay attempt).
    AlreadyExecuted = 403,
    /// Ed25519 signature verification failed.
    InvalidSignature = 404,
    /// Tip amount in the request is invalid (≤ 0).
    InvalidAmount = 405,
    /// Token in the request is not whitelisted.
    TokenNotWhitelisted = 406,
    /// Relayer is already registered.
    RelayerAlreadyRegistered = 407,
    /// Relayer is not registered (cannot remove).
    RelayerNotFound = 408,
}

/// Errors for commit-reveal operations.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CommitRevealError {
    /// Round not found.
    RoundNotFound = 500,
    /// Round is not in the commit phase.
    NotInCommitPhase = 501,
    /// Round is not in the reveal phase.
    NotInRevealPhase = 502,
    /// Commit phase has not started yet.
    CommitPhaseNotStarted = 503,
    /// Commit phase has ended.
    CommitPhaseEnded = 504,
    /// Reveal phase has ended.
    RevealPhaseEnded = 505,
    /// Participant has already committed.
    AlreadyCommitted = 506,
    /// Participant has already revealed.
    AlreadyRevealed = 507,
    /// No commitment found for this participant.
    NoCommitment = 508,
    /// Revealed value does not match commitment hash.
    InvalidReveal = 509,
    /// Round is already finalized or cancelled.
    RoundNotActive = 510,
    /// Commit duration is invalid (too short or too long).
    InvalidCommitDuration = 511,
    /// Reveal duration is invalid (too short or too long).
    InvalidRevealDuration = 512,
    /// Reveal phase has not ended yet.
    RevealPhaseNotEnded = 513,
    /// Commit phase has not ended yet.
    CommitPhaseNotEnded = 514,
}

/// Errors for zero-knowledge proof operations.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ZkProofError {
    /// Circuit not found.
    CircuitNotFound = 600,
    /// Circuit is not active.
    CircuitNotActive = 601,
    /// Proof not found.
    ProofNotFound = 602,
    /// Proof data exceeds maximum size.
    ProofTooLarge = 603,
    /// Too many public inputs.
    TooManyPublicInputs = 604,
    /// Nullifier has already been used.
    NullifierUsed = 605,
    /// Proof is not in pending status.
    ProofNotPending = 606,
    /// Proof has not been verified.
    ProofNotVerified = 607,
    /// Nullifier mismatch.
    NullifierMismatch = 608,
    /// Private tip not found.
    PrivateTipNotFound = 609,
    /// Private tip already claimed.
    AlreadyClaimed = 610,
    /// Caller is not the circuit owner.
    NotCircuitOwner = 611,
    /// Caller is not the tip creator.
    NotTipCreator = 612,
}

#[contract]
pub struct TipJarContract;

#[contractimpl]
impl TipJarContract {
    // ── pause guard ──────────────────────────────────────────────────────────

    fn require_not_paused(env: &Env) {
        if Self::check_is_paused(env) {
            panic_with_error!(env, TipJarError::ContractPaused);
        }
    }

    /// Core pause check: returns true if the contract is currently paused.
    /// Handles auto-unpause by clearing pause state when `PauseUntil` has elapsed.
    fn check_is_paused(env: &Env) -> bool {
        let paused: bool = env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Paused)
            .unwrap_or(false);
        if !paused {
            return false;
        }
        // Check auto-unpause
        if let Some(unpause_time) = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::PauseUntil)
        {
            if env.ledger().timestamp() >= unpause_time {
                env.storage().instance().set(&DataKey::Paused, &false);
                env.storage().instance().remove(&DataKey::PauseReason);
                env.storage().instance().remove(&DataKey::PauseUntil);
                return false;
            }
        }
        true
    }

    fn update_breaker_state(env: &Env, state: &CircuitBreakerState) {
        env.storage()
            .instance()
            .set(&DataKey::CircuitBreaker(CircuitBreakerKey::State), state);
    }

    fn add_creator_auction(env: &Env, creator: &Address, auction_id: u64) {
        let mut auctions: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::Auction(AuctionKey::CreatorList(creator.clone())))
            .unwrap_or_else(|| Vec::new(env));
        auctions.push_back(auction_id);
        env.storage().persistent().set(
            &DataKey::Auction(AuctionKey::CreatorList(creator.clone())),
            &auctions,
        );
    }

    fn get_auction_internal(env: &Env, auction_id: u64) -> Option<Auction> {
        env.storage()
            .persistent()
            .get(&DataKey::Auction(AuctionKey::Record(auction_id)))
    }

    // ── leaderboard helpers ──────────────────────────────────────────────────

    fn update_leaderboard_stats(env: &Env, tipper: &Address, creator: &Address, amount: i128) {
        const BUCKET_ALL_TIME: u32 = 0;
        Self::update_aggregate(
            env,
            tipper,
            amount,
            BUCKET_ALL_TIME,
            ParticipantKind::Tipper,
        );
        Self::update_aggregate(
            env,
            creator,
            amount,
            BUCKET_ALL_TIME,
            ParticipantKind::Creator,
        );
    }

    fn update_aggregate(
        env: &Env,
        addr: &Address,
        amount: i128,
        bucket: u32,
        kind: ParticipantKind,
    ) {
        let agg_key = match kind {
            ParticipantKind::Tipper => DataKey::Stats(StatsKey::TipperAgg(addr.clone(), bucket)),
            ParticipantKind::Creator => DataKey::Stats(StatsKey::CreatorAgg(addr.clone(), bucket)),
        };
        let mut entry: LeaderboardEntry =
            env.storage()
                .persistent()
                .get(&agg_key)
                .unwrap_or(LeaderboardEntry {
                    address: addr.clone(),
                    total_amount: 0,
                    tip_count: 0,
                });
        entry.total_amount += amount;
        entry.tip_count += 1;
        env.storage().persistent().set(&agg_key, &entry);

        let part_key = match kind {
            ParticipantKind::Tipper => DataKey::Stats(StatsKey::TipperParts(bucket)),
            ParticipantKind::Creator => DataKey::Stats(StatsKey::CreatorParts(bucket)),
        };
        let mut participants: Vec<Address> = env
            .storage()
            .persistent()
            .get(&part_key)
            .unwrap_or_else(|| Vec::new(env));
        if !participants.contains(addr) {
            participants.push_back(addr.clone());
            env.storage().persistent().set(&part_key, &participants);
        }
    }

    fn accrue_interest(env: &Env, account: &mut CreditAccount, config: &CreditConfig) {
        let now = env.ledger().timestamp();
        let elapsed = now.saturating_sub(account.last_update);
        if elapsed == 0 || account.principal == 0 {
            return;
        }

        // Rate is per 30 days (2,592,000 seconds)
        let interest = (account.principal * config.interest_rate_bps as i128 * elapsed as i128)
            / (10_000 * 30 * 24 * 3600);
        account.interest_accrued = account.interest_accrued.saturating_add(interest);
        account.last_update = now;
    }

    fn process_repayment(
        env: &Env,
        creator: &Address,
        token: &Address,
        amount_to_credit: i128,
    ) -> i128 {
        let config = env
            .storage()
            .instance()
            .get::<soroban_sdk::Symbol, CreditConfig>(&symbol_short!("cr_cfg"));
        if config.is_none() || !config.as_ref().unwrap().enabled {
            return amount_to_credit;
        }
        let config = config.unwrap();

        let mut account = match env
            .storage()
            .persistent()
            .get::<soroban_sdk::Vec<soroban_sdk::Val>, CreditAccount>(
                &(symbol_short!("cr_acc"), creator.clone(), token.clone()).into_val(env),
            ) {
            Some(acc) => acc,
            None => return amount_to_credit,
        };

        if account.principal == 0 && account.interest_accrued == 0 {
            return amount_to_credit;
        }

        Self::accrue_interest(env, &mut account, &config);

        let repayment_share = (amount_to_credit * config.repayment_share_bps as i128) / 10_000;
        let total_debt = account.principal + account.interest_accrued;
        let actual_repayment = if repayment_share > total_debt {
            total_debt
        } else {
            repayment_share
        };

        if actual_repayment == 0 {
            return amount_to_credit;
        }

        let mut interest_paid = 0;
        if actual_repayment <= account.interest_accrued {
            account.interest_accrued -= actual_repayment;
            interest_paid = actual_repayment;
        } else {
            interest_paid = account.interest_accrued;
            let principal_paid = actual_repayment - interest_paid;
            account.interest_accrued = 0;
            account.principal -= principal_paid;
        }

        account.total_repaid += actual_repayment;
        account.last_update = env.ledger().timestamp();
        env.storage().persistent().set(
            &(symbol_short!("cr_acc"), creator.clone(), token.clone()).into_val(env),
            &account,
        );

        // Repayment goes back to PlatformFeeBalance
        let fee_key = DataKey::Fee(FeeKey::Balance(token.clone()));
        let platform_bal: i128 = env.storage().instance().get(&fee_key).unwrap_or(0);
        env.storage()
            .instance()
            .set(&fee_key, &(platform_bal + actual_repayment));

        Self::record_credit_event(env, creator, token, actual_repayment, interest_paid, true);

        env.events().publish(
            (symbol_short!("repay_au"), creator.clone()),
            (actual_repayment, token.clone()),
        );

        amount_to_credit - actual_repayment
    }

    fn record_credit_event(
        env: &Env,
        creator: &Address,
        token: &Address,
        amount: i128,
        interest_portion: i128,
        is_repayment: bool,
    ) {
        let count_key = symbol_short!("cr_ctr");
        let id: u64 = env.storage().instance().get(&count_key).unwrap_or(0);
        env.storage().instance().set(&count_key, &(id + 1));

        let record = CreditRecord {
            id,
            amount,
            interest_portion,
            timestamp: env.ledger().timestamp(),
            is_repayment,
        };

        let history_key = (symbol_short!("cr_hst"), creator.clone(), token.clone()).into_val(env);
        let mut history: Vec<CreditRecord> = env
            .storage()
            .persistent()
            .get(&history_key)
            .unwrap_or_else(|| Vec::new(env));
        if history.len() >= 50 {
            history.remove(0);
        }
        history.push_back(record);
        env.storage().persistent().set(&history_key, &history);
    }

    // ── initialization ───────────────────────────────────────────────────────

    /// One-time setup to choose the administrator for the TipJar.
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, TipJarError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::Fee(FeeKey::BasisPoints), &0u32);
        env.storage().instance().set(&DataKey::RefundWindow, &0u64);
    }

    /// Sets an off-chain condition flag that can later be referenced in
    /// conditional tip execution.
    pub fn set_offchain_condition(
        env: Env,
        oracle: Address,
        condition_id: BytesN<32>,
        approved: bool,
    ) {
        oracle.require_auth();
        conditions::evaluator::set_offchain_approval(&env, &condition_id, approved);
    }

    /// Adds a token to the whitelist. Admin only.
    pub fn add_token(env: Env, admin: Address, token: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        env.storage()
            .instance()
            .set(&DataKey::TokenWhitelist(token), &true);
    }

    /// Removes a token from the whitelist. Admin only.
    pub fn remove_token(env: Env, admin: Address, token: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        env.storage()
            .instance()
            .set(&DataKey::TokenWhitelist(token), &false);
    }

    /// Returns `true` if `token` is on the whitelist.
    pub fn is_whitelisted(env: Env, token: Address) -> bool {
        env.storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::TokenWhitelist(token))
            .unwrap_or(false)
    }

    /// Pauses all state-changing operations. Admin only.
    ///
    /// `reason` is stored on-chain for transparency.
    /// Emits `("paused",)` with data `(admin, reason)`.
    pub fn pause(env: Env, admin: Address, reason: String) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        env.storage().instance().set(&DataKey::Paused, &true);
        env.storage().instance().set(&DataKey::PauseReason, &reason);
        env.events()
            .publish((symbol_short!("paused"),), (admin, reason));
    }

    /// Resumes normal operations. Admin only.
    ///
    /// Emits `("unpaused",)` with data `admin`.
    pub fn unpause(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().remove(&DataKey::PauseReason);
        env.events().publish((symbol_short!("unpaused"),), admin);
    }

    /// Returns `true` when the contract is paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Sets the circuit breaker configuration. Admin only.
    pub fn set_circuit_breaker_config(env: Env, admin: Address, config: CircuitBreakerConfig) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        env.storage()
            .instance()
            .set(&DataKey::CircuitBreaker(CircuitBreakerKey::Cfg), &config);
    }

    /// Returns the current circuit breaker configuration.
    pub fn get_circuit_breaker_config(env: Env) -> Option<CircuitBreakerConfig> {
        env.storage()
            .instance()
            .get(&DataKey::CircuitBreaker(CircuitBreakerKey::Cfg))
    }

    /// Returns the current circuit breaker state.
    pub fn get_circuit_breaker_state(env: Env) -> Option<CircuitBreakerState> {
        env.storage()
            .instance()
            .get(&DataKey::CircuitBreaker(CircuitBreakerKey::State))
    }

    /// Manually triggers the circuit breaker. Admin only.
    pub fn trigger_circuit_breaker(env: Env, admin: Address, reason: soroban_sdk::Symbol) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        let config = env
            .storage()
            .instance()
            .get::<DataKey, CircuitBreakerConfig>(&DataKey::CircuitBreaker(CircuitBreakerKey::Cfg))
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::CircuitBreakerNotConfigured));

        let mut state = env
            .storage()
            .instance()
            .get::<DataKey, CircuitBreakerState>(&DataKey::CircuitBreaker(CircuitBreakerKey::State))
            .unwrap_or(CircuitBreakerState {
                window_start: env.ledger().timestamp(),
                current_volume: 0,
                halted_until: 0,
                trigger_count: 0,
            });

        Self::trigger_breaker(&env, &mut state, &config, reason);
    }

    /// Resets the circuit breaker state, clearing any active halts. Admin only.
    pub fn reset_circuit_breaker(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        if let Some(mut state) = env
            .storage()
            .instance()
            .get::<DataKey, CircuitBreakerState>(&DataKey::CircuitBreaker(CircuitBreakerKey::State))
        {
            state.halted_until = 0;
            state.current_volume = 0;
            state.window_start = env.ledger().timestamp();
            Self::update_breaker_state(&env, &state);
            env.events().publish((symbol_short!("cb_reset"),), admin);
        }
    }

    // ── Enhanced Circuit Breaker Functions ──────────────────────────────────

    /// Sets the enhanced circuit breaker configuration. Admin only.
    ///
    /// Validates all configuration parameters before storing.
    /// Emits `("enhanced_cb_config",)` with data `admin`.
    pub fn set_enhanced_cb_config(env: Env, admin: Address, config: EnhancedCircuitBreakerConfig) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        // Validate configuration before storing
        if let Err(_) = config.validate() {
            panic_with_error!(&env, CreditError::EnhancedCircuitBreakerError);
        }

        env.storage().instance().set(
            &DataKey::CircuitBreaker(CircuitBreakerKey::EnhancedConfig),
            &config,
        );

        env.events().publish((symbol_short!("cb_cfg"),), admin);
    }

    /// Returns the current enhanced circuit breaker configuration.
    pub fn get_enhanced_cb_config(env: Env) -> Option<EnhancedCircuitBreakerConfig> {
        env.storage()
            .instance()
            .get(&DataKey::CircuitBreaker(CircuitBreakerKey::EnhancedConfig))
    }

    /// Initializes enhanced circuit breaker with default configuration. Admin only.
    ///
    /// Creates default configuration if not already present.
    /// Emits `("enhanced_cb_init",)` with data `admin`.
    pub fn init_enhanced_cb(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        // Only initialize if not already configured
        if env
            .storage()
            .instance()
            .has(&DataKey::CircuitBreaker(CircuitBreakerKey::EnhancedConfig))
        {
            return;
        }

        let default_config = EnhancedCircuitBreakerConfig::default_config();
        env.storage().instance().set(
            &DataKey::CircuitBreaker(CircuitBreakerKey::EnhancedConfig),
            &default_config,
        );

        env.events().publish((symbol_short!("cb_init"),), admin);
    }

    /// Pauses all state-changing operations. Admin only.
    ///
    /// `reason` is stored on-chain for transparency.
    /// `duration_seconds` optionally sets an auto-unpause time; pass `None` for indefinite pause.
    /// Emits `("paused",)` with data `(admin, reason, unpause_time_or_zero)`.
    pub fn pause(env: Env, admin: Address, reason: String, duration_seconds: Option<u64>) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        env.storage().instance().set(&DataKey::Paused, &true);
        env.storage().instance().set(&DataKey::PauseReason, &reason);
        let unpause_time: u64 = if let Some(duration) = duration_seconds {
            let t = env.ledger().timestamp().saturating_add(duration);
            env.storage().instance().set(&DataKey::PauseUntil, &t);
            t
        } else {
            env.storage().instance().remove(&DataKey::PauseUntil);
            0
        };
        env.events()
            .publish((symbol_short!("paused"),), (admin, reason, unpause_time));
    }

    /// Resumes normal operations. Admin only. Fails if contract is not paused.
    ///
    /// Emits `("unpaused",)` with data `admin`.
    pub fn unpause(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        if !Self::check_is_paused(&env) {
            panic_with_error!(&env, TipJarError::NotPaused);
        }
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().remove(&DataKey::PauseReason);
        env.storage().instance().remove(&DataKey::PauseUntil);
        env.events().publish((symbol_short!("unpaused"),), admin);
    }

    /// Returns `true` when the contract is currently paused (respects auto-unpause).
    pub fn is_paused(env: Env) -> bool {
        Self::check_is_paused(&env)
    }

    /// Returns the pause reason if the contract is paused, or `None` otherwise.
    pub fn get_pause_reason(env: Env) -> Option<String> {
        if !Self::check_is_paused(&env) {
            return None;
        }
        env.storage().instance().get(&DataKey::PauseReason)
    }

    /// Returns the auto-unpause timestamp if set, or `None` for indefinite pause.
    pub fn get_pause_until(env: Env) -> Option<u64> {
        env.storage().instance().get(&DataKey::PauseUntil)
    }

    /// Sets token-specific circuit breaker limits. Admin only.
    ///
    /// Stores token-specific limits separately from main configuration.
    /// Emits `("token_cb_limit",)` with data `(token, limit)`.
    pub fn set_token_circuit_breaker_limit(env: Env, admin: Address, token: Address, limit: i128) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        if limit <= 0 {
            panic_with_error!(&env, CreditError::EnhancedCircuitBreakerError);
        }

        // Store token-specific limit separately
        env.storage().persistent().set(
            &DataKey::CircuitBreaker(CircuitBreakerKey::TokenLimits(token.clone())),
            &limit,
        );

        env.events()
            .publish((symbol_short!("tk_cb_lmt"),), (token, limit));
    }

    /// Removes token-specific circuit breaker limits. Admin only.
    ///
    /// Reverts token to using global limits.
    /// Emits `("token_cb_remove",)` with data `token`.
    pub fn remove_token_circuit_breaker_limit(env: Env, admin: Address, token: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        env.storage().persistent().remove(&DataKey::CircuitBreaker(
            CircuitBreakerKey::TokenLimits(token.clone()),
        ));

        env.events().publish((symbol_short!("tk_cb_rmv"),), token);
    }

    /// Sets the credit system configuration. Admin only.
    pub fn set_credit_config(env: Env, admin: Address, config: CreditConfig) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        env.storage()
            .instance()
            .set(&symbol_short!("cr_cfg"), &config);
    }

    /// Returns the current credit system configuration.
    pub fn get_credit_config(env: Env) -> Option<CreditConfig> {
        env.storage().instance().get(&symbol_short!("cr_cfg"))
    }

    /// Computes the current credit limit for a creator/token pair.
    pub fn get_credit_limit(env: Env, creator: Address, token: Address) -> i128 {
        let config = env
            .storage()
            .instance()
            .get::<soroban_sdk::Symbol, CreditConfig>(&symbol_short!("cr_cfg"));
        let Some(config) = config else {
            return 0;
        };
        if !config.enabled {
            return 0;
        }

        let total_tips = env
            .storage()
            .persistent()
            .get::<DataKey, i128>(&DataKey::CreatorTotal(creator, token))
            .unwrap_or(0);
        if total_tips < config.min_total_tips {
            return 0;
        }
        (total_tips * config.max_credit_ratio_bps as i128) / 10_000
    }

    /// Borrow against future tips. Creator only.
    pub fn borrow(env: Env, creator: Address, token: Address, amount: i128) {
        Self::require_not_paused(&env);
        creator.require_auth();

        let config = env
            .storage()
            .instance()
            .get::<soroban_sdk::Symbol, CreditConfig>(&symbol_short!("cr_cfg"))
            .unwrap_or_else(|| panic_with_error!(&env, OtherError::CreditDisabled));

        if !config.enabled {
            panic_with_error!(&env, OtherError::CreditDisabled);
        }
        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }

        let total_tips = env
            .storage()
            .persistent()
            .get::<DataKey, i128>(&DataKey::CreatorTotal(creator.clone(), token.clone()))
            .unwrap_or(0);
        if total_tips < config.min_total_tips {
            panic_with_error!(&env, OtherError::IneligibleForCredit);
        }

        let limit = (total_tips * config.max_credit_ratio_bps as i128) / 10_000;

        let mut account = env
            .storage()
            .persistent()
            .get::<soroban_sdk::Vec<soroban_sdk::Val>, CreditAccount>(
                &(symbol_short!("cr_acc"), creator.clone(), token.clone()).into_val(&env),
            )
            .unwrap_or(CreditAccount {
                principal: 0,
                interest_accrued: 0,
                last_update: env.ledger().timestamp(),
                total_borrowed: 0,
                total_repaid: 0,
            });

        Self::accrue_interest(&env, &mut account, &config);

        if account.principal + account.interest_accrued + amount > limit {
            panic_with_error!(&env, OtherError::CreditLimitExceeded);
        }

        // Check lending liquidity from platform fees
        let fee_key = DataKey::Fee(FeeKey::Balance(token.clone()));
        let platform_bal: i128 = env.storage().instance().get(&fee_key).unwrap_or(0);
        if platform_bal < amount {
            panic_with_error!(&env, OtherError::InsufficientLendingLiquidity);
        }

        // Deduct from platform balance
        env.storage()
            .instance()
            .set(&fee_key, &(platform_bal - amount));

        // Credit creator balance
        let bal_key = DataKey::CreatorBalance(creator.clone(), token.clone());
        let current_bal: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&bal_key, &(current_bal + amount));

        // Update credit account
        account.principal += amount;
        account.total_borrowed += amount;
        account.last_update = env.ledger().timestamp();
        env.storage().persistent().set(
            &(symbol_short!("cr_acc"), creator.clone(), token.clone()).into_val(&env),
            &account,
        );

        Self::record_credit_event(&env, &creator, &token, amount, 0, false);

        env.events()
            .publish((symbol_short!("borrow"), creator), (amount, token));
    }

    /// Manually repay outstanding credit. Creator only.
    pub fn repay_credit(env: Env, creator: Address, token: Address, amount: i128) {
        Self::require_not_paused(&env);
        creator.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }

        let mut account = env
            .storage()
            .persistent()
            .get::<soroban_sdk::Vec<soroban_sdk::Val>, CreditAccount>(
                &(symbol_short!("cr_acc"), creator.clone(), token.clone()).into_val(&env),
            )
            .unwrap_or_else(|| panic_with_error!(&env, OtherError::NoActiveCredit));

        let config = env
            .storage()
            .instance()
            .get::<soroban_sdk::Symbol, CreditConfig>(&symbol_short!("cr_cfg"))
            .unwrap();
        Self::accrue_interest(&env, &mut account, &config);

        let bal_key = DataKey::CreatorBalance(creator.clone(), token.clone());
        let bal: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
        if bal < amount {
            panic_with_error!(&env, TipJarError::InsufficientBalance);
        }

        let total_debt = account.principal + account.interest_accrued;
        let actual_repayment = if amount > total_debt {
            total_debt
        } else {
            amount
        };

        if actual_repayment == 0 {
            return;
        }

        env.storage()
            .persistent()
            .set(&bal_key, &(bal - actual_repayment));

        let mut interest_paid = 0;
        if actual_repayment <= account.interest_accrued {
            account.interest_accrued -= actual_repayment;
            interest_paid = actual_repayment;
        } else {
            interest_paid = account.interest_accrued;
            let principal_paid = actual_repayment - interest_paid;
            account.interest_accrued = 0;
            account.principal -= principal_paid;
        }

        account.total_repaid += actual_repayment;
        account.last_update = env.ledger().timestamp();
        env.storage().persistent().set(
            &(symbol_short!("cr_acc"), creator.clone(), token.clone()).into_val(&env),
            &account,
        );

        let fee_key = DataKey::Fee(FeeKey::Balance(token.clone()));
        let platform_bal: i128 = env.storage().instance().get(&fee_key).unwrap_or(0);
        env.storage()
            .instance()
            .set(&fee_key, &(platform_bal + actual_repayment));

        Self::record_credit_event(
            &env,
            &creator,
            &token,
            actual_repayment,
            interest_paid,
            true,
        );

        env.events()
            .publish((symbol_short!("repay"), creator), (actual_repayment, token));
    }

    /// Returns the credit account for a creator.
    pub fn get_credit_account(env: Env, creator: Address, token: Address) -> Option<CreditAccount> {
        env.storage()
            .persistent()
            .get(&(symbol_short!("cr_acc"), creator, token).into_val(&env))
    }

    /// Returns the credit history for a creator.
    pub fn get_credit_history(env: Env, creator: Address, token: Address) -> Vec<CreditRecord> {
        env.storage()
            .persistent()
            .get(&(symbol_short!("cr_hst"), creator, token).into_val(&env))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Transfers `amount` of `token` from `sender` into escrow for `creator`.
    ///
    /// Deducts the platform fee before crediting the creator. Returns the tip ID.
    /// Emits `("tip", creator)` with data `(sender, amount)`.
    pub fn tip(env: Env, sender: Address, creator: Address, token: Address, amount: i128) -> u64 {
        Self::require_not_paused(&env);
        Self::check_circuit_breaker(&env, amount);
        sender.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        let fee_bp: u32 = env
            .storage()
            .instance()
            .get(&DataKey::Fee(FeeKey::BasisPoints))
            .unwrap_or(0);
        let fee: i128 = (amount * fee_bp as i128) / 10_000;

        // --- Insurance Premium Calculation ---
        let ins_enabled: bool = env
            .storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Enabled))
            .unwrap_or(true);
        let mut ins_premium: i128 = 0;
        if ins_enabled {
            if let Some(config) = env
                .storage()
                .instance()
                .get::<DataKey, InsurancePoolConfig>(&DataKey::Insurance(InsuranceKey::Cfg))
            {
                ins_premium = (amount * config.tip_premium_bps as i128) / 10_000;
            }
        }

        let creator_amount = amount
            .checked_sub(fee)
            .and_then(|a| a.checked_sub(ins_premium))
            .unwrap_or(0);

        // ── state updates before external call (CEI pattern) ─────────────────
        if fee > 0 {
            let fee_key = DataKey::Fee(FeeKey::Balance(token.clone()));
            let current_fee: i128 = env.storage().instance().get(&fee_key).unwrap_or(0);
            let new_fee_bal: i128 = current_fee.checked_add(fee).expect("fee overflow");
            env.storage().instance().set(&fee_key, &new_fee_bal);
        }

        if ins_premium > 0 {
            let pool_key = DataKey::Insurance(InsuranceKey::Token(token.clone()));
            let mut pool: InsurancePool =
                env.storage()
                    .persistent()
                    .get(&pool_key)
                    .unwrap_or_else(|| InsurancePool {
                        token: token.clone(),
                        total_reserves: 0,
                        total_contributions: 0,
                        total_claims_paid: 0,
                        active_claims: 0,
                        total_claims: 0,
                        last_payout_time: env.ledger().timestamp(),
                    });
            pool.total_reserves += ins_premium;
            env.storage().persistent().set(&pool_key, &pool);
        }

        let bal_key = DataKey::CreatorBalance(creator.clone(), token.clone());
        let existing_bal: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
        let net_amount = Self::process_repayment(&env, &creator, &token, creator_amount);
        let new_bal: i128 = existing_bal
            .checked_add(net_amount)
            .expect("balance overflow");
        env.storage().persistent().set(&bal_key, &new_bal);

        let tot_key = DataKey::CreatorTotal(creator.clone(), token.clone());
        let existing_tot: i128 = env.storage().persistent().get(&tot_key).unwrap_or(0);
        let new_tot: i128 = existing_tot
            .checked_add(creator_amount)
            .expect("total overflow");
        env.storage().persistent().set(&tot_key, &new_tot);

        let tip_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::Tip(TipKey::Ctr))
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::Tip(TipKey::Ctr), &(tip_id + 1));

        Self::update_leaderboard_stats(&env, &sender, &creator, creator_amount);

        // Record reputation for tipper and creator
        reputation::record_tip_sent(&env, &sender, amount);
        reputation::record_tip_received(&env, &creator, creator_amount);

        // Track which tokens this creator has received
        Self::track_creator_token(&env, &creator, &token);

        // Check and award milestones
        Self::check_and_award_milestones(&env, &creator, &token, new_tot);

        // ── external call last ───────────────────────────────────────────────
        token::Client::new(&env, &token).transfer(
            &sender,
            &env.current_contract_address(),
            &amount,
        );

        env.events().publish(
            (symbol_short!("tip"), creator.clone()),
            (sender, creator_amount),
        );
        tip_id
    }

    /// Withdraws the full escrowed balance for `creator` in `token`.
    ///
    /// Enforces per-creator (or default) daily limits and cooldown periods.
    /// Emits `("withdraw", creator)` with data `amount`.
    pub fn withdraw(env: Env, creator: Address, token: Address) {
        Self::require_not_paused(&env);
        creator.require_auth();
        let bal_key = DataKey::CreatorBalance(creator.clone(), token.clone());
        let amount: i128 = env
            .storage()
            .persistent()
            .get(&bal_key)
            .unwrap_or_else(|| env.storage().instance().get(&bal_key).unwrap_or(0));
        if amount == 0 {
            panic_with_error!(&env, TipJarError::NothingToWithdraw);
        }
        Self::check_and_update_withdrawal_limits(&env, &creator, amount);
        env.storage().persistent().set(&bal_key, &0i128);
        token::Client::new(&env, &token).transfer(
            &env.current_contract_address(),
            &creator,
            &amount,
        );
        events::emit_withdraw_event(&env, &creator, amount, &token);
    }

    /// Withdraws a specific amount from the escrowed balance for `creator` in `token`.
    ///
    /// Validates amount is at least the minimum (1 token = 1_0000000 stroops) and does not exceed available balance.
    /// Enforces per-creator (or default) daily limits and cooldown periods.
    /// Emits `("withdraw", creator)` with data `amount`.
    pub fn withdraw_amount(env: Env, creator: Address, token: Address, amount: i128) {
        Self::require_not_paused(&env);
        creator.require_auth();

        const MIN_WITHDRAWAL: i128 = 1_0000000;
        if amount < MIN_WITHDRAWAL {
            panic_with_error!(&env, TipJarError::AmountTooSmall);
        }

        let bal_key = DataKey::CreatorBalance(creator.clone(), token.clone());
        let current_balance: i128 = env
            .storage()
            .persistent()
            .get(&bal_key)
            .unwrap_or_else(|| env.storage().instance().get(&bal_key).unwrap_or(0));

        if amount > current_balance {
            panic_with_error!(&env, TipJarError::InsufficientBalance);
        }

        Self::check_and_update_withdrawal_limits(&env, &creator, amount);

        let new_balance = current_balance - amount;
        env.storage().persistent().set(&bal_key, &new_balance);

        token::Client::new(&env, &token).transfer(
            &env.current_contract_address(),
            &creator,
            &amount,
        );
        events::emit_withdraw_event(&env, &creator, amount, &token);
    }

    /// Creates an auction for a creator with an optional reserve price.
    ///
    /// `duration_seconds` must be greater than zero.
    /// Emits `("auction_created",)` with data `(auction_id, creator, token, reserve_price, ends_at)`.
    pub fn create_auction(
        env: Env,
        creator: Address,
        token: Address,
        reserve_price: i128,
        duration_seconds: u64,
    ) -> u64 {
        Self::require_not_paused(&env);
        creator.require_auth();
        if reserve_price < 0 {
            panic_with_error!(&env, AuctionError::InvalidReservePrice);
        }
        if duration_seconds == 0 {
            panic_with_error!(&env, AuctionError::InvalidAuctionDuration);
        }

        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        let auction_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::Auction(AuctionKey::Ctr))
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::Auction(AuctionKey::Ctr), &(auction_id + 1));

        let now = env.ledger().timestamp();
        let ends_at = now.saturating_add(duration_seconds);
        let auction = Auction {
            auction_id,
            creator: creator.clone(),
            token: token.clone(),
            reserve_price,
            highest_bid: 0,
            highest_bidder: None,
            ends_at,
            created_at: now,
            settled: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Auction(AuctionKey::Record(auction_id)), &auction);
        Self::add_creator_auction(&env, &creator, auction_id);

        env.events().publish(
            (symbol_short!("auc_crt"),),
            (
                auction_id,
                creator.clone(),
                token.clone(),
                reserve_price,
                ends_at,
            ),
        );
        auction_id
    }

    /// Places a bid on an active auction.
    ///
    /// `amount` must exceed the current highest bid and, for the first bid,
    /// must meet the reserve price.
    /// Emits `("auction_bid",)` with data `(auction_id, bidder, amount)`.
    pub fn place_bid(env: Env, bidder: Address, auction_id: u64, amount: i128) {
        Self::require_not_paused(&env);
        Self::check_circuit_breaker(&env, amount);
        bidder.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }

        let mut auction: Auction = env
            .storage()
            .persistent()
            .get::<DataKey, Auction>(&DataKey::Auction(AuctionKey::Record(auction_id)))
            .unwrap_or_else(|| panic_with_error!(&env, AuctionError::AuctionNotFound));
        if auction.settled {
            panic_with_error!(&env, AuctionError::AuctionAlreadySettled);
        }

        let now = env.ledger().timestamp();
        if now >= auction.ends_at {
            panic_with_error!(&env, AuctionError::AuctionEnded);
        }

        if amount <= auction.highest_bid {
            panic_with_error!(&env, AuctionError::AuctionBidTooLow);
        }
        if auction.highest_bid == 0 && amount < auction.reserve_price {
            panic_with_error!(&env, AuctionError::AuctionReserveNotMet);
        }

        let previous_highest_bid = auction.highest_bid;
        let previous_highest_bidder = auction.highest_bidder.clone();

        auction.highest_bid = amount;
        auction.highest_bidder = Some(bidder.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Auction(AuctionKey::Record(auction_id)), &auction);

        token::Client::new(&env, &auction.token).transfer(
            &bidder,
            &env.current_contract_address(),
            &amount,
        );

        if let Some(previous_bidder) = previous_highest_bidder {
            token::Client::new(&env, &auction.token).transfer(
                &env.current_contract_address(),
                &previous_bidder,
                &previous_highest_bid,
            );
        }

        env.events().publish(
            (symbol_short!("auc_bid"),),
            (auction_id, bidder.clone(), amount),
        );
    }

    /// Settles an ended auction and credits creator earnings if successful.
    ///
    /// Emits `("auction_settled",)` on success or `("auction_failed",)` when the
    /// reserve is not met.
    pub fn settle_auction(env: Env, creator: Address, auction_id: u64) {
        Self::require_not_paused(&env);
        creator.require_auth();

        let mut auction: Auction = env
            .storage()
            .persistent()
            .get::<DataKey, Auction>(&DataKey::Auction(AuctionKey::Record(auction_id)))
            .unwrap_or_else(|| panic_with_error!(&env, AuctionError::AuctionNotFound));
        if auction.creator != creator {
            panic_with_error!(&env, AuctionError::AuctionUnauthorized);
        }
        if auction.settled {
            panic_with_error!(&env, AuctionError::AuctionAlreadySettled);
        }

        let now = env.ledger().timestamp();
        if now < auction.ends_at {
            panic_with_error!(&env, AuctionError::AuctionNotEnded);
        }

        auction.settled = true;
        env.storage()
            .persistent()
            .set(&DataKey::Auction(AuctionKey::Record(auction_id)), &auction);

        if auction.highest_bid == 0 || auction.highest_bid < auction.reserve_price {
            if let Some(winner) = auction.highest_bidder {
                token::Client::new(&env, &auction.token).transfer(
                    &env.current_contract_address(),
                    &winner,
                    &auction.highest_bid,
                );
            }
            env.events().publish(
                (symbol_short!("auc_fail"),),
                (auction_id, auction.highest_bidder, auction.highest_bid),
            );
            return;
        }

        let fee_bp: u32 = env
            .storage()
            .instance()
            .get(&DataKey::Fee(FeeKey::BasisPoints))
            .unwrap_or(0);
        let fee: i128 = (auction.highest_bid * fee_bp as i128) / 10_000;
        let mut ins_premium: i128 = 0;
        let ins_enabled: bool = env
            .storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Enabled))
            .unwrap_or(true);
        if ins_enabled {
            if let Some(config) = env
                .storage()
                .instance()
                .get::<DataKey, InsurancePoolConfig>(&DataKey::Insurance(InsuranceKey::Cfg))
            {
                ins_premium = (auction.highest_bid * config.tip_premium_bps as i128) / 10_000;
            }
        }
        let creator_amount = auction
            .highest_bid
            .checked_sub(fee)
            .and_then(|a| a.checked_sub(ins_premium))
            .unwrap_or(0);

        if fee > 0 {
            let fee_key = DataKey::Fee(FeeKey::Balance(auction.token.clone()));
            let current_fee: i128 = env.storage().instance().get(&fee_key).unwrap_or(0);
            let new_fee_bal = current_fee.checked_add(fee).expect("fee overflow");
            env.storage().instance().set(&fee_key, &new_fee_bal);
        }

        if ins_premium > 0 {
            let pool_key = DataKey::Insurance(InsuranceKey::Token(auction.token.clone()));
            let mut pool: InsurancePool =
                env.storage()
                    .persistent()
                    .get(&pool_key)
                    .unwrap_or_else(|| InsurancePool {
                        token: auction.token.clone(),
                        total_reserves: 0,
                        total_contributions: 0,
                        total_claims_paid: 0,
                        active_claims: 0,
                        total_claims: 0,
                        last_payout_time: env.ledger().timestamp(),
                    });
            pool.total_reserves = pool
                .total_reserves
                .checked_add(ins_premium)
                .expect("insurance reserve overflow");
            env.storage().persistent().set(&pool_key, &pool);
        }

        if creator_amount > 0 {
            let bal_key = DataKey::CreatorBalance(creator.clone(), auction.token.clone());
            let existing_bal: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
            let net_amount =
                Self::process_repayment(&env, &creator, &auction.token, creator_amount);
            let new_bal = existing_bal
                .checked_add(net_amount)
                .expect("balance overflow");
            env.storage().persistent().set(&bal_key, &new_bal);

            let tot_key = DataKey::CreatorTotal(creator.clone(), auction.token.clone());
            let existing_tot: i128 = env.storage().persistent().get(&tot_key).unwrap_or(0);
            let new_tot = existing_tot
                .checked_add(creator_amount)
                .expect("total overflow");
            env.storage().persistent().set(&tot_key, &new_tot);

            if let Some(winner) = auction.highest_bidder.clone() {
                Self::update_leaderboard_stats(&env, &winner, &creator, creator_amount);
            }
            Self::track_creator_token(&env, &creator, &auction.token);
            Self::check_and_award_milestones(&env, &creator, &auction.token, new_tot);
        }

        env.events().publish(
            (symbol_short!("auc_sett"),),
            (
                auction_id,
                creator.clone(),
                auction.highest_bid,
                auction.token.clone(),
            ),
        );
    }

    /// Returns auction details by ID.
    pub fn get_auction(env: Env, auction_id: u64) -> Option<Auction> {
        env.storage()
            .persistent()
            .get(&DataKey::Auction(AuctionKey::Record(auction_id)))
    }

    /// Returns the auction IDs created by a given creator.
    pub fn get_creator_auctions(env: Env, creator: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::Auction(AuctionKey::CreatorList(creator)))
            .unwrap_or_else(|| Vec::new(env))
    }

    /// Authorizes a delegate to withdraw on behalf of `creator`.
    ///
    /// `max_amount` is the lifetime cap and `duration` is seconds until expiry.
    /// Emits `("delegate", creator)` with data `(delegate, max_amount, expires_at)`.
    pub fn delegate_withdrawal(
        env: Env,
        creator: Address,
        delegate: Address,
        max_amount: i128,
        duration: u64,
    ) {
        Self::require_not_paused(&env);
        creator.require_auth();
        if max_amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        if duration == 0 {
            panic_with_error!(&env, FeatureError::InvalidDuration);
        }

        let now = env.ledger().timestamp();
        let expires_at = now.saturating_add(duration);
        let delegation = Delegation {
            creator: creator.clone(),
            delegate: delegate.clone(),
            max_amount,
            used_amount: 0,
            expires_at,
            active: true,
        };

        env.storage().persistent().set(
            &DataKey::Delegation(DelegationKey::Active(creator.clone(), delegate.clone())),
            &delegation,
        );
        Self::add_delegate(&env, &creator, &delegate);
        Self::append_delegation_history(&env, &creator, &delegation);

        env.events().publish(
            (symbol_short!("delegate"),),
            (creator, delegate, max_amount, expires_at),
        );
    }

    /// Withdraws `amount` from `creator` balance to `delegate` when authorized.
    ///
    /// Enforces the creator's withdrawal limits and the delegation cap.
    /// Emits `("delegate_withdraw", creator)` with data `(delegate, amount, token)`.
    pub fn withdraw_as_delegate(
        env: Env,
        delegate: Address,
        creator: Address,
        token: Address,
        amount: i128,
    ) {
        Self::require_not_paused(&env);
        delegate.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }

        let mut delegation: Delegation = env
            .storage()
            .persistent()
            .get(&DataKey::Delegation(DelegationKey::Active(
                creator.clone(),
                delegate.clone(),
            )))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::DelegationNotFound));

        if !delegation.active {
            panic_with_error!(&env, FeatureError::DelegationInactive);
        }
        let now = env.ledger().timestamp();
        if now > delegation.expires_at {
            delegation.active = false;
            env.storage().persistent().set(
                &DataKey::Delegation(DelegationKey::Active(creator.clone(), delegate.clone())),
                &delegation,
            );
            Self::remove_delegate(&env, &creator, &delegate);
            Self::append_delegation_history(&env, &creator, &delegation);
            panic_with_error!(&env, FeatureError::DelegationExpired);
        }
        if delegation
            .used_amount
            .checked_add(amount)
            .unwrap_or(i128::MAX)
            > delegation.max_amount
        {
            panic_with_error!(&env, FeatureError::DelegationLimitExceeded);
        }

        let bal_key = DataKey::CreatorBalance(creator.clone(), token.clone());
        let balance: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
        if amount > balance || balance == 0 {
            panic_with_error!(&env, TipJarError::NothingToWithdraw);
        }

        Self::check_and_update_withdrawal_limits(&env, &creator, amount);

        env.storage()
            .persistent()
            .set(&bal_key, &(balance - amount));
        delegation.used_amount += amount;
        if delegation.used_amount >= delegation.max_amount {
            delegation.active = false;
            Self::remove_delegate(&env, &creator, &delegate);
        }
        env.storage().persistent().set(
            &DataKey::Delegation(DelegationKey::Active(creator.clone(), delegate.clone())),
            &delegation,
        );
        Self::append_delegation_history(&env, &creator, &delegation);

        token::Client::new(&env, &token).transfer(
            &env.current_contract_address(),
            &delegate,
            &amount,
        );

        env.events().publish(
            (symbol_short!("del_wdr"),),
            (creator, delegate, amount, token),
        );
    }

    /// Revokes an active delegation. Only the creator may revoke.
    /// Emits `("delegate_revoked", creator)` with data `(delegate,)`.
    pub fn revoke_delegation(env: Env, creator: Address, delegate: Address) {
        Self::require_not_paused(&env);
        creator.require_auth();

        let mut delegation: Delegation = env
            .storage()
            .persistent()
            .get(&DataKey::Delegation(DelegationKey::Active(
                creator.clone(),
                delegate.clone(),
            )))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::DelegationNotFound));

        if !delegation.active {
            panic_with_error!(&env, FeatureError::DelegationInactive);
        }

        delegation.active = false;
        env.storage().persistent().set(
            &DataKey::Delegation(DelegationKey::Active(creator.clone(), delegate.clone())),
            &delegation,
        );
        Self::remove_delegate(&env, &creator, &delegate);
        Self::append_delegation_history(&env, &creator, &delegation);

        env.events()
            .publish((symbol_short!("del_rev"),), (creator, delegate));
    }

    /// Returns the active delegation record for `creator` and `delegate`.
    pub fn get_delegation(env: Env, creator: Address, delegate: Address) -> Option<Delegation> {
        env.storage()
            .persistent()
            .get(&DataKey::Delegation(DelegationKey::Active(
                creator, delegate,
            )))
    }

    /// Returns the active delegate addresses for `creator`.
    pub fn get_delegates(env: Env, creator: Address) -> Vec<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::Delegation(DelegationKey::List(creator)))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Returns the historical delegation snapshots for `creator`.
    pub fn get_delegation_history(env: Env, creator: Address) -> Vec<Delegation> {
        env.storage()
            .persistent()
            .get(&DataKey::Delegation(DelegationKey::History(
                creator.clone(),
            )))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Records `token` in the creator's token list if not already present.
    fn track_creator_token(env: &Env, creator: &Address, token: &Address) {
        let key = DataKey::CreatorTokens(creator.clone());
        let mut tokens: Vec<Address> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));
        if !tokens.contains(token) {
            tokens.push_back(token.clone());
            env.storage().persistent().set(&key, &tokens);
        }
    }

    fn add_delegate(env: &Env, creator: &Address, delegate: &Address) {
        let mut delegates: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Delegation(DelegationKey::List(creator.clone())))
            .unwrap_or_else(|| Vec::new(env));
        if !delegates.contains(delegate) {
            delegates.push_back(delegate.clone());
            env.storage().persistent().set(
                &DataKey::Delegation(DelegationKey::List(creator.clone())),
                &delegates,
            );
        }
    }

    fn remove_delegate(env: &Env, creator: &Address, delegate: &Address) {
        let delegates: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Delegation(DelegationKey::List(creator.clone())))
            .unwrap_or_else(|| Vec::new(env));
        let mut remaining = Vec::new(env);
        for d in delegates.iter() {
            if d != delegate {
                remaining.push_back(d);
            }
        }
        env.storage().persistent().set(
            &DataKey::Delegation(DelegationKey::List(creator.clone())),
            &remaining,
        );
    }

    fn append_delegation_history(env: &Env, creator: &Address, delegation: &Delegation) {
        let mut history: Vec<Delegation> = env
            .storage()
            .persistent()
            .get(&DataKey::Delegation(DelegationKey::History(
                creator.clone(),
            )))
            .unwrap_or_else(|| Vec::new(env));
        history.push_back(delegation.clone());
        env.storage().persistent().set(
            &DataKey::Delegation(DelegationKey::History(creator.clone())),
            &history,
        );
    }

    /// Returns the current withdrawable balance for `creator` in `token`.
    pub fn get_withdrawable_balance(env: Env, creator: Address, token: Address) -> i128 {
        let key = DataKey::CreatorBalance(creator.clone(), token.clone());
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| env.storage().instance().get(&key).unwrap_or(0))
    }

    /// Returns the historical total tips received by `creator` in `token`.
    pub fn get_total_tips(env: Env, creator: Address, token: Address) -> i128 {
        let key = DataKey::CreatorTotal(creator.clone(), token.clone());
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| env.storage().instance().get(&key).unwrap_or(0))
    }

    // ── vesting schedules ────────────────────────────────────────────────────

    /// Creates a new vesting schedule for a tip.
    ///
    /// Parameters:
    /// - `tipper`: The address that sent the tip
    /// - `creator`: The address that will receive vested amounts
    /// - `token`: The token being vested
    /// - `amount`: Total amount to vest
    /// - `cliff_duration`: Seconds until vesting begins
    /// - `vesting_duration`: Total vesting period from start_time
    ///
    /// Emits: `("vest_new",)` with data `(creator, tipper, amount, start_time, vesting_duration, cliff_duration)`.
    pub fn create_vesting_schedule(
        env: Env,
        tipper: Address,
        creator: Address,
        token: Address,
        amount: i128,
        cliff_duration: u64,
        vesting_duration: u64,
    ) -> u64 {
        Self::require_not_paused(&env);
        Self::check_circuit_breaker(&env, amount);
        tipper.require_auth();

        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        if vesting_duration == 0 {
            panic_with_error!(&env, TipJarError::InvalidVestingDuration);
        }
        if cliff_duration > vesting_duration {
            panic_with_error!(&env, TipJarError::CliffExceedsVesting);
        }

        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        let now = env.ledger().timestamp();
        let schedule_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::Vesting(VestingKey::Ctr))
            .unwrap_or(0);

        let schedule = VestingSchedule {
            id: schedule_id,
            creator: creator.clone(),
            tipper: tipper.clone(),
            token: token.clone(),
            total_amount: amount,
            start_time: now,
            cliff_duration,
            vesting_duration,
            withdrawn: 0,
            created_at: now,
        };

        env.storage().persistent().set(
            &DataKey::Vesting(VestingKey::Schedule(schedule_id)),
            &schedule,
        );

        let mut schedules: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::Vesting(VestingKey::CreatorList(creator.clone())))
            .unwrap_or_else(|| Vec::new(&env));
        schedules.push_back(schedule_id);
        env.storage().persistent().set(
            &DataKey::Vesting(VestingKey::CreatorList(creator.clone())),
            &schedules,
        );

        env.storage()
            .instance()
            .set(&DataKey::Vesting(VestingKey::Ctr), &(schedule_id + 1));

        // Transfer tokens into contract for vesting
        token::Client::new(&env, &token).transfer(
            &tipper,
            &env.current_contract_address(),
            &amount,
        );

        env.events().publish(
            (symbol_short!("vest_new"),),
            (
                creator,
                tipper,
                amount,
                now,
                vesting_duration,
                cliff_duration,
            ),
        );

        schedule_id
    }

    /// Calculates the vested amount for a schedule at the current ledger time.
    fn calculate_vested_amount(env: &Env, schedule: &VestingSchedule) -> i128 {
        let current_time = env.ledger().timestamp();

        // No vesting until cliff is reached
        if current_time < schedule.start_time + schedule.cliff_duration {
            return 0;
        }

        let elapsed = current_time - schedule.start_time;

        // Full vesting after vesting_duration has passed
        if elapsed >= schedule.vesting_duration {
            return schedule.total_amount;
        }

        // Linear vesting between cliff and end
        (schedule.total_amount * elapsed as i128) / schedule.vesting_duration as i128
    }

    /// Returns the currently vested amount for a schedule.
    pub fn get_vested_amount(env: Env, schedule_id: u64) -> i128 {
        if schedule_id == 0 {
            panic_with_error!(&env, TipJarError::InvalidVestingId);
        }

        let schedule: VestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::Vesting(VestingKey::Schedule(schedule_id)))
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::VestingScheduleNotFound));

        Self::calculate_vested_amount(&env, &schedule)
    }

    /// Returns the available vested amount that can be withdrawn (vested - already withdrawn).
    pub fn get_available_vested_amount(env: Env, schedule_id: u64) -> i128 {
        if schedule_id == 0 {
            panic_with_error!(&env, TipJarError::InvalidVestingId);
        }

        let schedule: VestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::Vesting(VestingKey::Schedule(schedule_id)))
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::VestingScheduleNotFound));

        let vested = Self::calculate_vested_amount(&env, &schedule);
        vested.saturating_sub(schedule.withdrawn)
    }

    /// Withdraws available vested amounts from a vesting schedule to the creator.
    ///
    /// Emits: `("vest_withdraw",)` with data `(creator, schedule_id, amount, token)`.
    pub fn withdraw_vested(env: Env, creator: Address, schedule_id: u64) -> i128 {
        Self::require_not_paused(&env);
        creator.require_auth();

        if schedule_id == 0 {
            panic_with_error!(&env, TipJarError::InvalidVestingId);
        }

        let mut schedule: VestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::Vesting(VestingKey::Schedule(schedule_id)))
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::VestingScheduleNotFound));

        if schedule.creator != creator {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        let vested = Self::calculate_vested_amount(&env, &schedule);
        let available = vested.saturating_sub(schedule.withdrawn);

        if available <= 0 {
            panic_with_error!(&env, TipJarError::NoVestedAmount);
        }

        schedule.withdrawn = schedule
            .withdrawn
            .checked_add(available)
            .expect("withdrawn overflow");
        env.storage().persistent().set(
            &DataKey::Vesting(VestingKey::Schedule(schedule_id)),
            &schedule,
        );

        let net_amount = Self::process_repayment(&env, &creator, &schedule.token, available);
        token::Client::new(&env, &schedule.token).transfer(
            &env.current_contract_address(),
            &creator,
            &net_amount,
        );

        env.events().publish(
            (symbol_short!("vest_wdr"),),
            (creator, schedule_id, available, schedule.token),
        );

        available
    }

    /// Returns the vesting schedule details.
    pub fn get_vesting_schedule(env: Env, schedule_id: u64) -> Option<VestingSchedule> {
        if schedule_id == 0 {
            return None;
        }
        env.storage()
            .persistent()
            .get(&DataKey::Vesting(VestingKey::Schedule(schedule_id)))
    }

    /// Returns all vesting schedule IDs for a creator.
    pub fn get_creator_vesting_schedules(env: Env, creator: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::Vesting(VestingKey::CreatorList(creator)))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Returns all token addresses that `creator` has ever received tips in.
    pub fn get_creator_tokens(env: Env, creator: Address) -> Vec<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::CreatorTokens(creator))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Returns the top N participants (tippers or creators) for a given time period.
    ///
    /// Results are sorted by `total_amount` descending. `limit` is capped at 100.
    pub fn get_leaderboard(
        env: Env,
        period: TimePeriod,
        kind: ParticipantKind,
        limit: u32,
    ) -> Vec<LeaderboardEntry> {
        let bucket = match period {
            TimePeriod::AllTime => 0,
            _ => 0, // Monthly/Weekly not implemented; default to AllTime
        };
        let part_key = match kind {
            ParticipantKind::Tipper => DataKey::Stats(StatsKey::TipperParts(bucket)),
            ParticipantKind::Creator => DataKey::Stats(StatsKey::CreatorParts(bucket)),
        };
        let participants: Vec<Address> = env
            .storage()
            .persistent()
            .get(&part_key)
            .unwrap_or_else(|| Vec::new(&env));

        let cap = if limit > 100 { 100 } else { limit };
        let mut entries = Vec::new(&env);
        for addr in participants.iter() {
            let agg_key = match kind {
                ParticipantKind::Tipper => {
                    DataKey::Stats(StatsKey::TipperAgg(addr.clone(), bucket))
                }
                ParticipantKind::Creator => {
                    DataKey::Stats(StatsKey::CreatorAgg(addr.clone(), bucket))
                }
            };
            if let Some(entry) = env
                .storage()
                .persistent()
                .get::<_, LeaderboardEntry>(&agg_key)
            {
                entries.push_back(entry);
            }
        }

        // Build top-N by repeated linear scan (O(n*cap), cap ≤ 100, n ≤ participants).
        // Avoids in-place mutation since Soroban Vec has no set().
        let mut result = Vec::new(&env);
        let mut used = Vec::<u32>::new(&env);
        for _ in 0..cap {
            if used.len() == entries.len() {
                break;
            }
            let mut best_idx: Option<u32> = None;
            let mut best_amt: i128 = -1;
            for idx in 0..entries.len() {
                if used.contains(&idx) {
                    continue;
                }
                let amt = entries.get(idx).unwrap().total_amount;
                if amt > best_amt {
                    best_amt = amt;
                    best_idx = Some(idx);
                }
            }
            if let Some(idx) = best_idx {
                result.push_back(entries.get(idx).unwrap());
                used.push_back(idx);
            }
        }
        result
    }

    /// Executes a token tip only if all provided conditions evaluate to true.
    ///
    /// Returns true when the transfer is executed and false when conditions fail.
    pub fn execute_conditional_tip(
        env: Env,
        sender: Address,
        creator: Address,
        token: Address,
        amount: i128,
        condition_list: Vec<conditions::types::Condition>,
    ) -> bool {
        Self::require_not_paused(&env);
        Self::check_circuit_breaker(&env, amount);
        sender.require_auth();

        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }

        let is_valid = conditions::evaluator::evaluate_all(&env, &condition_list);
        if !is_valid {
            return false;
        }

        let net_amount = Self::process_repayment(&env, &creator, &token, amount);
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&sender, &creator, &net_amount);

        env.events().publish(
            (symbol_short!("condtip"), sender.clone()),
            (creator.clone(), token, amount),
        );

        true
    }

    /// Returns the last dynamically computed fee in basis points.
    ///
    /// Defaults to the base fee (100 bps = 1%) if no tip has been processed yet.
    pub fn get_current_fee_bps(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::Fee(FeeKey::CurrentDynamic))
            .unwrap_or(fees::calculator::BASE_FEE_BPS)
    }

    /// Like `tip`, but deducts a dynamic platform fee before crediting the creator.
    ///
    /// `congestion` is a `u32` mapped as: 0 = Low, 1 = Normal, 2 = High.
    /// The fee is retained in the contract; the creator receives `amount - fee`.
    ///
    /// Emits `("tip", creator)` with data `(sender, net_amount)` and
    /// `("fee", creator)` with data `(fee_amount, fee_bps)`.
    pub fn tip_with_fee(
        env: Env,
        sender: Address,
        creator: Address,
        token: Address,
        amount: i128,
        congestion: u32,
    ) {
        Self::require_not_paused(&env);
        Self::check_circuit_breaker(&env, amount);
        sender.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        let level = match congestion {
            0 => fees::CongestionLevel::Low,
            2 => fees::CongestionLevel::High,
            _ => fees::CongestionLevel::Normal,
        };
        let (fee, fee_bps) = fees::compute_fee(&env, amount, level);

        // --- Insurance Premium Calculation ---
        let ins_enabled: bool = env
            .storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Enabled))
            .unwrap_or(true);
        let mut ins_premium: i128 = 0;
        if ins_enabled {
            if let Some(config) = env
                .storage()
                .instance()
                .get::<DataKey, InsurancePoolConfig>(&DataKey::Insurance(InsuranceKey::Cfg))
            {
                ins_premium = (amount * config.tip_premium_bps as i128) / 10_000;
            }
        }

        let net = amount
            .checked_sub(fee)
            .and_then(|a| a.checked_sub(ins_premium))
            .unwrap_or(0);

        token::Client::new(&env, &token).transfer(
            &sender,
            &env.current_contract_address(),
            &amount,
        );

        if ins_premium > 0 {
            let pool_key = DataKey::Insurance(InsuranceKey::Token(token.clone()));
            let mut pool: InsurancePool =
                env.storage()
                    .persistent()
                    .get(&pool_key)
                    .unwrap_or_else(|| InsurancePool {
                        token: token.clone(),
                        total_reserves: 0,
                        total_contributions: 0,
                        total_claims_paid: 0,
                        active_claims: 0,
                        total_claims: 0,
                        last_payout_time: env.ledger().timestamp(),
                    });
            pool.total_reserves += ins_premium;
            env.storage().persistent().set(&pool_key, &pool);
        }

        let bal_key = DataKey::CreatorBalance(creator.clone(), token.clone());
        let current_bal: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
        let net_amount = Self::process_repayment(&env, &creator, &token, net);
        let new_bal: i128 = current_bal
            .checked_add(net_amount)
            .expect("balance overflow");
        env.storage().persistent().set(&bal_key, &new_bal);

        let tot_key = DataKey::CreatorTotal(creator.clone(), token.clone());
        let current_tot: i128 = env.storage().persistent().get(&tot_key).unwrap_or(0);
        let new_tot: i128 = current_tot.checked_add(net).expect("total overflow");
        env.storage().persistent().set(&tot_key, &new_tot);

        env.events()
            .publish((symbol_short!("tip"), creator.clone()), (sender, net));
        env.events()
            .publish((symbol_short!("fee"), creator.clone()), (fee, fee_bps));
    }

    /// Upgrades the contract WASM to `new_wasm_hash`. Admin only.
    ///
    /// Increments the on-chain version and emits `("upgraded",)` with the new
    /// version number.  All storage is preserved by the Soroban host.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        upgrade::upgrade(&env, new_wasm_hash);
    }

    /// Returns the current contract version (0 before the first upgrade).
    pub fn get_version(env: Env) -> u32 {
        upgrade::get_version(&env)
    }

    /// Applies any required state migrations after a WASM upgrade.
    ///
    /// This is a safe, idempotent entrypoint for release-specific storage
    /// updates that must run after the host preserves storage during upgrade.
    pub fn migrate_state(env: Env) {
        let version = upgrade::get_version(&env);
        match version {
            1 => {
                // No migration is required for the current v1 release.
                // Future upgrades can add version-specific migration paths
                // here, e.g. `migrate_v1_to_v2` or `migrate_v2_to_v3`.
            }
            _ => {}
        }
    }

    /// Sets the configuration for a subscription tier. Admin only.
    ///
    /// `price` is the amount charged per payment interval.
    /// `benefits` is a human-readable description of what the tier provides.
    /// Emits `("tier_set",)` with data `(tier, price)`.
    pub fn set_tier_config(
        env: Env,
        admin: Address,
        tier: SubscriptionTier,
        price: i128,
        benefits: String,
    ) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        if price <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        let config = TierConfig { price, benefits };
        env.storage()
            .persistent()
            .set(&DataKey::TierConfig(tier.clone()), &config);
        env.events()
            .publish((symbol_short!("tier_set"),), (tier, price));
    }

    /// Returns the configuration for a tier, or `None` if not configured.
    pub fn get_tier_config(env: Env, tier: SubscriptionTier) -> Option<TierConfig> {
        env.storage().persistent().get(&DataKey::TierConfig(tier))
    }

    /// Returns the benefits description for a tier, or `None` if not configured.
    pub fn get_tier_benefits(env: Env, tier: SubscriptionTier) -> Option<String> {
        env.storage()
            .persistent()
            .get::<DataKey, TierConfig>(&DataKey::TierConfig(tier))
            .map(|c| c.benefits)
    }

    /// Creates a recurring tip subscription from `subscriber` to `creator` at the given tier.
    ///
    /// The tier must be configured via `set_tier_config` first.
    /// The first payment becomes due immediately (at creation time).
    /// Minimum interval is 1 day (86 400 seconds).
    ///
    /// Emits `("sub_new", creator)` with data `(subscriber, amount, interval_seconds)`.
    pub fn create_subscription(
        env: Env,
        subscriber: Address,
        creator: Address,
        token: Address,
        amount: i128,
        interval_seconds: u64,
    ) {
        Self::require_not_paused(&env);
        Self::check_circuit_breaker(&env, amount);
        subscriber.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        const MIN_INTERVAL: u64 = 86_400;
        if interval_seconds < MIN_INTERVAL {
            panic_with_error!(&env, FeatureError::InvalidInterval);
        }
        let now = env.ledger().timestamp();
        let sub = Subscription {
            subscriber: subscriber.clone(),
            creator: creator.clone(),
            token,
            amount,
            interval_seconds,
            last_payment: 0,
            next_payment: now,
            status: SubscriptionStatus::Active,
            tier: SubscriptionTier::Bronze,
            pending_tier: None,
        };
        env.storage().persistent().set(
            &DataKey::Subscription(subscriber.clone(), creator.clone()),
            &sub,
        );
        env.events().publish(
            (symbol_short!("sub_new"), creator),
            (subscriber, amount, interval_seconds),
        );
    }

    /// Creates a tiered subscription from `subscriber` to `creator`.
    ///
    /// The tier must be configured via `set_tier_config`. The price from the tier
    /// config is used as the payment amount.
    /// Minimum interval is 1 day (86 400 seconds).
    ///
    /// Emits `("sub_new", creator)` with data `(subscriber, amount, interval_seconds)`.
    pub fn create_tiered_subscription(
        env: Env,
        subscriber: Address,
        creator: Address,
        token: Address,
        tier: SubscriptionTier,
        interval_seconds: u64,
    ) {
        Self::require_not_paused(&env);
        subscriber.require_auth();
        let config: TierConfig = env
            .storage()
            .persistent()
            .get(&DataKey::TierConfig(tier.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::TierNotConfigured));
        Self::check_circuit_breaker(&env, config.price);
        const MIN_INTERVAL: u64 = 86_400;
        if interval_seconds < MIN_INTERVAL {
            panic_with_error!(&env, FeatureError::InvalidInterval);
        }
        let now = env.ledger().timestamp();
        let amount = config.price;
        let sub = Subscription {
            subscriber: subscriber.clone(),
            creator: creator.clone(),
            token,
            amount,
            interval_seconds,
            last_payment: 0,
            next_payment: now,
            status: SubscriptionStatus::Active,
            tier: tier.clone(),
            pending_tier: None,
        };
        env.storage().persistent().set(
            &DataKey::Subscription(subscriber.clone(), creator.clone()),
            &sub,
        );
        env.events().publish(
            (symbol_short!("sub_new"), creator),
            (subscriber, amount, interval_seconds),
        );
    }

    /// Upgrades an active subscription to a higher tier immediately.
    ///
    /// Executes an immediate payment at the new tier's price and updates the
    /// subscription amount for future payments.
    /// Emits `("sub_upgr", creator)` with data `(subscriber, new_tier_price)`.
    pub fn upgrade_subscription(
        env: Env,
        subscriber: Address,
        creator: Address,
        new_tier: SubscriptionTier,
    ) {
        Self::require_not_paused(&env);
        subscriber.require_auth();
        let key = DataKey::Subscription(subscriber.clone(), creator.clone());
        let mut sub: Subscription = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::SubscriptionNotFound));
        if sub.status != SubscriptionStatus::Active {
            panic_with_error!(&env, TipJarError::SubscriptionNotActive);
        }
        let config: TierConfig = env
            .storage()
            .persistent()
            .get(&DataKey::TierConfig(new_tier.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::TierNotConfigured));

        Self::check_circuit_breaker(&env, config.price);

        // Execute immediate payment at new tier price.
        token::Client::new(&env, &sub.token).transfer(
            &subscriber,
            &env.current_contract_address(),
            &config.price,
        );
        let bal_key = DataKey::CreatorBalance(creator.clone(), sub.token.clone());
        let bal: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
        let net_amount = Self::process_repayment(&env, &creator, &sub.token, config.price);
        env.storage()
            .persistent()
            .set(&bal_key, &(bal + net_amount));
        let tot_key = DataKey::CreatorTotal(creator.clone(), sub.token.clone());
        let tot: i128 = env.storage().persistent().get(&tot_key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&tot_key, &(tot + config.price));

        let now = env.ledger().timestamp();
        sub.tier = new_tier;
        sub.amount = config.price;
        sub.last_payment = now;
        sub.next_payment = now + sub.interval_seconds;
        sub.pending_tier = None;
        env.storage().persistent().set(&key, &sub);
        env.events().publish(
            (symbol_short!("sub_upgr"), creator),
            (subscriber, config.price),
        );
    }

    /// Schedules a downgrade to a lower tier, effective at the next payment cycle.
    ///
    /// The current tier and amount remain active until `execute_subscription_payment`
    /// is called, at which point the pending tier is applied.
    /// Emits `("sub_dngr", creator)` with data `(subscriber, new_tier_price)`.
    pub fn downgrade_subscription(
        env: Env,
        subscriber: Address,
        creator: Address,
        new_tier: SubscriptionTier,
    ) {
        Self::require_not_paused(&env);
        subscriber.require_auth();
        let key = DataKey::Subscription(subscriber.clone(), creator.clone());
        let mut sub: Subscription = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::SubscriptionNotFound));
        if sub.status != SubscriptionStatus::Active {
            panic_with_error!(&env, TipJarError::SubscriptionNotActive);
        }
        // Validate the target tier is configured.
        let config: TierConfig = env
            .storage()
            .persistent()
            .get(&DataKey::TierConfig(new_tier.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::TierNotConfigured));
        sub.pending_tier = Some(new_tier);
        env.storage().persistent().set(&key, &sub);
        env.events().publish(
            (symbol_short!("sub_dngr"), creator),
            (subscriber, config.price),
        );
    }

    /// Executes a due subscription payment, transferring tokens from subscriber
    /// into escrow for the creator.
    ///
    /// Applies any pending tier downgrade before charging.
    /// Anyone may call this; the subscriber's auth is pulled via `transfer`.
    /// Emits `("sub_pay", creator)` with data `(subscriber, amount)`.
    pub fn execute_subscription_payment(env: Env, subscriber: Address, creator: Address) {
        Self::require_not_paused(&env);
        let key = DataKey::Subscription(subscriber.clone(), creator.clone());
        let mut sub: Subscription = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::SubscriptionNotFound));

        if sub.status != SubscriptionStatus::Active {
            panic_with_error!(&env, TipJarError::SubscriptionNotActive);
        }
        let now = env.ledger().timestamp();
        if now < sub.next_payment {
            panic_with_error!(&env, FeatureError::PaymentNotDue);
        }

        Self::check_circuit_breaker(&env, sub.amount);

        // Apply pending downgrade if present.
        if let Some(pending) = sub.pending_tier.clone() {
            if let Some(config) = env
                .storage()
                .persistent()
                .get::<DataKey, TierConfig>(&DataKey::TierConfig(pending.clone()))
            {
                sub.tier = pending;
                sub.amount = config.price;
            }
            sub.pending_tier = None;
        }

        token::Client::new(&env, &sub.token).transfer(
            &subscriber,
            &env.current_contract_address(),
            &sub.amount,
        );

        let bal_key = DataKey::CreatorBalance(creator.clone(), sub.token.clone());
        let bal: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
        let net_amount = Self::process_repayment(&env, &creator, &sub.token, sub.amount);
        env.storage()
            .persistent()
            .set(&bal_key, &(bal + net_amount));

        let tot_key = DataKey::CreatorTotal(creator.clone(), sub.token.clone());
        let tot: i128 = env.storage().persistent().get(&tot_key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&tot_key, &(tot + sub.amount));

        sub.last_payment = now;
        sub.next_payment = now + sub.interval_seconds;
        env.storage().persistent().set(&key, &sub);

        env.events().publish(
            (symbol_short!("sub_pay"), creator),
            (subscriber, sub.amount),
        );
    }

    /// Pauses an active subscription. Only the subscriber may pause.
    ///
    /// Emits `("sub_paus", creator)` with data `subscriber`.
    pub fn pause_subscription(env: Env, subscriber: Address, creator: Address) {
        Self::require_not_paused(&env);
        subscriber.require_auth();
        let key = DataKey::Subscription(subscriber.clone(), creator.clone());
        let mut sub: Subscription = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::SubscriptionNotFound));
        if sub.status != SubscriptionStatus::Active {
            panic_with_error!(&env, TipJarError::SubscriptionNotActive);
        }
        sub.status = SubscriptionStatus::Paused;
        env.storage().persistent().set(&key, &sub);
        env.events()
            .publish((symbol_short!("sub_paus"), creator), subscriber);
    }

    /// Resumes a paused subscription. Only the subscriber may resume.
    ///
    /// Resets `next_payment` to now so a payment can be executed immediately.
    /// Emits `("sub_res", creator)` with data `subscriber`.
    pub fn resume_subscription(env: Env, subscriber: Address, creator: Address) {
        Self::require_not_paused(&env);
        subscriber.require_auth();
        let key = DataKey::Subscription(subscriber.clone(), creator.clone());
        let mut sub: Subscription = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::SubscriptionNotFound));
        if sub.status != SubscriptionStatus::Paused {
            panic_with_error!(&env, TipJarError::SubscriptionNotActive);
        }
        sub.status = SubscriptionStatus::Active;
        sub.next_payment = env.ledger().timestamp();
        env.storage().persistent().set(&key, &sub);
        env.events()
            .publish((symbol_short!("sub_res"), creator), subscriber);
    }

    /// Cancels a subscription. Only the subscriber may cancel.
    ///
    /// Emits `("sub_cncl", creator)` with data `subscriber`.
    pub fn cancel_subscription(env: Env, subscriber: Address, creator: Address) {
        Self::require_not_paused(&env);
        subscriber.require_auth();
        let key = DataKey::Subscription(subscriber.clone(), creator.clone());
        let mut sub: Subscription = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::SubscriptionNotFound));
        sub.status = SubscriptionStatus::Cancelled;
        env.storage().persistent().set(&key, &sub);
        env.events()
            .publish((symbol_short!("sub_cncl"), creator), subscriber);
    }

    /// Returns the subscription between `subscriber` and `creator`, if it exists.
    pub fn get_subscription(
        env: Env,
        subscriber: Address,
        creator: Address,
    ) -> Option<Subscription> {
        env.storage()
            .persistent()
            .get(&DataKey::Subscription(subscriber, creator))
    }

    /// Like `tip`, but stores an optional on-chain message and metadata.
    ///
    /// `message` is limited to 200 Unicode scalar values (character count, not
    /// byte count) so that emoji and multi-byte characters are treated fairly.
    /// Panics with `TipJarError::MessageTooLong` when the limit is exceeded.
    ///
    /// Metadata is stored in persistent storage under `TipHistory(creator, index)`
    /// and the per-creator counter `TipCount(creator)` is incremented.
    ///
    /// Emits `("tip_msg", creator)` with data `(sender, amount, message)`.
    pub fn tip_with_message(
        env: Env,
        sender: Address,
        creator: Address,
        token: Address,
        amount: i128,
        message: Option<String>,
    ) -> u64 {
        Self::require_not_paused(&env);
        Self::check_circuit_breaker(&env, amount);
        sender.require_auth();

        // Validate message length by character count (not bytes) to support emoji.
        if let Some(ref msg) = message {
            // Soroban String stores raw bytes; convert to a &str slice for char counting.
            let bytes = msg.to_string();
            let char_count = bytes.chars().count();
            if char_count > 200 {
                panic_with_error!(&env, TipJarError::MessageTooLong);
            }
        }

        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        token::Client::new(&env, &token).transfer(
            &sender,
            &env.current_contract_address(),
            &amount,
        );

        let fee_bp: u32 = env
            .storage()
            .instance()
            .get(&DataKey::Fee(FeeKey::BasisPoints))
            .unwrap_or(0);
        let fee: i128 = (amount * fee_bp as i128) / 10_000;

        // --- Insurance Premium Calculation ---
        let ins_enabled: bool = env
            .storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Enabled))
            .unwrap_or(true);
        let mut ins_premium: i128 = 0;
        if ins_enabled {
            if let Some(config) = env
                .storage()
                .instance()
                .get::<DataKey, InsurancePoolConfig>(&DataKey::Insurance(InsuranceKey::Cfg))
            {
                ins_premium = (amount * config.tip_premium_bps as i128) / 10_000;
            }
        }

        let creator_amount = amount
            .checked_sub(fee)
            .and_then(|a| a.checked_sub(ins_premium))
            .unwrap_or(0);

        if fee > 0 {
            let fee_key = DataKey::Fee(FeeKey::Balance(token.clone()));
            let new_fee_bal: i128 = env.storage().instance().get(&fee_key).unwrap_or(0) + fee;
            env.storage().instance().set(&fee_key, &new_fee_bal);
        }

        if ins_premium > 0 {
            let pool_key = DataKey::Insurance(InsuranceKey::Token(token.clone()));
            let mut pool: InsurancePool =
                env.storage()
                    .persistent()
                    .get(&pool_key)
                    .unwrap_or_else(|| InsurancePool {
                        token: token.clone(),
                        total_reserves: 0,
                        total_contributions: 0,
                        total_claims_paid: 0,
                        active_claims: 0,
                        total_claims: 0,
                        last_payout_time: env.ledger().timestamp(),
                    });
            pool.total_reserves += ins_premium;
            env.storage().persistent().set(&pool_key, &pool);
        }

        let bal_key = DataKey::CreatorBalance(creator.clone(), token.clone());
        let existing_bal: i128 = env
            .storage()
            .persistent()
            .get(&bal_key)
            .unwrap_or_else(|| env.storage().instance().get(&bal_key).unwrap_or(0));
        let net_amount = Self::process_repayment(&env, &creator, &token, creator_amount);
        env.storage()
            .persistent()
            .set(&bal_key, &(existing_bal + net_amount));

        let tot_key = DataKey::CreatorTotal(creator.clone(), token.clone());
        let existing_tot: i128 = env
            .storage()
            .persistent()
            .get(&tot_key)
            .unwrap_or_else(|| env.storage().instance().get(&tot_key).unwrap_or(0));
        env.storage()
            .persistent()
            .set(&tot_key, &(existing_tot + amount));

        Self::update_leaderboard_stats(&env, &sender, &creator, amount);

        // Store metadata and increment tip count.
        let count_key = DataKey::Tip(TipKey::Count(creator.clone()));
        let tip_index: u64 = env.storage().persistent().get(&count_key).unwrap_or(0u64);

        let timestamp = env.ledger().timestamp();
        let metadata = TipMetadata {
            sender: sender.clone(),
            amount,
            message: message.clone(),
            timestamp,
        };
        env.storage().persistent().set(
            &DataKey::Tip(TipKey::History(creator.clone(), tip_index)),
            &metadata,
        );
        env.storage().persistent().set(&count_key, &(tip_index + 1));

        env.events().publish(
            (symbol_short!("tip_msg"), creator.clone()),
            (sender, amount, message),
        );

        tip_index
    }

    /// Returns the most recent tips (with metadata) for `creator`, newest first.
    ///
    /// `limit` is capped at 100 to bound storage reads.
    pub fn get_tip_history(env: Env, creator: Address, limit: u32) -> Vec<TipMetadata> {
        let count_key = DataKey::Tip(TipKey::Count(creator.clone()));
        let total: u64 = env.storage().persistent().get(&count_key).unwrap_or(0u64);

        let cap = if limit > 100 { 100 } else { limit } as u64;
        let mut result = Vec::new(&env);

        if total == 0 {
            return result;
        }

        // Iterate from newest (total-1) down to oldest, up to `cap` entries.
        let mut idx = total;
        let mut fetched: u64 = 0;
        while idx > 0 && fetched < cap {
            idx -= 1;
            if let Some(meta) = env
                .storage()
                .persistent()
                .get::<_, TipMetadata>(&DataKey::Tip(TipKey::History(creator.clone(), idx)))
            {
                result.push_back(meta);
                fetched += 1;
            }
        }

        result
    }

    /// Splits a single tip among multiple recipients proportionally.
    ///
    /// `recipients` must contain 2–10 entries whose `percentage` values (basis
    /// points) sum to exactly 10 000.  The last recipient absorbs any rounding
    /// remainder so the full `amount` is always distributed.
    ///
    /// Emits `("tip_splt", creator)` with data `(sender, recipient_amount, percentage)`
    /// for every recipient.
    pub fn tip_split(
        env: Env,
        sender: Address,
        token: Address,
        recipients: Vec<TipRecipient>,
        amount: i128,
    ) {
        Self::require_not_paused(&env);
        Self::check_circuit_breaker(&env, amount);
        sender.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        let count = recipients.len();
        if count < 2 || count > 10 {
            panic_with_error!(&env, FeatureError::InvalidRecipientCount);
        }
        let mut total_pct: u32 = 0;
        for r in recipients.iter() {
            if r.percentage == 0 {
                panic_with_error!(&env, FeatureError::InvalidPercentage);
            }
            total_pct += r.percentage;
        }
        if total_pct != 10_000 {
            panic_with_error!(&env, FeatureError::InvalidPercentageSum);
        }

        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        token::Client::new(&env, &token).transfer(
            &sender,
            &env.current_contract_address(),
            &amount,
        );

        let last_idx = count - 1;
        let mut distributed: i128 = 0;
        for (i, r) in recipients.iter().enumerate() {
            let share = if i == last_idx as usize {
                amount - distributed
            } else {
                (amount * r.percentage as i128) / 10_000
            };

            let bal_key = DataKey::CreatorBalance(r.creator.clone(), token.clone());
            let bal: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
            let net_amount = Self::process_repayment(&env, &r.creator, &token, share);
            env.storage()
                .persistent()
                .set(&bal_key, &(bal + net_amount));

            let tot_key = DataKey::CreatorTotal(r.creator.clone(), token.clone());
            let tot: i128 = env.storage().persistent().get(&tot_key).unwrap_or(0);
            env.storage().persistent().set(&tot_key, &(tot + share));

            distributed += share;

            env.events().publish(
                (symbol_short!("tip_splt"), r.creator.clone()),
                (sender.clone(), share, r.percentage),
            );
        }
    }

    // ── RBAC ─────────────────────────────────────────────────────────────────

    /// Grants `role` to `user`. Caller must be the stored admin.
    pub fn grant_role(env: Env, admin: Address, user: Address, role: Role) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        env.storage()
            .persistent()
            .set(&DataKey::Role(RoleKey::User(user.clone())), &role);
        let mut members: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Role(RoleKey::Members(role.clone())))
            .unwrap_or_else(|| Vec::new(&env));
        if !members.contains(&user) {
            members.push_back(user.clone());
            env.storage()
                .persistent()
                .set(&DataKey::Role(RoleKey::Members(role.clone())), &members);
        }
        env.events()
            .publish((symbol_short!("role_grt"),), (user, role));
    }

    /// Revokes any role from `user`. Caller must be the stored admin.
    pub fn revoke_role(env: Env, admin: Address, user: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        if let Some(role) = env
            .storage()
            .persistent()
            .get::<DataKey, Role>(&DataKey::Role(RoleKey::User(user.clone())))
        {
            env.storage()
                .persistent()
                .remove(&DataKey::Role(RoleKey::User(user.clone())));
            let mut members: Vec<Address> = env
                .storage()
                .persistent()
                .get(&DataKey::Role(RoleKey::Members(role.clone())))
                .unwrap_or_else(|| Vec::new(&env));
            members.retain(|a| a != user);
            env.storage()
                .persistent()
                .set(&DataKey::Role(RoleKey::Members(role.clone())), &members);
            env.events()
                .publish((symbol_short!("role_rev"),), (user, role));
        }
    }

    /// Returns `true` if `user` holds `role`.
    pub fn has_role(env: Env, user: Address, role: Role) -> bool {
        env.storage()
            .persistent()
            .get::<DataKey, Role>(&DataKey::Role(RoleKey::User(user.clone())))
            .map(|r| r == role)
            .unwrap_or(false)
    }

    /// Internal helper — panics with `Unauthorized` if `user` does not hold `role`.
    #[allow(dead_code)]
    fn require_role(env: &Env, user: &Address, role: Role) {
        let stored: Option<Role> = env
            .storage()
            .persistent()
            .get(&DataKey::Role(RoleKey::User(user.clone())));
        if stored != Some(role) {
            panic_with_error!(env, TipJarError::Unauthorized);
        }
    }

    /// Returns all addresses that currently hold `role`.
    pub fn get_role_members(env: Env, role: Role) -> Vec<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::Role(RoleKey::Members(role)))
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ── time-locked tips ──────────────────────────────────────────────────────

    /// Creates a time-locked tip for a specific `token`. Tokens are transferred
    /// immediately into escrow but can only be withdrawn by `creator` after `unlock_time`.
    ///
    /// Returns the lock ID.
    /// Emits `("lock", creator)` with data `(sender, amount, unlock_time, lock_id)`.
    pub fn tip_time_locked(
        env: Env,
        sender: Address,
        creator: Address,
        token: Address,
        amount: i128,
        unlock_time: u64,
    ) -> u64 {
        Self::require_not_paused(&env);
        Self::check_circuit_breaker(&env, amount);
        sender.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        if unlock_time <= env.ledger().timestamp() {
            panic_with_error!(&env, TipJarError::InvalidUnlockTime);
        }

        // State updates before external call (CEI).
        let lock_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::NextLockId)
            .unwrap_or(0);
        let created_at = env.ledger().timestamp();
        let refund_window: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RefundWindow)
            .unwrap_or(0);
        let time_lock = TimeLock {
            sender: sender.clone(),
            creator: creator.clone(),
            token: token.clone(),
            amount,
            unlock_time,
            created_at,
            expires_at: created_at.saturating_add(refund_window),
            cancelled: false,
        };
        env.storage()
            .persistent()
            .set(&DataKey::TimeLock(lock_id), &time_lock);
        env.storage()
            .persistent()
            .set(&DataKey::NextLockId, &(lock_id + 1));

        let mut creator_locks: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::CreatorLocks(creator.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        creator_locks.push_back(lock_id);
        env.storage()
            .persistent()
            .set(&DataKey::CreatorLocks(creator.clone()), &creator_locks);

        Self::add_active_time_lock(&env, lock_id);

        // External call last.
        token::Client::new(&env, &token).transfer(
            &sender,
            &env.current_contract_address(),
            &amount,
        );

        env.events().publish(
            (symbol_short!("lock"), creator),
            (sender, amount, unlock_time, lock_id),
        );
        lock_id
    }

    /// Convenience wrapper matching the public `tip_locked` API.
    pub fn tip_locked(
        env: Env,
        sender: Address,
        creator: Address,
        token: Address,
        amount: i128,
        unlock_time: u64,
    ) -> u64 {
        Self::tip_time_locked(env, sender, creator, token, amount, unlock_time)
    }

    /// Withdraws a time-locked tip after its unlock time. Only `creator` may call.
    ///
    /// Emits `("unlock", creator)` with data `(amount, lock_id)`.
    pub fn withdraw_time_locked(env: Env, creator: Address, token: Address, lock_id: u64) {
        Self::require_not_paused(&env);
        creator.require_auth();

        let time_lock: TimeLock = env
            .storage()
            .persistent()
            .get(&DataKey::TimeLock(lock_id))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::LockNotFound));

        if time_lock.creator != creator {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        if time_lock.cancelled {
            panic_with_error!(&env, FeatureError::LockCancelled);
        }
        if env.ledger().timestamp() < time_lock.unlock_time {
            panic_with_error!(&env, FeatureError::NotUnlocked);
        }

        // State update before external call (CEI).
        env.storage()
            .persistent()
            .remove(&DataKey::TimeLock(lock_id));
        Self::remove_active_time_lock(&env, lock_id);

        token::Client::new(&env, &token).transfer(
            &env.current_contract_address(),
            &creator,
            &time_lock.amount,
        );

        env.events().publish(
            (symbol_short!("unlock"), creator),
            (time_lock.amount, lock_id),
        );
    }

    /// Convenience wrapper matching the public `withdraw_locked` API.
    pub fn withdraw_locked(env: Env, creator: Address, token: Address, lock_id: u64) {
        Self::withdraw_time_locked(env, creator, token, lock_id)
    }

    /// Cancels a time-locked tip and refunds the sender. Only the original sender may call.
    ///
    /// Emits `("lk_cncl", sender)` with data `(amount, lock_id)`.
    pub fn cancel_time_lock(env: Env, sender: Address, token: Address, lock_id: u64) {
        Self::require_not_paused(&env);
        sender.require_auth();

        let mut time_lock: TimeLock = env
            .storage()
            .persistent()
            .get(&DataKey::TimeLock(lock_id))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::LockNotFound));

        if time_lock.sender != sender {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        if time_lock.cancelled {
            panic_with_error!(&env, FeatureError::LockCancelled);
        }

        // State update before external call (CEI).
        time_lock.cancelled = true;
        env.storage()
            .persistent()
            .set(&DataKey::TimeLock(lock_id), &time_lock);
        Self::remove_active_time_lock(&env, lock_id);

        token::Client::new(&env, &token).transfer(
            &env.current_contract_address(),
            &sender,
            &time_lock.amount,
        );

        env.events().publish(
            (symbol_short!("lk_cncl"), sender),
            (time_lock.amount, lock_id),
        );
    }

    /// Convenience wrapper matching the public `cancel_locked` API.
    pub fn cancel_locked(env: Env, sender: Address, token: Address, lock_id: u64) {
        Self::cancel_time_lock(env, sender, token, lock_id)
    }

    /// Returns all time-lock records for `creator`.
    pub fn get_time_locks(env: Env, creator: Address) -> Vec<TimeLock> {
        let lock_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::CreatorLocks(creator))
            .unwrap_or_else(|| Vec::new(&env));
        let mut locks = Vec::new(&env);
        for lock_id in lock_ids.iter() {
            if let Some(lock) = env
                .storage()
                .persistent()
                .get::<DataKey, TimeLock>(&DataKey::TimeLock(lock_id))
            {
                locks.push_back(lock);
            }
        }
        locks
    }

    /// Returns a single active locked tip for `creator`.
    pub fn get_locked_tip(env: Env, creator: Address, lock_id: u64) -> LockedTip {
        let time_lock: TimeLock = env
            .storage()
            .persistent()
            .get(&DataKey::TimeLock(lock_id))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::LockNotFound));

        if time_lock.creator != creator {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        LockedTip {
            sender: time_lock.sender.clone(),
            creator: time_lock.creator.clone(),
            token: time_lock.token.clone(),
            amount: time_lock.amount,
            unlock_timestamp: time_lock.unlock_time,
        }
    }

    /// Returns the refund window used to compute tip expiry.
    pub fn get_refund_window(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::RefundWindow)
            .unwrap_or(0)
    }

    /// Updates the refund window used by time-locked tips.
    /// Admin only.
    pub fn set_refund_window(env: Env, admin: Address, refund_window_seconds: u64) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        env.storage()
            .instance()
            .set(&DataKey::RefundWindow, &refund_window_seconds);
        env.events()
            .publish((symbol_short!("ref_wind"),), refund_window_seconds);
    }

    /// Returns all expired time-locked tips whose refund window has passed.
    fn get_expired_time_lock_ids(env: &Env, current_time: u64) -> Vec<u64> {
        let lock_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveTimeLocks)
            .unwrap_or_else(|| Vec::new(env));
        let mut expired = Vec::new(env);
        for lock_id in lock_ids.iter() {
            if let Some(lock) = env
                .storage()
                .persistent()
                .get::<DataKey, TimeLock>(&DataKey::TimeLock(lock_id))
            {
                if !lock.cancelled && lock.expires_at <= current_time {
                    expired.push_back(lock_id);
                }
            }
        }
        expired
    }

    /// Processes all expired time-locked tips and refunds their senders.
    /// Returns the number of refunded tips.
    pub fn get_expired_time_locks(env: Env) -> Vec<TipWithExpiry> {
        let current_time = env.ledger().timestamp();
        let expired_ids = Self::get_expired_time_lock_ids(&env, current_time);
        let mut result = Vec::new(&env);
        for lock_id in expired_ids.iter() {
            if let Some(lock) = env
                .storage()
                .persistent()
                .get::<DataKey, TimeLock>(&DataKey::TimeLock(lock_id))
            {
                result.push_back(TipWithExpiry {
                    tipper: lock.sender.clone(),
                    creator: lock.creator.clone(),
                    amount: lock.amount,
                    created_at: lock.created_at,
                    expires_at: lock.expires_at,
                    claimed: false,
                });
            }
        }
        result
    }

    pub fn process_expired_tips(env: Env) -> u32 {
        let current_time = env.ledger().timestamp();
        let expired_locks = Self::get_expired_time_lock_ids(&env, current_time);
        let mut refunded_count = 0u32;
        for lock_id in expired_locks.iter() {
            if let Some(time_lock) = env
                .storage()
                .persistent()
                .get::<DataKey, TimeLock>(&DataKey::TimeLock(lock_id))
            {
                if !time_lock.cancelled {
                    Self::refund_time_lock(&env, lock_id, &time_lock);
                    refunded_count += 1;
                }
            }
        }
        refunded_count
    }

    fn add_active_time_lock(env: &Env, lock_id: u64) {
        let mut active: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveTimeLocks)
            .unwrap_or_else(|| Vec::new(env));
        if !active.contains(&lock_id) {
            active.push_back(lock_id);
        }
        env.storage()
            .persistent()
            .set(&DataKey::ActiveTimeLocks, &active);
    }

    fn remove_active_time_lock(env: &Env, lock_id: u64) {
        let active: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveTimeLocks)
            .unwrap_or_else(|| Vec::new(env));
        let mut remaining = Vec::new(env);
        for id in active.iter() {
            if id != lock_id {
                remaining.push_back(id);
            }
        }
        env.storage()
            .persistent()
            .set(&DataKey::ActiveTimeLocks, &remaining);
    }

    fn refund_time_lock(env: &Env, lock_id: u64, time_lock: &TimeLock) {
        env.storage()
            .persistent()
            .remove(&DataKey::TimeLock(lock_id));
        Self::remove_active_time_lock(env, lock_id);
        token::Client::new(&env, &time_lock.token).transfer(
            &env.current_contract_address(),
            &time_lock.sender,
            &time_lock.amount,
        );
        env.events().publish(
            (symbol_short!("tip_exp"), time_lock.creator.clone()),
            (
                time_lock.sender.clone(),
                time_lock.amount,
                time_lock.expires_at,
                lock_id,
            ),
        );
    }

    // ── withdrawal limits ─────────────────────────────────────────────────────

    /// Checks cooldown and daily limit for `creator`, then updates state.
    ///
    /// Panics with `WithdrawalCooldown` or `DailyLimitExceeded` on violation.
    fn check_and_update_withdrawal_limits(env: &Env, creator: &Address, amount: i128) {
        const DAY_SECS: u64 = 86_400;

        // Resolve per-creator config, falling back to platform default.
        let mut limits: WithdrawalLimits = env
            .storage()
            .persistent()
            .get(&DataKey::Limit(LimitKey::Withdrawal(creator.clone())))
            .or_else(|| {
                env.storage()
                    .instance()
                    .get(&DataKey::Limit(LimitKey::Default))
            })
            .unwrap_or(WithdrawalLimits {
                daily_limit: 0,
                cooldown_seconds: 0,
                last_withdrawal: 0,
                withdrawn_today: 0,
                day_start: 0,
            });

        let now = env.ledger().timestamp();

        // Cooldown check.
        if limits.cooldown_seconds > 0 && limits.last_withdrawal > 0 {
            if now < limits.last_withdrawal + limits.cooldown_seconds {
                panic_with_error!(env, FeatureError::WithdrawalCooldown);
            }
        }

        // Daily window reset.
        if now >= limits.day_start + DAY_SECS {
            limits.withdrawn_today = 0;
            limits.day_start = now;
        }

        // Daily limit check (0 = unlimited).
        if limits.daily_limit > 0 {
            if limits.withdrawn_today + amount > limits.daily_limit {
                panic_with_error!(env, FeatureError::DailyLimitExceeded);
            }
        }

        limits.withdrawn_today += amount;
        limits.last_withdrawal = now;
        env.storage().persistent().set(
            &DataKey::Limit(LimitKey::Withdrawal(creator.clone())),
            &limits,
        );
    }

    /// Sets per-creator withdrawal limits. Admin only.
    ///
    /// Pass `daily_limit = 0` for unlimited; `cooldown_seconds = 0` for no cooldown.
    /// Emits `("wl_set", creator)` with data `(daily_limit, cooldown_seconds)`.
    pub fn set_withdrawal_limits(
        env: Env,
        admin: Address,
        creator: Address,
        daily_limit: i128,
        cooldown_seconds: u64,
    ) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        let existing: WithdrawalLimits = env
            .storage()
            .persistent()
            .get(&DataKey::Limit(LimitKey::Withdrawal(creator.clone())))
            .unwrap_or(WithdrawalLimits {
                daily_limit: 0,
                cooldown_seconds: 0,
                last_withdrawal: 0,
                withdrawn_today: 0,
                day_start: 0,
            });

        let limits = WithdrawalLimits {
            daily_limit,
            cooldown_seconds,
            last_withdrawal: existing.last_withdrawal,
            withdrawn_today: existing.withdrawn_today,
            day_start: existing.day_start,
        };
        env.storage().persistent().set(
            &DataKey::Limit(LimitKey::Withdrawal(creator.clone())),
            &limits,
        );

        env.events().publish(
            (symbol_short!("wl_set"), creator),
            (daily_limit, cooldown_seconds),
        );
    }

    /// Sets platform-wide default withdrawal limits applied to creators without
    /// a per-creator config. Admin only.
    ///
    /// Emits `("wl_def",)` with data `(daily_limit, cooldown_seconds)`.
    pub fn set_default_withdrawal_limits(
        env: Env,
        admin: Address,
        daily_limit: i128,
        cooldown_seconds: u64,
    ) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        let defaults = WithdrawalLimits {
            daily_limit,
            cooldown_seconds,
            last_withdrawal: 0,
            withdrawn_today: 0,
            day_start: 0,
        };
        env.storage()
            .instance()
            .set(&DataKey::Limit(LimitKey::Default), &defaults);

        env.events()
            .publish((symbol_short!("wl_def"),), (daily_limit, cooldown_seconds));
    }

    /// Emergency withdrawal that bypasses limits. Admin only.
    ///
    /// Transfers the full escrowed balance for `creator` in `token` directly,
    /// skipping cooldown and daily-limit checks.
    /// Emits `("wl_emrg", creator)` with data `amount`.
    pub fn emergency_withdraw(env: Env, admin: Address, creator: Address, token: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        let bal_key = DataKey::CreatorBalance(creator.clone(), token.clone());
        let amount: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
        if amount == 0 {
            panic_with_error!(&env, TipJarError::NothingToWithdraw);
        }

        env.storage().persistent().set(&bal_key, &0i128);
        token::Client::new(&env, &token).transfer(
            &env.current_contract_address(),
            &creator,
            &amount,
        );

        env.events()
            .publish((symbol_short!("wl_emrg"), creator), amount);
    }

    /// Returns the withdrawal limits for `creator`, or the platform defaults if
    /// no per-creator config exists.
    pub fn get_withdrawal_limits(env: Env, creator: Address) -> WithdrawalLimits {
        env.storage()
            .persistent()
            .get(&DataKey::Limit(LimitKey::Withdrawal(creator)))
            .or_else(|| {
                env.storage()
                    .instance()
                    .get(&DataKey::Limit(LimitKey::Default))
            })
            .unwrap_or(WithdrawalLimits {
                daily_limit: 0,
                cooldown_seconds: 0,
                last_withdrawal: 0,
                withdrawn_today: 0,
                day_start: 0,
            })
    }

    // ── multi-signature withdrawals ───────────────────────────────────────────

    /// Sets the multi-sig configuration. Admin only.
    ///
    /// `threshold` — amounts strictly above this require multi-sig (0 = all withdrawals).
    /// Emits `("ms_cfg",)` with data `(threshold, required_approvals, expiry_seconds)`.
    pub fn set_multisig_config(
        env: Env,
        admin: Address,
        threshold: i128,
        required_approvals: u32,
        expiry_seconds: u64,
        signers: Vec<Address>,
    ) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        if required_approvals == 0 || required_approvals as u32 > signers.len() {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        let cfg = MultiSigConfig {
            threshold,
            required_approvals,
            expiry_seconds,
            signers,
        };
        env.storage()
            .instance()
            .set(&DataKey::MultiSig(MultiSigKey::Config), &cfg);
        env.events().publish(
            (symbol_short!("ms_cfg"),),
            (threshold, required_approvals, expiry_seconds),
        );
    }

    /// Creates a multi-sig withdrawal request for `amount` of `token`.
    ///
    /// If `amount` is at or below the configured threshold the withdrawal is
    /// processed immediately (no multi-sig needed) and returns `0`.
    /// Otherwise a pending request is created and its ID is returned.
    ///
    /// Emits `("ms_req", creator)` with data `(request_id, amount, expires_at)`.
    pub fn request_multisig_withdrawal(
        env: Env,
        creator: Address,
        token: Address,
        amount: i128,
    ) -> u64 {
        Self::require_not_paused(&env);
        creator.require_auth();

        let bal_key = DataKey::CreatorBalance(creator.clone(), token.clone());
        let balance: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
        if balance == 0 || amount <= 0 || amount > balance {
            panic_with_error!(&env, TipJarError::NothingToWithdraw);
        }

        let cfg: MultiSigConfig = env
            .storage()
            .instance()
            .get(&DataKey::MultiSig(MultiSigKey::Config))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::MultiSigNotConfigured));

        // Below-or-at threshold: process immediately.
        if cfg.threshold > 0 && amount <= cfg.threshold {
            Self::check_and_update_withdrawal_limits(&env, &creator, amount);
            env.storage()
                .persistent()
                .set(&bal_key, &(balance - amount));
            token::Client::new(&env, &token).transfer(
                &env.current_contract_address(),
                &creator,
                &amount,
            );
            events::emit_withdraw_event(&env, &creator, amount, &token);
            return 0;
        }

        // Above threshold: create pending request.
        let request_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::MultiSig(MultiSigKey::Ctr))
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::MultiSig(MultiSigKey::Ctr), &(request_id + 1));

        let expires_at = env.ledger().timestamp() + cfg.expiry_seconds;
        let request = MultiSigWithdrawal {
            request_id,
            creator: creator.clone(),
            token,
            amount,
            approvals: Vec::new(&env),
            required_approvals: cfg.required_approvals,
            expires_at,
            executed: false,
            cancelled: false,
        };
        env.storage().persistent().set(
            &DataKey::MultiSig(MultiSigKey::Request(request_id)),
            &request,
        );

        env.events().publish(
            (symbol_short!("ms_req"), creator),
            (request_id, amount, expires_at),
        );
        request_id
    }

    /// Approves a pending multi-sig withdrawal request.
    ///
    /// Once `required_approvals` is reached the withdrawal is executed automatically.
    /// Emits `("ms_apr", approver)` with data `request_id`.
    /// Emits `("ms_exe", creator)` with data `(request_id, amount)` on execution.
    pub fn approve_withdrawal(env: Env, approver: Address, request_id: u64) {
        Self::require_not_paused(&env);
        approver.require_auth();

        let cfg: MultiSigConfig = env
            .storage()
            .instance()
            .get(&DataKey::MultiSig(MultiSigKey::Config))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::MultiSigNotConfigured));

        if !cfg.signers.contains(&approver) {
            panic_with_error!(&env, FeatureError::NotASigner);
        }

        let mut request: MultiSigWithdrawal = env
            .storage()
            .persistent()
            .get(&DataKey::MultiSig(MultiSigKey::Request(request_id)))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::MsigReqNotFound));

        if request.executed || request.cancelled {
            panic_with_error!(&env, FeatureError::MultiSigReqClosed);
        }
        if env.ledger().timestamp() > request.expires_at {
            panic_with_error!(&env, FeatureError::MultiSigReqExpired);
        }
        if request.approvals.contains(&approver) {
            panic_with_error!(&env, FeatureError::AlreadyApproved);
        }

        request.approvals.push_back(approver.clone());
        env.events()
            .publish((symbol_short!("ms_apr"), approver), request_id);

        if request.approvals.len() >= request.required_approvals {
            // Execute withdrawal.
            let bal_key = DataKey::CreatorBalance(request.creator.clone(), request.token.clone());
            let balance: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
            if balance < request.amount {
                panic_with_error!(&env, TipJarError::InsufficientBalance);
            }
            env.storage()
                .persistent()
                .set(&bal_key, &(balance - request.amount));
            request.executed = true;
            env.storage().persistent().set(
                &DataKey::MultiSig(MultiSigKey::Request(request_id)),
                &request,
            );

            token::Client::new(&env, &request.token).transfer(
                &env.current_contract_address(),
                &request.creator,
                &request.amount,
            );
            env.events().publish(
                (symbol_short!("ms_exe"), request.creator.clone()),
                (request_id, request.amount),
            );
        } else {
            env.storage().persistent().set(
                &DataKey::MultiSig(MultiSigKey::Request(request_id)),
                &request,
            );
        }
    }

    /// Cancels a pending multi-sig withdrawal request.
    ///
    /// Only the original creator or admin may cancel.
    /// Emits `("ms_cncl", creator)` with data `request_id`.
    pub fn cancel_multisig_withdrawal(env: Env, caller: Address, request_id: u64) {
        caller.require_auth();

        let mut request: MultiSigWithdrawal = env
            .storage()
            .persistent()
            .get(&DataKey::MultiSig(MultiSigKey::Request(request_id)))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::MsigReqNotFound));

        if request.executed || request.cancelled {
            panic_with_error!(&env, FeatureError::MultiSigReqClosed);
        }

        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if caller != request.creator && caller != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        request.cancelled = true;
        env.storage().persistent().set(
            &DataKey::MultiSig(MultiSigKey::Request(request_id)),
            &request,
        );
        env.events()
            .publish((symbol_short!("ms_cncl"), request.creator), request_id);
    }

    /// Returns a multi-sig withdrawal request by ID.
    pub fn get_multisig_request(env: Env, request_id: u64) -> MultiSigWithdrawal {
        env.storage()
            .persistent()
            .get(&DataKey::MultiSig(MultiSigKey::Request(request_id)))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::MsigReqNotFound))
    }

    /// Returns the current multi-sig configuration.
    pub fn get_multisig_config(env: Env) -> MultiSigConfig {
        env.storage()
            .instance()
            .get(&DataKey::MultiSig(MultiSigKey::Config))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::MultiSigNotConfigured))
    }

    // ── upgrade / migration ───────────────────────────────────────────────────

    /// Runs any data migration needed after an upgrade. Admin only.
    ///
    /// Match on the current version to apply version-specific migrations.
    pub fn migrate(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        let version = upgrade::get_version(&env);
        match version {
            // v1 → v2: no data migration required in this example.
            _ => {}
        }
    }

    // ── dispute resolution ────────────────────────────────────────────────────

    /// Creates a dispute for a tip. Only the tipper or creator can initiate.
    ///
    /// Emits `("dispute_created",)` with data `(dispute_id, tip_id, initiator)`.
    pub fn create_dispute(env: Env, tip_id: u64, initiator: Address, reason: String) -> u64 {
        Self::require_not_paused(&env);
        initiator.require_auth();

        let dispute_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::Dispute(DisputeKey::Ctr))
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::Dispute(DisputeKey::Ctr), &(dispute_id + 1));

        let created_at = env.ledger().timestamp();
        let dispute = dispute::Dispute {
            id: dispute_id,
            tip_id,
            initiator: initiator.clone(),
            reason,
            status: dispute::DisputeStatus::Open,
            arbitrator: None,
            resolution: None,
            created_at,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Dispute(DisputeKey::Record(dispute_id)), &dispute);

        let mut creator_disputes: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::Dispute(DisputeKey::CreatorList(
                initiator.clone(),
            )))
            .unwrap_or_else(|| Vec::new(&env));
        creator_disputes.push_back(dispute_id);
        env.storage().persistent().set(
            &DataKey::Dispute(DisputeKey::CreatorList(initiator.clone())),
            &creator_disputes,
        );

        env.events().publish(
            (symbol_short!("disp_crt"),),
            (dispute_id, tip_id, initiator),
        );

        dispute_id
    }

    /// Assigns an arbitrator to a dispute. Admin only.
    ///
    /// Emits `("dispute_assigned",)` with data `(dispute_id, arbitrator)`.
    pub fn assign_arbitrator(env: Env, admin: Address, dispute_id: u64, arbitrator: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        let mut dispute: dispute::Dispute = env
            .storage()
            .persistent()
            .get(&DataKey::Dispute(DisputeKey::Record(dispute_id)))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::DisputeNotFound));

        dispute.arbitrator = Some(arbitrator.clone());
        dispute.status = dispute::DisputeStatus::UnderReview;
        env.storage()
            .persistent()
            .set(&DataKey::Dispute(DisputeKey::Record(dispute_id)), &dispute);

        env.events()
            .publish((symbol_short!("disp_asgn"),), (dispute_id, arbitrator));
    }

    /// Submits evidence for a dispute.
    ///
    /// Emits `("evidence_submitted",)` with data `(dispute_id, submitter)`.
    pub fn submit_evidence(env: Env, dispute_id: u64, submitter: Address, evidence: String) {
        Self::require_not_paused(&env);
        submitter.require_auth();

        let dispute: dispute::Dispute = env
            .storage()
            .persistent()
            .get(&DataKey::Dispute(DisputeKey::Record(dispute_id)))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::DisputeNotFound));

        if dispute.status != dispute::DisputeStatus::Open
            && dispute.status != dispute::DisputeStatus::UnderReview
        {
            panic_with_error!(&env, FeatureError::DisputeNotOpen);
        }

        let evidence_idx: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Dispute(DisputeKey::EvidenceCtr(dispute_id)))
            .unwrap_or(0);

        let submitted_at = env.ledger().timestamp();
        let evidence_record = dispute::DisputeEvidence {
            dispute_id,
            submitter: submitter.clone(),
            evidence,
            submitted_at,
        };

        env.storage().persistent().set(
            &DataKey::Dispute(DisputeKey::Evidence(dispute_id, evidence_idx)),
            &evidence_record,
        );
        env.storage().persistent().set(
            &DataKey::Dispute(DisputeKey::EvidenceCtr(dispute_id)),
            &(evidence_idx + 1),
        );

        env.events()
            .publish((symbol_short!("evid_sub"),), (dispute_id, submitter));
    }

    /// Resolves a dispute. Only the arbitrator can resolve.
    ///
    /// Emits `("dispute_resolved",)` with data `(dispute_id, resolution)`.
    pub fn resolve_dispute(
        env: Env,
        dispute_id: u64,
        arbitrator: Address,
        resolution: String,
        approved: bool,
    ) {
        Self::require_not_paused(&env);
        arbitrator.require_auth();

        let mut dispute: dispute::Dispute = env
            .storage()
            .persistent()
            .get(&DataKey::Dispute(DisputeKey::Record(dispute_id)))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::DisputeNotFound));

        if dispute.arbitrator != Some(arbitrator.clone()) {
            panic_with_error!(&env, InsuranceError::DisputeUnauthorized);
        }

        dispute.resolution = Some(resolution.clone());
        dispute.status = if approved {
            dispute::DisputeStatus::Resolved
        } else {
            dispute::DisputeStatus::Rejected
        };

        env.storage()
            .persistent()
            .set(&DataKey::Dispute(DisputeKey::Record(dispute_id)), &dispute);

        env.events()
            .publish((symbol_short!("disp_res"),), (dispute_id, resolution));
    }

    /// Returns a dispute by ID.
    pub fn get_dispute(env: Env, dispute_id: u64) -> dispute::Dispute {
        env.storage()
            .persistent()
            .get(&DataKey::Dispute(DisputeKey::Record(dispute_id)))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::DisputeNotFound))
    }

    /// Returns all disputes for a creator.
    pub fn get_creator_disputes(env: Env, creator: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::Dispute(DisputeKey::CreatorList(creator)))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Returns evidence for a dispute.
    pub fn get_dispute_evidence(
        env: Env,
        dispute_id: u64,
        evidence_idx: u64,
    ) -> dispute::DisputeEvidence {
        env.storage()
            .persistent()
            .get(&DataKey::Dispute(DisputeKey::Evidence(
                dispute_id,
                evidence_idx,
            )))
            .unwrap_or_else(|| panic_with_error!(&env, FeatureError::DisputeNotFound))
    }

    // ── batch tipping ─────────────────────────────────────────────────────────

    /// Sends multiple tips in a single transaction to reduce gas costs.
    ///
    /// `tips` is a vector of (creator, amount) pairs. Returns the number of successful tips.
    /// Emits `("batch_tip",)` with data `(tipper, count, total_amount)`.
    pub fn batch_tip(env: Env, tipper: Address, token: Address, tips: Vec<BatchTip>) -> u32 {
        Self::require_not_paused(&env);
        tipper.require_auth();

        if tips.len() == 0 || tips.len() > 100 {
            panic_with_error!(&env, TipJarError::BatchTooLarge);
        }

        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        let mut total_amount: i128 = 0;
        for tip in tips.iter() {
            if tip.amount <= 0 {
                panic_with_error!(&env, TipJarError::InvalidAmount);
            }
            total_amount = total_amount
                .checked_add(tip.amount)
                .expect("total overflow");
        }

        Self::check_circuit_breaker(&env, total_amount);

        // Transfer all tokens at once
        token::Client::new(&env, &token).transfer(
            &tipper,
            &env.current_contract_address(),
            &total_amount,
        );

        let fee_bp: u32 = env
            .storage()
            .instance()
            .get(&DataKey::Fee(FeeKey::BasisPoints))
            .unwrap_or(0);
        let mut successful_tips: u32 = 0;

        for tip in tips.iter() {
            let fee: i128 = (tip.amount * fee_bp as i128) / 10_000;
            let creator_amount = tip.amount - fee;

            if fee > 0 {
                let fee_key = DataKey::Fee(FeeKey::Balance(token.clone()));
                let new_fee_bal: i128 = env
                    .storage()
                    .instance()
                    .get(&fee_key)
                    .unwrap_or(0)
                    .checked_add(fee)
                    .expect("fee overflow");
                env.storage().instance().set(&fee_key, &new_fee_bal);
            }

            let bal_key = DataKey::CreatorBalance(tip.creator.clone(), token.clone());
            let existing_bal: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
            let net_amount = Self::process_repayment(&env, &tip.creator, &token, creator_amount);
            let new_bal: i128 = existing_bal
                .checked_add(net_amount)
                .expect("balance overflow");
            env.storage().persistent().set(&bal_key, &new_bal);

            let tot_key = DataKey::CreatorTotal(tip.creator.clone(), token.clone());
            let existing_tot: i128 = env.storage().persistent().get(&tot_key).unwrap_or(0);
            let new_tot: i128 = existing_tot
                .checked_add(creator_amount)
                .expect("total overflow");
            env.storage().persistent().set(&tot_key, &new_tot);

            Self::update_leaderboard_stats(&env, &tipper, &tip.creator, creator_amount);
            successful_tips += 1;
        }

        env.events().publish(
            (symbol_short!("batch_tip"),),
            (tipper, successful_tips, total_amount),
        );

        successful_tips
    }

    // ── batch withdraw ────────────────────────────────────────────────────────

    /// Withdraws balances across multiple tokens in a single transaction.
    ///
    /// Each `WithdrawOperation` specifies a `token` and the `amount` to withdraw.
    /// All operations are validated before any transfer is executed (atomic: all-or-nothing).
    /// Batch size is capped at 20 to prevent gas exhaustion.
    ///
    /// Emits `("batch_wdr",)` with data `(creator, count, operations_count)` on success.
    /// Emits `("batch_wdr_op",)` with data `(creator, token, amount, index)` for each operation.
    pub fn batch_withdraw(
        env: Env,
        creator: Address,
        operations: Vec<WithdrawOperation>,
    ) -> Vec<BatchResult> {
        Self::require_not_paused(&env);
        creator.require_auth();

        const MAX_BATCH_SIZE: u32 = 20;
        let op_count = operations.len();

        if op_count == 0 || op_count > MAX_BATCH_SIZE {
            panic_with_error!(&env, TipJarError::BatchSizeExceeded);
        }

        // ── Validation pass (checks before any state change) ─────────────────
        for op in operations.iter() {
            if op.amount <= 0 {
                panic_with_error!(&env, TipJarError::InvalidAmount);
            }

            let bal_key = DataKey::CreatorBalance(creator.clone(), op.token.clone());
            let balance: i128 = env
                .storage()
                .persistent()
                .get(&bal_key)
                .unwrap_or_else(|| env.storage().instance().get(&bal_key).unwrap_or(0));

            if op.amount > balance {
                panic_with_error!(&env, TipJarError::InsufficientBalance);
            }
        }

        // ── Effects pass (state updates before external calls) ────────────────
        for op in operations.iter() {
            let bal_key = DataKey::CreatorBalance(creator.clone(), op.token.clone());
            let balance: i128 = env
                .storage()
                .persistent()
                .get(&bal_key)
                .unwrap_or_else(|| env.storage().instance().get(&bal_key).unwrap_or(0));
            env.storage()
                .persistent()
                .set(&bal_key, &(balance - op.amount));
        }

        // ── Interactions pass (external token transfers) ──────────────────────
        let mut results: Vec<BatchResult> = Vec::new(&env);
        let contract_address = env.current_contract_address();

        for (index, op) in operations.iter().enumerate() {
            token::Client::new(&env, &op.token).transfer(&contract_address, &creator, &op.amount);

            env.events().publish(
                (symbol_short!("btch_wdr"),),
                (creator.clone(), op.token.clone(), op.amount, index as u32),
            );

            results.push_back(BatchResult {
                success: true,
                index: index as u32,
            });
        }

        env.events()
            .publish((symbol_short!("batch_wdr"),), (creator.clone(), op_count));

        results
    }

    /// Sends multiple tips in a single transaction, returning per-operation results.
    ///
    /// Accepts up to 20 `TipOperation` entries. All amounts are validated before any
    /// token transfer occurs (atomic: all-or-nothing). A single transfer covers the
    /// total amount, then balances are distributed to each creator.
    ///
    /// Emits `("btch_tip2",)` with data `(tipper, count, total_amount)` on success.
    /// Emits `("btch_tip_op",)` with data `(tipper, creator, token, amount, index)` per operation.
    pub fn batch_tip_v2(
        env: Env,
        tipper: Address,
        operations: Vec<TipOperation>,
    ) -> Vec<BatchResult> {
        Self::require_not_paused(&env);
        tipper.require_auth();

        const MAX_BATCH_SIZE: u32 = 20;
        let op_count = operations.len();

        if op_count == 0 || op_count > MAX_BATCH_SIZE {
            panic_with_error!(&env, TipJarError::BatchSizeExceeded);
        }

        // ── Validation pass ───────────────────────────────────────────────────
        // Group totals by token so we can do one transfer per token.
        let mut token_totals: Map<Address, i128> = Map::new(&env);

        for op in operations.iter() {
            if op.amount <= 0 {
                panic_with_error!(&env, TipJarError::InvalidAmount);
            }

            let whitelisted: bool = env
                .storage()
                .instance()
                .get(&DataKey::TokenWhitelist(op.token.clone()))
                .unwrap_or(false);
            if !whitelisted {
                panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
            }

            let existing: i128 = token_totals.get(op.token.clone()).unwrap_or(0);
            token_totals.set(
                op.token.clone(),
                existing.checked_add(op.amount).expect("total overflow"),
            );
        }

        // ── Transfer total per token (one transfer per distinct token) ─────────
        let contract_address = env.current_contract_address();
        let fee_bp: u32 = env
            .storage()
            .instance()
            .get(&DataKey::FeeBasisPoints)
            .unwrap_or(0);

        for token_key in token_totals.keys() {
            let total: i128 = token_totals.get(token_key.clone()).unwrap_or(0);
            token::Client::new(&env, &token_key).transfer(&tipper, &contract_address, &total);
        }

        // ── Effects + Interactions pass ───────────────────────────────────────
        let mut results: Vec<BatchResult> = Vec::new(&env);
        let mut grand_total: i128 = 0;

        for (index, op) in operations.iter().enumerate() {
            let fee: i128 = (op.amount * fee_bp as i128) / 10_000;
            let creator_amount = op.amount - fee;

            if fee > 0 {
                let fee_key = DataKey::PlatformFeeBalance(op.token.clone());
                let new_fee_bal: i128 = env
                    .storage()
                    .instance()
                    .get(&fee_key)
                    .unwrap_or(0i128)
                    .checked_add(fee)
                    .expect("fee overflow");
                env.storage().instance().set(&fee_key, &new_fee_bal);
            }

            let bal_key = DataKey::CreatorBalance(op.creator.clone(), op.token.clone());
            let existing_bal: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
            env.storage().persistent().set(
                &bal_key,
                &existing_bal
                    .checked_add(creator_amount)
                    .expect("balance overflow"),
            );

            let tot_key = DataKey::CreatorTotal(op.creator.clone(), op.token.clone());
            let existing_tot: i128 = env.storage().persistent().get(&tot_key).unwrap_or(0);
            env.storage().persistent().set(
                &tot_key,
                &existing_tot
                    .checked_add(creator_amount)
                    .expect("total overflow"),
            );

            Self::update_leaderboard_stats(&env, &tipper, &op.creator, creator_amount);

            env.events().publish(
                (symbol_short!("btp_op"),),
                (
                    tipper.clone(),
                    op.creator.clone(),
                    op.token.clone(),
                    creator_amount,
                    index as u32,
                ),
            );

            results.push_back(BatchResult {
                success: true,
                index: index as u32,
            });

            grand_total = grand_total
                .checked_add(op.amount)
                .expect("grand total overflow");
        }

        env.events().publish(
            (symbol_short!("btch_tip2"),),
            (tipper, op_count, grand_total),
        );

        results
    }

    /// Sends multiple tips in a single transaction with partial failure handling.
    ///
    /// Unlike `batch_tip_v2`, invalid operations (zero/negative amount, unwhitelisted
    /// token) are **skipped** rather than causing the entire batch to revert.
    /// Each operation is transferred individually so only valid ops consume tokens.
    ///
    /// Batch size is capped at 20. Returns per-operation results indicating success
    /// or skip. Updates cumulative `BatchStats` stored under `symbol_short!("bt_stats")`.
    ///
    /// Emits `("btp_multi",)` with data `(tipper, success_count, skipped_count, total_amount)`.
    pub fn batch_tip_multi(
        env: Env,
        tipper: Address,
        operations: Vec<TipOperation>,
    ) -> Vec<BatchResult> {
        Self::require_not_paused(&env);
        tipper.require_auth();

        const MAX_BATCH_SIZE: u32 = 20;
        let op_count = operations.len();

        if op_count == 0 || op_count > MAX_BATCH_SIZE {
            panic_with_error!(&env, TipJarError::BatchSizeExceeded);
        }

        let fee_bp: u32 = env
            .storage()
            .instance()
            .get(&DataKey::FeeBasisPoints)
            .unwrap_or(0);

        let contract_address = env.current_contract_address();
        let mut results: Vec<BatchResult> = Vec::new(&env);
        let mut success_count: u64 = 0;
        let mut skipped_count: u64 = 0;
        let mut grand_total: i128 = 0;

        for (index, op) in operations.iter().enumerate() {
            let idx = index as u32;

            // Validate — skip instead of panic.
            if op.amount <= 0 {
                results.push_back(BatchResult { success: false, index: idx });
                skipped_count += 1;
                continue;
            }
            let whitelisted: bool = env
                .storage()
                .instance()
                .get(&DataKey::TokenWhitelist(op.token.clone()))
                .unwrap_or(false);
            if !whitelisted {
                results.push_back(BatchResult { success: false, index: idx });
                skipped_count += 1;
                continue;
            }

            // Transfer this op's tokens individually (CEI: transfer before state update).
            token::Client::new(&env, &op.token).transfer(
                &tipper,
                &contract_address,
                &op.amount,
            );

            let fee: i128 = (op.amount * fee_bp as i128) / 10_000;
            let creator_amount = op.amount - fee;

            if fee > 0 {
                let fee_key = DataKey::PlatformFeeBalance(op.token.clone());
                let new_fee: i128 = env
                    .storage()
                    .instance()
                    .get(&fee_key)
                    .unwrap_or(0i128)
                    .saturating_add(fee);
                env.storage().instance().set(&fee_key, &new_fee);
            }

            let bal_key = DataKey::CreatorBalance(op.creator.clone(), op.token.clone());
            let existing_bal: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
            env.storage()
                .persistent()
                .set(&bal_key, &existing_bal.saturating_add(creator_amount));

            let tot_key = DataKey::CreatorTotal(op.creator.clone(), op.token.clone());
            let existing_tot: i128 = env.storage().persistent().get(&tot_key).unwrap_or(0);
            env.storage()
                .persistent()
                .set(&tot_key, &existing_tot.saturating_add(creator_amount));

            Self::update_leaderboard_stats(&env, &tipper, &op.creator, creator_amount);

            env.events().publish(
                (symbol_short!("btp_mop"),),
                (tipper.clone(), op.creator.clone(), op.token.clone(), creator_amount, idx),
            );

            results.push_back(BatchResult { success: true, index: idx });
            success_count += 1;
            grand_total = grand_total.saturating_add(op.amount);
        }

        // Update cumulative batch stats.
        let stats_key = symbol_short!("bt_stats");
        let mut stats: BatchStats = env
            .storage()
            .instance()
            .get(&stats_key)
            .unwrap_or(BatchStats {
                total_batches: 0,
                total_tips: 0,
                total_amount: 0,
                total_skipped: 0,
            });
        stats.total_batches += 1;
        stats.total_tips += success_count;
        stats.total_amount = stats.total_amount.saturating_add(grand_total);
        stats.total_skipped += skipped_count;
        env.storage().instance().set(&stats_key, &stats);

        env.events().publish(
            (symbol_short!("btp_multi"),),
            (tipper, success_count as u32, skipped_count as u32, grand_total),
        );

        results
    }

    /// Returns the cumulative batch processing statistics.
    pub fn get_batch_stats(env: Env) -> BatchStats {
        env.storage()
            .instance()
            .get(&symbol_short!("bt_stats"))
            .unwrap_or(BatchStats {
                total_batches: 0,
                total_tips: 0,
                total_amount: 0,
                total_skipped: 0,
            })
    }

    // ── liquidity mining ──────────────────────────────────────────────────────

    /// Creates a new liquidity mining program.
    ///
    /// Transfers `total_rewards` of `reward_token` from `admin` into the contract.
    /// Returns the new program ID.
    ///
    /// * `reward_rate_bps`  — annual reward rate in basis points (e.g. 2000 = 20 %).
    /// * `vesting_cliff`    — seconds before any rewards unlock.
    /// * `vesting_duration` — total vesting window in seconds (>= cliff, <= 4 years).
    /// * `end_time`         — program end timestamp; pass 0 for no end.
    ///
    /// Emits `("lm_create",)` with `(program_id, lp_token, reward_token, total_rewards, rate_bps)`.
    pub fn lm_create_program(
        env: Env,
        admin: Address,
        lp_token: Address,
        reward_token: Address,
        total_rewards: i128,
        reward_rate_bps: u32,
        vesting_cliff: u64,
        vesting_duration: u64,
        end_time: u64,
    ) -> u64 {
        Self::require_not_paused(&env);
        liquidity_mining::create_program(
            &env,
            &admin,
            &lp_token,
            &reward_token,
            total_rewards,
            reward_rate_bps,
            vesting_cliff,
            vesting_duration,
            end_time,
        )
    }

    /// Stakes `amount` LP tokens into a liquidity mining program.
    ///
    /// Emits `("lm_stake",)` with `(provider, program_id, amount, new_total_staked)`.
    pub fn lm_stake(env: Env, provider: Address, program_id: u64, amount: i128) {
        Self::require_not_paused(&env);
        liquidity_mining::stake(&env, &provider, program_id, amount);
    }

    /// Unstakes `amount` LP tokens from a liquidity mining program.
    ///
    /// Accrues pending rewards before reducing the position.
    /// Emits `("lm_unstk",)` with `(provider, program_id, amount, remaining_staked)`.
    pub fn lm_unstake(env: Env, provider: Address, program_id: u64, amount: i128) {
        Self::require_not_paused(&env);
        liquidity_mining::unstake(&env, &provider, program_id, amount);
    }

    /// Claims all vested mining rewards for a provider. Returns the amount claimed.
    ///
    /// Emits `("lm_claim",)` with `(provider, program_id, amount_claimed)`.
    pub fn lm_claim_rewards(env: Env, provider: Address, program_id: u64) -> i128 {
        Self::require_not_paused(&env);
        liquidity_mining::claim_rewards(&env, &provider, program_id)
    }

    /// Applies a boost to a provider's position by locking rewards for `lock_duration` seconds.
    ///
    /// Boost scales linearly from 1× (no lock) to 3× (lock = 1 year).
    /// Only increasing boosts are accepted.
    /// Emits `("lm_boost",)` with `(provider, program_id, new_boost, lock_until)`.
    pub fn lm_apply_boost(env: Env, provider: Address, program_id: u64, lock_duration: u64) {
        Self::require_not_paused(&env);
        liquidity_mining::apply_boost(&env, &provider, program_id, lock_duration);
    }

    /// Deactivates a mining program. Existing positions can still claim vested rewards.
    ///
    /// Emits `("lm_deact",)` with `(program_id,)`.
    pub fn lm_deactivate_program(env: Env, admin: Address, program_id: u64) {
        Self::require_not_paused(&env);
        liquidity_mining::deactivate_program(&env, &admin, program_id);
    }

    /// Returns the vesting schedule info for a provider's position.
    pub fn lm_get_vesting_info(
        env: Env,
        provider: Address,
        program_id: u64,
    ) -> liquidity_mining::VestingInfo {
        liquidity_mining::get_vesting_info(&env, &provider, program_id)
    }

    /// Returns a mining program by ID.
    pub fn lm_get_program(env: Env, program_id: u64) -> liquidity_mining::MiningProgram {
        liquidity_mining::get_program_info(&env, program_id)
    }

    /// Returns a provider's position in a mining program (with accrued rewards).
    pub fn lm_get_position(
        env: Env,
        provider: Address,
        program_id: u64,
    ) -> liquidity_mining::MiningPosition {
        liquidity_mining::get_position_info(&env, &provider, program_id)
    }

    /// Returns all program IDs a provider has participated in.
    pub fn lm_get_provider_programs(env: Env, provider: Address) -> Vec<u64> {
        liquidity_mining::get_provider_program_ids(&env, &provider)
    }

    /// Returns the pending (not yet vested) rewards for a provider in a program.
    pub fn lm_get_pending_rewards(env: Env, provider: Address, program_id: u64) -> i128 {
        liquidity_mining::get_pending_rewards(&env, &provider, program_id)
    }

    // ── bonding curves ────────────────────────────────────────────────────────

    /// Creates a new bonding curve for dynamic tip token pricing.
    ///
    /// * `params`           — curve type, pricing parameters, and fees.
    /// * `initial_reserve`  — optional seed collateral transferred from `creator` (pass 0 for none).
    ///
    /// Returns the new curve ID.
    /// Emits `("bc_create",)` with `(curve_id, creator, tip_token, reserve_token)`.
    pub fn bc_create_curve(
        env: Env,
        creator: Address,
        tip_token: Address,
        reserve_token: Address,
        params: bonding_curve::CurveParams,
        initial_reserve: i128,
    ) -> u64 {
        Self::require_not_paused(&env);
        bonding_curve::create_curve(
            &env,
            &creator,
            &tip_token,
            &reserve_token,
            params,
            initial_reserve,
        )
    }

    /// Buys `token_amount` tip tokens from a bonding curve.
    ///
    /// Collateral is calculated by integrating the price curve.
    /// The transaction reverts if the total cost exceeds `max_collateral`.
    ///
    /// Emits `("bc_buy",)` with `(buyer, curve_id, token_amount, collateral_paid, new_price)`.
    pub fn bc_buy(
        env: Env,
        buyer: Address,
        curve_id: u64,
        token_amount: i128,
        max_collateral: i128,
    ) -> bonding_curve::TradeResult {
        Self::require_not_paused(&env);
        bonding_curve::buy(&env, &buyer, curve_id, token_amount, max_collateral)
    }

    /// Sells `token_amount` tip tokens back to a bonding curve.
    ///
    /// Collateral returned is calculated by integrating the price curve.
    /// The transaction reverts if the net return is below `min_collateral`.
    ///
    /// Emits `("bc_sell",)` with `(seller, curve_id, token_amount, collateral_returned, new_price)`.
    pub fn bc_sell(
        env: Env,
        seller: Address,
        curve_id: u64,
        token_amount: i128,
        min_collateral: i128,
    ) -> bonding_curve::TradeResult {
        Self::require_not_paused(&env);
        bonding_curve::sell(&env, &seller, curve_id, token_amount, min_collateral)
    }

    /// Updates the buy and sell fee parameters of a curve.
    /// Only the curve creator can call this.
    ///
    /// Emits `("bc_fee",)` with `(curve_id, buy_fee_bps, sell_fee_bps)`.
    pub fn bc_update_fees(
        env: Env,
        creator: Address,
        curve_id: u64,
        buy_fee_bps: u32,
        sell_fee_bps: u32,
    ) {
        Self::require_not_paused(&env);
        bonding_curve::update_fees(&env, &creator, curve_id, buy_fee_bps, sell_fee_bps);
    }

    /// Withdraws accumulated fees to the curve creator.
    /// Returns the amount withdrawn.
    ///
    /// Emits `("bc_wfee",)` with `(curve_id, creator, amount)`.
    pub fn bc_withdraw_fees(env: Env, creator: Address, curve_id: u64) -> i128 {
        Self::require_not_paused(&env);
        bonding_curve::withdraw_fees(&env, &creator, curve_id)
    }

    /// Deactivates a bonding curve. Only the creator can call this.
    ///
    /// Emits `("bc_deact",)` with `(curve_id,)`.
    pub fn bc_deactivate(env: Env, creator: Address, curve_id: u64) {
        Self::require_not_paused(&env);
        bonding_curve::deactivate_curve(&env, &creator, curve_id);
    }

    /// Returns a price quote for buying or selling `amount` tokens without
    /// executing any trade.
    pub fn bc_get_quote(env: Env, curve_id: u64, amount: i128) -> bonding_curve::PriceQuote {
        bonding_curve::get_quote(&env, curve_id, amount)
    }

    /// Returns the current spot price for a bonding curve (× PRECISION).
    pub fn bc_get_spot_price(env: Env, curve_id: u64) -> i128 {
        bonding_curve::get_spot_price(&env, curve_id)
    }

    /// Returns a bonding curve's full configuration and state.
    pub fn bc_get_curve(env: Env, curve_id: u64) -> bonding_curve::BondingCurve {
        bonding_curve::get_curve_info(&env, curve_id)
    }

    // ── TWAP oracle ───────────────────────────────────────────────────────────

    /// Creates a new TWAP oracle for manipulation-resistant price feeds.
    ///
    /// * `updater`          — address authorised to push price updates.
    /// * `window_seconds`   — default TWAP window in seconds (60 – 604 800).
    /// * `max_observations` — ring-buffer capacity (2 – 256).
    /// * `initial_price`    — seed price × PRICE_PRECISION (must be > 0).
    ///
    /// Returns the new oracle ID.
    /// Emits `("twap_new",)` with `(oracle_id, base_token, quote_token, initial_price)`.
    pub fn twap_create_oracle(
        env: Env,
        creator: Address,
        updater: Address,
        base_token: Address,
        quote_token: Address,
        window_seconds: u64,
        max_observations: u32,
        initial_price: i128,
    ) -> u64 {
        Self::require_not_paused(&env);
        twap_oracle::create_oracle(
            &env,
            &creator,
            &updater,
            &base_token,
            &quote_token,
            window_seconds,
            max_observations,
            initial_price,
        )
    }

    /// Records a new price observation into the oracle's ring buffer.
    ///
    /// Only the oracle's designated `updater` address may call this.
    /// Emits `("twap_upd",)` with `(oracle_id, price, timestamp, accumulator)`.
    pub fn twap_record_price(env: Env, updater: Address, oracle_id: u64, price: i128) {
        Self::require_not_paused(&env);
        twap_oracle::record_price(&env, &updater, oracle_id, price);
    }

    /// Returns the TWAP over the oracle's configured default window.
    ///
    /// Uses cumulative price accumulators: `TWAP = Δaccumulator / Δtime`.
    /// Falls back to the latest spot price if fewer than 2 observations exist.
    pub fn twap_get_twap(env: Env, oracle_id: u64) -> twap_oracle::TwapResult {
        twap_oracle::get_twap(&env, oracle_id)
    }

    /// Returns the TWAP over a custom `window_seconds`.
    /// Pass `window_seconds = 0` to use the oracle's configured default.
    pub fn twap_get_twap_window(
        env: Env,
        oracle_id: u64,
        window_seconds: u64,
    ) -> twap_oracle::TwapResult {
        twap_oracle::get_twap_with_window(&env, oracle_id, window_seconds)
    }

    /// Returns the latest spot price for an oracle (not time-weighted).
    pub fn twap_get_latest_price(env: Env, oracle_id: u64) -> i128 {
        twap_oracle::get_latest_price(&env, oracle_id)
    }

    /// Returns up to `limit` most-recent observations for an oracle,
    /// in chronological order (oldest first).
    pub fn twap_get_observations(
        env: Env,
        oracle_id: u64,
        limit: u32,
    ) -> Vec<twap_oracle::Observation> {
        twap_oracle::get_observations(&env, oracle_id, limit)
    }

    /// Returns the oracle configuration and live state.
    pub fn twap_get_oracle(env: Env, oracle_id: u64) -> twap_oracle::TwapOracle {
        twap_oracle::get_oracle_info(&env, oracle_id)
    }

    /// Updates the oracle's TWAP window and/or updater address.
    /// Only the current updater may call this.
    /// Emits `("twap_cfg",)` with `(oracle_id, new_window_seconds, new_updater)`.
    pub fn twap_update_config(
        env: Env,
        updater: Address,
        oracle_id: u64,
        new_window_seconds: u64,
        new_updater: Address,
    ) {
        Self::require_not_paused(&env);
        twap_oracle::update_config(&env, &updater, oracle_id, new_window_seconds, &new_updater);
    }

    /// Deactivates a TWAP oracle. Only the updater may call this.
    /// Emits `("twap_off",)` with `(oracle_id,)`.
    pub fn twap_deactivate(env: Env, updater: Address, oracle_id: u64) {
        Self::require_not_paused(&env);
        twap_oracle::deactivate_oracle(&env, &updater, oracle_id);
    }

    /// Checks and awards milestones when a creator reaches specific tip thresholds.
    ///
    /// Called internally after tips are processed. Emits milestone events.
    fn check_and_award_milestones(env: &Env, creator: &Address, token: &Address, new_total: i128) {
        let milestones = Self::get_creator_milestones(env, creator);

        for (idx, milestone) in milestones.iter().enumerate() {
            if !milestone.completed && new_total >= milestone.goal_amount {
                let reward = (milestone.goal_amount * 5) / 100; // 5% reward

                let bal_key = DataKey::CreatorBalance(creator.clone(), token.clone());
                let existing_bal: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
                let new_bal: i128 = existing_bal.checked_add(reward).expect("balance overflow");
                env.storage().persistent().set(&bal_key, &new_bal);

                let mut updated_milestone = milestone.clone();
                updated_milestone.completed = true;
                env.storage().persistent().set(
                    &DataKey::Milestone(MilestoneKey::Record(creator.clone(), idx as u64)),
                    &updated_milestone,
                );

                env.events().publish(
                    (symbol_short!("milestone"),),
                    (creator.clone(), milestone.goal_amount, reward),
                );
            }
        }
    }

    /// Creates a milestone for a creator. Admin only.
    ///
    /// Emits `("milestone_created",)` with data `(creator, goal_amount)`.
    pub fn create_milestone(
        env: Env,
        admin: Address,
        creator: Address,
        goal_amount: i128,
        description: String,
    ) -> u64 {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        if goal_amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidGoalAmount);
        }

        let counter_key = DataKey::Milestone(MilestoneKey::Counter(creator.clone()));
        let milestone_id: u64 = env.storage().persistent().get(&counter_key).unwrap_or(0);

        let milestone = Milestone {
            id: milestone_id,
            creator: creator.clone(),
            goal_amount,
            current_amount: 0,
            description,
            deadline: None,
            completed: false,
        };

        env.storage().persistent().set(
            &DataKey::Milestone(MilestoneKey::Record(creator.clone(), milestone_id)),
            &milestone,
        );
        env.storage()
            .persistent()
            .set(&counter_key, &(milestone_id + 1));

        let mut active: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::Milestone(MilestoneKey::Active(creator.clone())))
            .unwrap_or_else(|| Vec::new(&env));
        active.push_back(milestone_id);
        env.storage().persistent().set(
            &DataKey::Milestone(MilestoneKey::Active(creator.clone())),
            &active,
        );

        env.events()
            .publish((symbol_short!("ms_crt"),), (creator, goal_amount));

        milestone_id
    }

    /// Returns all milestones for a creator.
    pub fn get_creator_milestones(env: Env, creator: Address) -> Vec<Milestone> {
        let milestone_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::Milestone(MilestoneKey::Active(creator.clone())))
            .unwrap_or_else(|| Vec::new(&env));

        let mut milestones = Vec::new(&env);
        for id in milestone_ids.iter() {
            if let Some(milestone) =
                env.storage()
                    .persistent()
                    .get::<_, Milestone>(&DataKey::Milestone(MilestoneKey::Record(
                        creator.clone(),
                        id,
                    )))
            {
                milestones.push_back(milestone);
            }
        }
        milestones
    }

    /// Returns a specific milestone for a creator.
    pub fn get_milestone(env: Env, creator: Address, milestone_id: u64) -> Milestone {
        env.storage()
            .persistent()
            .get(&DataKey::Milestone(MilestoneKey::Record(
                creator,
                milestone_id,
            )))
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::MilestoneNotFound))
    }

    // ── privacy features ──────────────────────────────────────────────────────

    /// Sends an anonymous or private tip. Amount is hashed for privacy.
    ///
    /// If `is_anonymous` is true, the tipper identity is not stored.
    /// Returns the private tip ID.
    /// Emits `("private_tip",)` with data `(tip_id, creator, is_anonymous)`.
    pub fn tip_private(
        env: Env,
        creator: Address,
        token: Address,
        amount: i128,
        is_anonymous: bool,
    ) -> u64 {
        Self::require_not_paused(&env);
        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }

        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        let tip_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::PrivateTip(PrivateTipKey::Ctr))
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::PrivateTip(PrivateTipKey::Ctr), &(tip_id + 1));

        let amount_bytes = amount.to_le_bytes();
        let amount_hash = env.crypto().sha256(&amount_bytes);

        let tipper = if is_anonymous {
            None
        } else {
            Some(env.current_contract_address())
        };

        let created_at = env.ledger().timestamp();
        let private_tip = privacy_tip::PrivateTip {
            id: tip_id,
            creator: creator.clone(),
            amount_hash,
            is_anonymous,
            tipper,
            created_at,
            revealed: false,
        };

        env.storage().persistent().set(
            &DataKey::PrivateTip(PrivateTipKey::Record(tip_id)),
            &private_tip,
        );

        env.events().publish(
            (symbol_short!("priv_tip"),),
            (tip_id, creator, is_anonymous),
        );

        tip_id
    }

    /// Reveals the amount of a private tip by providing the original amount.
    ///
    /// The amount is hashed and compared with the stored hash. If it matches,
    /// the tip is credited to the creator and marked as revealed.
    /// Emits `("tip_revealed",)` with data `(tip_id, amount)`.
    pub fn reveal_tip(env: Env, sender: Address, token: Address, tip_id: u64, amount: i128) {
        Self::require_not_paused(&env);
        sender.require_auth();

        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }

        let mut private_tip: privacy_tip::PrivateTip = env
            .storage()
            .persistent()
            .get(&DataKey::PrivateTip(PrivateTipKey::Record(tip_id)))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::PrivateTipNotFound));

        let amount_bytes = amount.to_le_bytes();
        let computed_hash = env.crypto().sha256(&amount_bytes);

        if computed_hash != private_tip.amount_hash {
            panic_with_error!(&env, InsuranceError::InvalidReveal);
        }

        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        // Transfer tokens
        token::Client::new(&env, &token).transfer(
            &sender,
            &env.current_contract_address(),
            &amount,
        );

        let fee_bp: u32 = env
            .storage()
            .instance()
            .get(&DataKey::Fee(FeeKey::BasisPoints))
            .unwrap_or(0);
        let fee: i128 = (amount * fee_bp as i128) / 10_000;
        let creator_amount = amount - fee;

        if fee > 0 {
            let fee_key = DataKey::Fee(FeeKey::Balance(token.clone()));
            let current_fee: i128 = env.storage().instance().get(&fee_key).unwrap_or(0);
            let new_fee_bal: i128 = current_fee.checked_add(fee).expect("fee overflow");
            env.storage().instance().set(&fee_key, &new_fee_bal);
        }

        let bal_key = DataKey::CreatorBalance(private_tip.creator.clone(), token.clone());
        let existing_bal: i128 = env.storage().persistent().get(&bal_key).unwrap_or(0);
        let new_bal: i128 = existing_bal
            .checked_add(creator_amount)
            .expect("balance overflow");
        env.storage().persistent().set(&bal_key, &new_bal);

        let tot_key = DataKey::CreatorTotal(private_tip.creator.clone(), token.clone());
        let existing_tot: i128 = env.storage().persistent().get(&tot_key).unwrap_or(0);
        let new_tot: i128 = existing_tot
            .checked_add(creator_amount)
            .expect("total overflow");
        env.storage().persistent().set(&tot_key, &new_tot);

        private_tip.revealed = true;
        env.storage().persistent().set(
            &DataKey::PrivateTip(PrivateTipKey::Record(tip_id)),
            &private_tip,
        );
        env.storage()
            .persistent()
            .set(&DataKey::PrivateTip(PrivateTipKey::Amount(tip_id)), &amount);

        env.events()
            .publish((symbol_short!("tip_rev"),), (tip_id, amount));
    }

    /// Returns a private tip record by ID.
    pub fn get_private_tip(env: Env, tip_id: u64) -> privacy_tip::PrivateTip {
        env.storage()
            .persistent()
            .get(&DataKey::PrivateTip(PrivateTipKey::Record(tip_id)))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::PrivateTipNotFound))
    }

    /// Returns the revealed amount for a private tip (if revealed).
    pub fn get_private_tip_amount(env: Env, tip_id: u64) -> Option<i128> {
        env.storage()
            .persistent()
            .get(&DataKey::PrivateTip(PrivateTipKey::Amount(tip_id)))
    }

    // ── streaming protocol ──────────────────────────────────────────────────────

    /// Creates a new stream from `sender` to `creator`.
    ///
    /// The stream will continuously transfer funds at `amount_per_second` until
    /// it is stopped, cancelled, or reaches its end time.
    ///
    /// Emits `("stream_created",)` with data `(stream_id, sender, creator, amount_per_second, duration)`.
    pub fn create_stream(
        env: Env,
        sender: Address,
        creator: Address,
        token: Address,
        amount_per_second: i128,
        duration: u64,
    ) -> u64 {
        Self::require_not_paused(&env);
        sender.require_auth();

        if amount_per_second <= 0 {
            panic_with_error!(&env, InsuranceError::InvalidStreamRate);
        }

        // Maximum rate: 1000 tokens/second (adjust as needed)
        if amount_per_second > 1000 {
            panic_with_error!(&env, AuctionError::StrmRateMax);
        }

        if duration == 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }

        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        let stream_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::Stream(StreamKey::Ctr))
            .unwrap_or(0);
        let now = env.ledger().timestamp();
        let total_amount = amount_per_second * duration as i128;

        let stream = Stream {
            stream_id,
            sender: sender.clone(),
            creator: creator.clone(),
            token: token.clone(),
            amount_per_second,
            start_time: now,
            end_time: now + duration,
            withdrawn: 0,
            status: StreamStatus::Active,
            created_at: now,
            updated_at: now,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Stream(StreamKey::Record(stream_id)), &stream);
        env.storage()
            .instance()
            .set(&DataKey::Stream(StreamKey::Ctr), &(stream_id + 1));

        // Add to sender's stream list
        let mut sender_streams: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::Stream(StreamKey::SenderStreams(sender.clone())))
            .unwrap_or_else(|| Vec::new(&env));
        sender_streams.push_back(stream_id);
        env.storage().persistent().set(
            &DataKey::Stream(StreamKey::SenderStreams(sender.clone())),
            &sender_streams,
        );

        // Add to creator's stream list
        let mut creator_streams: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::Stream(StreamKey::CreatorStreams(creator.clone())))
            .unwrap_or_else(|| Vec::new(&env));
        creator_streams.push_back(stream_id);
        env.storage().persistent().set(
            &DataKey::Stream(StreamKey::CreatorStreams(creator.clone())),
            &creator_streams,
        );

        // Transfer total amount into escrow
        token::Client::new(&env, &token).transfer(
            &sender,
            &env.current_contract_address(),
            &total_amount,
        );

        env.events().publish(
            (symbol_short!("strm_cre"),),
            (stream_id, sender, creator, amount_per_second, duration),
        );

        stream_id
    }

    /// Calculates the amount that has been streamed up to the current time for a given stream.
    fn calculate_streamed_amount(env: &Env, stream: &Stream) -> i128 {
        let current_time = env.ledger().timestamp();

        if stream.status != StreamStatus::Active && stream.status != StreamStatus::Paused {
            return stream.withdrawn;
        }

        let elapsed = if current_time < stream.start_time {
            0
        } else if current_time > stream.end_time {
            stream.end_time - stream.start_time
        } else {
            current_time - stream.start_time
        };

        (stream.amount_per_second * elapsed as i128)
            .min(stream.amount_per_second * (stream.end_time - stream.start_time) as i128)
    }

    /// Starts a stream (or resumes a paused stream).
    ///
    /// Only the sender can start/activate a stream.
    /// Emits `("stream_started",)` with data `(stream_id)`.
    pub fn start_stream(env: Env, sender: Address, stream_id: u64) {
        Self::require_not_paused(&env);
        sender.require_auth();

        let mut stream: Stream = env
            .storage()
            .persistent()
            .get(&DataKey::Stream(StreamKey::Record(stream_id)))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::StreamNotFound));

        if stream.sender != sender {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        if stream.status == StreamStatus::Cancelled {
            panic_with_error!(&env, InsuranceError::StreamAlreadyCancelled);
        }

        if stream.status == StreamStatus::Completed {
            panic_with_error!(&env, InsuranceError::StreamAlreadyCompleted);
        }

        let now = env.ledger().timestamp();

        // If starting from scratch, set start_time
        if stream.status == StreamStatus::Paused {
            // Resume from paused state
            let pause_duration = now - stream.updated_at;
            stream.start_time += pause_duration;
            stream.end_time += pause_duration;
        }

        stream.status = StreamStatus::Active;
        stream.updated_at = now;

        env.storage()
            .persistent()
            .set(&DataKey::Stream(StreamKey::Record(stream_id)), &stream);

        env.events()
            .publish((symbol_short!("strm_sta"),), stream_id);
    }

    /// Stops (pauses) an active stream.
    ///
    /// Only the sender can stop a stream.
    /// Emits `("stream_stopped",)` with data `(stream_id, streamed_amount)`.
    pub fn stop_stream(env: Env, sender: Address, stream_id: u64) {
        Self::require_not_paused(&env);
        sender.require_auth();

        let mut stream: Stream = env
            .storage()
            .persistent()
            .get(&DataKey::Stream(StreamKey::Record(stream_id)))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::StreamNotFound));

        if stream.sender != sender {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        if stream.status != StreamStatus::Active {
            panic_with_error!(&env, InsuranceError::StreamNotStarted);
        }

        if stream.status == StreamStatus::Cancelled {
            panic_with_error!(&env, InsuranceError::StreamAlreadyCancelled);
        }

        let streamed_amount = Self::calculate_streamed_amount(&env, &stream);
        stream.status = StreamStatus::Paused;
        stream.withdrawn = streamed_amount;
        stream.updated_at = env.ledger().timestamp();

        env.storage()
            .persistent()
            .set(&DataKey::Stream(StreamKey::Record(stream_id)), &stream);

        env.events()
            .publish((symbol_short!("strm_sto"),), (stream_id, streamed_amount));
    }

    /// Withdraws the currently streamed amount for a stream.
    ///
    /// The creator can withdraw the amount that has been streamed up to now.
    /// Emits `("stream_withdrawn",)` with data `(stream_id, amount, creator)`.
    pub fn withdraw_streamed(env: Env, creator: Address, stream_id: u64) {
        Self::require_not_paused(&env);
        creator.require_auth();

        let mut stream: Stream = env
            .storage()
            .persistent()
            .get(&DataKey::Stream(StreamKey::Record(stream_id)))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::StreamNotFound));

        if stream.creator != creator {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        if stream.status == StreamStatus::Cancelled {
            panic_with_error!(&env, InsuranceError::StreamAlreadyCancelled);
        }

        let current_time = env.ledger().timestamp();

        if current_time < stream.start_time {
            panic_with_error!(&env, InsuranceError::StreamNotStarted);
        }

        let total_streamable =
            stream.amount_per_second * (stream.end_time - stream.start_time) as i128;
        let streamed_amount = Self::calculate_streamed_amount(&env, &stream);
        let available_to_withdraw = streamed_amount - stream.withdrawn;

        if available_to_withdraw <= 0 {
            panic_with_error!(&env, AuctionError::NoStreamedAmount);
        }

        // Update stream state BEFORE external call
        stream.withdrawn = streamed_amount;

        // Check if stream is completed
        if current_time >= stream.end_time {
            stream.status = StreamStatus::Completed;
        }

        stream.updated_at = current_time;

        env.storage()
            .persistent()
            .set(&DataKey::Stream(StreamKey::Record(stream_id)), &stream);

        // Transfer tokens to creator
        token::Client::new(&env, &stream.token).transfer(
            &env.current_contract_address(),
            &creator,
            &available_to_withdraw,
        );

        env.events().publish(
            (symbol_short!("strm_wit"),),
            (stream_id, available_to_withdraw, creator),
        );
    }

    /// Cancels an active stream and refunds the remaining tokens to the sender.
    ///
    /// Only the sender can cancel a stream.
    /// Emits `("stream_cancelled",)` with data `(stream_id, refunded_amount)`.
    pub fn cancel_stream(env: Env, sender: Address, stream_id: u64) {
        Self::require_not_paused(&env);
        sender.require_auth();

        let mut stream: Stream = env
            .storage()
            .persistent()
            .get(&DataKey::Stream(StreamKey::Record(stream_id)))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::StreamNotFound));

        if stream.sender != sender {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        if stream.status == StreamStatus::Cancelled {
            panic_with_error!(&env, InsuranceError::StreamAlreadyCancelled);
        }

        if stream.status == StreamStatus::Completed {
            panic_with_error!(&env, InsuranceError::StreamAlreadyCompleted);
        }

        let current_time = env.ledger().timestamp();
        let streamed_amount = Self::calculate_streamed_amount(&env, &stream);

        // Calculate total amount that was put into escrow
        let total_amount = stream.amount_per_second * (stream.end_time - stream.start_time) as i128;
        let remaining_amount = total_amount - streamed_amount;

        // Mark stream as cancelled
        stream.status = StreamStatus::Cancelled;
        stream.updated_at = current_time;

        env.storage()
            .persistent()
            .set(&DataKey::Stream(StreamKey::Record(stream_id)), &stream);

        // Refund remaining tokens to sender
        if remaining_amount > 0 {
            token::Client::new(&env, &stream.token).transfer(
                &env.current_contract_address(),
                &sender,
                &remaining_amount,
            );
        }

        // If there's any withdrawn amount not yet claimed, it's already in the creator's balance
        // (handled by the periodic withdraw_streamed calls)

        env.events()
            .publish((symbol_short!("strm_can"),), (stream_id, remaining_amount));
    }

    /// Returns the current stream details.
    pub fn get_stream(env: Env, stream_id: u64) -> Option<Stream> {
        env.storage()
            .persistent()
            .get(&DataKey::Stream(StreamKey::Record(stream_id)))
    }

    /// Returns all stream IDs for a creator.
    pub fn get_streams_by_creator(env: Env, creator: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::Stream(StreamKey::CreatorStreams(creator)))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Returns all stream IDs for a sender.
    pub fn get_streams_by_sender(env: Env, sender: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::Stream(StreamKey::SenderStreams(sender)))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Returns the current streamed amount for a stream.
    pub fn get_streamed_amount(env: Env, stream_id: u64) -> i128 {
        let stream: Stream = env
            .storage()
            .persistent()
            .get(&DataKey::Stream(StreamKey::Record(stream_id)))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::StreamNotFound));

        Self::calculate_streamed_amount(&env, &stream)
    }

    /// Returns the available amount to withdraw for a stream.
    pub fn get_available_to_withdraw(env: Env, stream_id: u64) -> i128 {
        let stream: Stream = env
            .storage()
            .persistent()
            .get(&DataKey::Stream(StreamKey::Record(stream_id)))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::StreamNotFound));

        if stream.status == StreamStatus::Cancelled || stream.status == StreamStatus::Completed {
            return 0;
        }

        let current_time = env.ledger().timestamp();
        if current_time < stream.start_time {
            return 0;
        }

        let streamed_amount = Self::calculate_streamed_amount(&env, &stream);
        streamed_amount - stream.withdrawn
    }

    /// Initialize the insurance pool configuration for the contract.
    ///
    /// Admin only. Sets up insurance pool parameters.
    /// Emits `("insurance_config_set",)` with data `(min_contrib, max_contrib, premium_rate, payout_ratio)`.
    pub fn insurance_set_config(
        env: Env,
        admin: Address,
        min_contribution: i128,
        max_contribution: i128,
        premium_rate_bps: u32,
        payout_ratio_bps: u32,
        claim_cooldown: u64,
        admin_fee_bps: u32,
        tip_premium_bps: u32,
    ) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        if min_contribution < 0 || max_contribution <= min_contribution {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        if premium_rate_bps > 500 {
            panic_with_error!(&env, FeatureError::FeeExceedsMaximum);
        }
        if payout_ratio_bps > 10000 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        if tip_premium_bps > 1000 {
            panic_with_error!(&env, FeatureError::FeeExceedsMaximum);
        }

        let config = InsurancePoolConfig {
            min_contribution,
            max_contribution,
            premium_rate_bps,
            payout_ratio_bps,
            claim_cooldown,
            admin_fee_bps,
            tip_premium_bps,
        };
        env.storage()
            .instance()
            .set(&DataKey::Insurance(InsuranceKey::Cfg), &config);

        env.events().publish(
            (symbol_short!("ins_cfg"),),
            (
                min_contribution,
                max_contribution,
                premium_rate_bps,
                payout_ratio_bps,
                tip_premium_bps,
            ),
        );
    }

    /// Enable or disable the insurance feature.
    ///
    /// Admin only.
    pub fn insurance_set_enabled(env: Env, admin: Address, enabled: bool) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        env.storage()
            .instance()
            .set(&DataKey::Insurance(InsuranceKey::Enabled), &enabled);
        env.events().publish((symbol_short!("ins_en"),), enabled);
    }

    /// Set the maximum number of active claims a creator can have simultaneously.
    ///
    /// Admin only.
    pub fn insurance_set_max_active_claims(env: Env, admin: Address, max_claims: u32) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        if max_claims == 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        env.storage()
            .instance()
            .set(&DataKey::Insurance(InsuranceKey::MaxClms), &max_claims);
        env.events()
            .publish((symbol_short!("ins_max"),), max_claims);
    }

    /// Set the insurance admin address.
    ///
    /// Admin only.
    pub fn insurance_set_admin(env: Env, admin: Address, insurance_admin: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        env.storage()
            .instance()
            .set(&DataKey::Insurance(InsuranceKey::Admin), &insurance_admin);
        env.events()
            .publish((symbol_short!("ins_adm"),), insurance_admin);
    }

    /// Contribute to the insurance pool for a specific token.
    ///
    /// Creator can contribute funds to gain insurance coverage. The contribution amount
    /// must be within configured limits. The sender must transfer the tokens to this contract.
    ///
    /// Emits `("insurance_contribution",)` with data `(creator, token, amount, coverage_amount)`.
    pub fn insurance_contribute(env: Env, creator: Address, token: Address, amount: i128) {
        Self::require_not_paused(&env);
        creator.require_auth();

        // Check if insurance is enabled
        let enabled: bool = env
            .storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Enabled))
            .unwrap_or(true);
        if !enabled {
            panic_with_error!(&env, InsuranceError::InsuranceDisabled);
        }

        let config: InsurancePoolConfig = env
            .storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Cfg))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::InsPoolNotCfg));

        if amount < config.min_contribution {
            panic_with_error!(&env, InsuranceError::ContributionTooLow);
        }
        if amount > config.max_contribution {
            panic_with_error!(&env, InsuranceError::ContributionTooHigh);
        }

        // Check whitelist
        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        // Transfer tokens from creator to contract
        token::Client::new(&env, &token).transfer(
            &creator,
            &env.current_contract_address(),
            &amount,
        );

        // Calculate premium for this contribution period
        let premium_amount = (amount * config.premium_rate_bps as i128) / 10_000;

        // Update pool state
        let pool_key = DataKey::Insurance(InsuranceKey::Token(token.clone()));
        let mut pool: InsurancePool =
            env.storage()
                .persistent()
                .get(&pool_key)
                .unwrap_or_else(|| InsurancePool {
                    token: token.clone(),
                    total_reserves: 0,
                    total_contributions: 0,
                    total_claims_paid: 0,
                    active_claims: 0,
                    total_claims: 0,
                    last_payout_time: env.ledger().timestamp(),
                });

        pool.total_reserves += amount - premium_amount;
        pool.total_contributions += amount;
        env.storage().persistent().set(&pool_key, &pool);

        // Update creator contribution
        let contrib_key = DataKey::Insurance(InsuranceKey::Contrib(creator.clone(), token.clone()));
        let current_contrib: i128 = env.storage().persistent().get(&contrib_key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&contrib_key, &(current_contrib + amount));

        // Add to creator's token list
        let tokens_key = DataKey::CreatorTokens(creator.clone());
        let mut tokens: Vec<Address> = env
            .storage()
            .persistent()
            .get(&tokens_key)
            .unwrap_or_else(|| Vec::new(&env));
        if !tokens.contains(&token) {
            tokens.push_back(token.clone());
            env.storage().persistent().set(&tokens_key, &tokens);
        }

        // Calculate and add to platform fee balance
        if premium_amount > 0 {
            let fee_key = DataKey::Fee(FeeKey::Balance(token.clone()));
            let current_fee: i128 = env.storage().instance().get(&fee_key).unwrap_or(0);
            env.storage()
                .instance()
                .set(&fee_key, &(current_fee + premium_amount));
        }

        env.events().publish(
            (symbol_short!("ins_con"),),
            (creator.clone(), token, amount, pool.total_reserves),
        );
    }

    /// Submit an insurance claim for a failed transaction.
    ///
    /// A creator can submit a claim when they experience a failed transaction
    /// (e.g., failed tip, failed withdrawal). The claim must include proof
    /// (transaction hash) and will be subject to review.
    ///
    /// Emits `("claim_submitted",)` with data `(claim_id, creator, token, amount)`.
    pub fn insurance_submit_claim(
        env: Env,
        creator: Address,
        token: Address,
        amount: i128,
        tx_hash: BytesN<32>,
    ) -> u64 {
        Self::require_not_paused(&env);
        creator.require_auth();

        if amount <= 0 {
            panic_with_error!(&env, InsuranceError::InvalidClaimAmount);
        }

        // Check if insurance is enabled
        let enabled: bool = env
            .storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Enabled))
            .unwrap_or(true);
        if !enabled {
            panic_with_error!(&env, InsuranceError::InsuranceDisabled);
        }

        // Check if pool is configured
        let config: InsurancePoolConfig = env
            .storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Cfg))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::InsPoolNotCfg));

        // Check creator has coverage
        let max_payout = Self::insurance_get_coverage(env.clone(), creator.clone(), token.clone());
        if max_payout <= 0 {
            panic_with_error!(&env, InsuranceError::NoCoverage);
        }

        // Check active claim limit
        let max_active: u32 = env
            .storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::MaxClms))
            .unwrap_or(3);
        let active_key =
            DataKey::Insurance(InsuranceKey::ActiveClms(creator.clone(), token.clone()));
        let active_claims: u32 = env.storage().persistent().get(&active_key).unwrap_or(0);
        if active_claims >= max_active {
            panic_with_error!(&env, InsuranceError::TooManyActiveClaims);
        }

        // Check last claim cooldown
        let last_claim_key =
            DataKey::Insurance(InsuranceKey::LastClm(creator.clone(), token.clone()));
        let last_claim: u64 = env.storage().persistent().get(&last_claim_key).unwrap_or(0);
        let now = env.ledger().timestamp();
        if last_claim > 0 && now < last_claim + config.claim_cooldown {
            panic_with_error!(&env, InsuranceError::ClaimCooldownActive);
        }

        // Check pool has sufficient reserves
        let pool_key = DataKey::Insurance(InsuranceKey::Token(token.clone()));
        let pool: InsurancePool = env
            .storage()
            .persistent()
            .get(&pool_key)
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::InsufficientReserves));

        if amount > max_payout {
            panic_with_error!(&env, InsuranceError::PayoutExceedsReserves);
        }

        if amount > pool.total_reserves {
            panic_with_error!(&env, InsuranceError::InsufficientReserves);
        }

        // Create claim
        let claim_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Ctr))
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::Insurance(InsuranceKey::Ctr), &(claim_id + 1));

        let claim = InsuranceClaim {
            claim_id,
            creator: creator.clone(),
            token: token.clone(),
            amount,
            tx_hash,
            status: ClaimStatus::Pending,
            created_at: now,
            updated_at: now,
            last_claim_at: last_claim,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Insurance(InsuranceKey::Claim(claim_id)), &claim);

        // Add to creator's claims list
        let creator_claims_key =
            DataKey::Insurance(InsuranceKey::Clms(creator.clone(), token.clone()));
        let mut creator_claims: Vec<u64> = env
            .storage()
            .persistent()
            .get(&creator_claims_key)
            .unwrap_or_else(|| Vec::new(&env));
        creator_claims.push_back(claim_id);
        env.storage()
            .persistent()
            .set(&creator_claims_key, &creator_claims);

        // Update active claim count
        env.storage()
            .persistent()
            .set(&active_key, &(active_claims + 1));

        // Update total claims count
        let total_claims_key =
            DataKey::Insurance(InsuranceKey::TotalClms(creator.clone(), token.clone()));
        let total_claims: u32 = env
            .storage()
            .persistent()
            .get(&total_claims_key)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&total_claims_key, &(total_claims + 1));

        // Update pool
        let mut updated_pool = pool.clone();
        updated_pool.active_claims += 1;
        updated_pool.total_claims += 1;
        env.storage().persistent().set(&pool_key, &updated_pool);

        env.events().publish(
            (symbol_short!("clm_sub"),),
            (claim_id, creator, token, amount),
        );

        claim_id
    }

    /// Approve an insurance claim (admin or insurance admin).
    ///
    /// Only the contract admin or insurance admin can approve claims.
    /// Once approved, the claim can be paid out.
    ///
    /// Emits `("claim_approved",)` with data `(claim_id, approver)`.
    pub fn insurance_approve_claim(env: Env, approver: Address, claim_id: u64) {
        approver.require_auth();

        // Check if caller is admin or insurance admin
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        let insurance_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Admin))
            .unwrap_or(stored_admin.clone());
        if approver != stored_admin && approver != insurance_admin {
            panic_with_error!(&env, InsuranceError::AdmAppReq);
        }

        let claim: InsuranceClaim = env
            .storage()
            .persistent()
            .get(&DataKey::Insurance(InsuranceKey::Claim(claim_id)))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::ClaimNotFound));

        if claim.status != ClaimStatus::Pending {
            panic_with_error!(&env, InsuranceError::ClaimNotApproved);
        }

        let mut updated_claim = claim.clone();
        updated_claim.status = ClaimStatus::Approved;
        updated_claim.updated_at = env.ledger().timestamp();
        env.storage()
            .persistent()
            .set(&DataKey::Insurance(InsuranceKey::Claim(claim_id)), &claim);

        // Update pool
        let pool_key = DataKey::Insurance(InsuranceKey::Token(claim.token.clone()));
        let pool: InsurancePool = env.storage().persistent().get(&pool_key).unwrap();
        let mut updated_pool = pool.clone();
        updated_pool.active_claims -= 1;
        env.storage().persistent().set(&pool_key, &updated_pool);

        // Update creator active claims
        let active_key = DataKey::Insurance(InsuranceKey::ActiveClms(
            claim.creator.clone(),
            claim.token.clone(),
        ));
        let active_claims: u32 = env.storage().persistent().get(&active_key).unwrap_or(1);
        env.storage()
            .persistent()
            .set(&active_key, &(active_claims - 1));

        env.events()
            .publish((symbol_short!("clm_app"),), (claim_id, approver));
    }

    /// Reject an insurance claim (admin or insurance admin).
    ///
    /// Only the contract admin or insurance admin can reject claims.
    ///
    /// Emits `("claim_rejected",)` with data `(claim_id, rejector)`.
    pub fn insurance_reject_claim(env: Env, rejector: Address, claim_id: u64) {
        rejector.require_auth();

        // Check if caller is admin or insurance admin
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        let insurance_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Admin))
            .unwrap_or(stored_admin.clone());
        if rejector != stored_admin && rejector != insurance_admin {
            panic_with_error!(&env, InsuranceError::AdmAppReq);
        }

        let claim: InsuranceClaim = env
            .storage()
            .persistent()
            .get(&DataKey::Insurance(InsuranceKey::Claim(claim_id)))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::ClaimNotFound));

        if claim.status != ClaimStatus::Pending && claim.status != ClaimStatus::Approved {
            panic_with_error!(&env, InsuranceError::ClaimNotApproved);
        }

        let mut updated_claim = claim.clone();
        updated_claim.status = ClaimStatus::Rejected;
        updated_claim.updated_at = env.ledger().timestamp();
        env.storage()
            .persistent()
            .set(&DataKey::Insurance(InsuranceKey::Claim(claim_id)), &claim);

        if claim.status == ClaimStatus::Approved {
            // Update pool
            let pool_key = DataKey::Insurance(InsuranceKey::Token(claim.token.clone()));
            let pool: InsurancePool = env.storage().persistent().get(&pool_key).unwrap();
            let mut updated_pool = pool.clone();
            updated_pool.active_claims -= 1;
            env.storage().persistent().set(&pool_key, &updated_pool);

            // Update creator active claims
            let active_key = DataKey::Insurance(InsuranceKey::ActiveClms(
                claim.creator.clone(),
                claim.token.clone(),
            ));
            let active_claims: u32 = env.storage().persistent().get(&active_key).unwrap_or(1);
            env.storage()
                .persistent()
                .set(&active_key, &(active_claims - 1));
        }

        env.events()
            .publish((symbol_short!("clm_rej"),), (claim_id, rejector));
    }

    /// Pay out an approved insurance claim.
    ///
    /// Transfers funds from the insurance pool to the creator.
    /// Can only be called for approved claims that haven't been paid yet.
    ///
    /// Emits `("claim_paid",)` with data `(claim_id, amount, creator)`.
    pub fn insurance_pay_claim(env: Env, caller: Address, claim_id: u64) {
        caller.require_auth();

        // Check if caller is admin or insurance admin
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        let insurance_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Admin))
            .unwrap_or(stored_admin.clone());
        if caller != stored_admin && caller != insurance_admin {
            panic_with_error!(&env, InsuranceError::AdmAppReq);
        }

        let claim: InsuranceClaim = env
            .storage()
            .persistent()
            .get(&DataKey::Insurance(InsuranceKey::Claim(claim_id)))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::ClaimNotFound));

        if claim.status != ClaimStatus::Approved {
            panic_with_error!(&env, InsuranceError::ClaimNotApproved);
        }

        // Check pool has sufficient reserves
        let pool_key = DataKey::Insurance(InsuranceKey::Token(claim.token.clone()));
        let pool: InsurancePool = env
            .storage()
            .persistent()
            .get(&pool_key)
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::InsufficientReserves));

        if claim.amount > pool.total_reserves {
            panic_with_error!(&env, InsuranceError::InsufficientReserves);
        }

        // Update creator's contribution (deduct claim amount)
        let contrib_key = DataKey::Insurance(InsuranceKey::Contrib(
            claim.creator.clone(),
            claim.token.clone(),
        ));
        let current_contrib: i128 = env.storage().persistent().get(&contrib_key).unwrap_or(0);
        let new_contrib = current_contrib - claim.amount;
        env.storage()
            .persistent()
            .set(&contrib_key, &new_contrib.max(0));

        // Update pool reserves
        let mut updated_pool = pool.clone();
        updated_pool.total_reserves -= claim.amount;
        updated_pool.total_claims_paid += claim.amount;
        updated_pool.active_claims -= 1;
        updated_pool.last_payout_time = env.ledger().timestamp();
        env.storage().persistent().set(&pool_key, &updated_pool);

        // Update claim status
        let mut updated_claim = claim.clone();
        updated_claim.status = ClaimStatus::Paid;
        updated_claim.updated_at = env.ledger().timestamp();
        env.storage()
            .persistent()
            .set(&DataKey::Insurance(InsuranceKey::Claim(claim_id)), &claim);

        // Update creator's last claim time and active claims
        let last_claim_key = DataKey::Insurance(InsuranceKey::LastClm(
            claim.creator.clone(),
            claim.token.clone(),
        ));
        env.storage()
            .persistent()
            .set(&last_claim_key, &env.ledger().timestamp());

        let active_key = DataKey::Insurance(InsuranceKey::ActiveClms(
            claim.creator.clone(),
            claim.token.clone(),
        ));
        let active_claims: u32 = env.storage().persistent().get(&active_key).unwrap_or(1);
        env.storage()
            .persistent()
            .set(&active_key, &(active_claims - 1));

        // Transfer funds to creator
        token::Client::new(&env, &claim.token).transfer(
            &env.current_contract_address(),
            &claim.creator,
            &claim.amount,
        );

        env.events().publish(
            (symbol_short!("clm_paid"),),
            (claim_id, claim.amount, claim.creator),
        );
    }

    /// Get the insurance pool configuration.
    pub fn insurance_get_config(env: Env) -> InsurancePoolConfig {
        env.storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Cfg))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::InsPoolNotCfg))
    }

    /// Check if the insurance feature is enabled.
    pub fn insurance_is_enabled(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Enabled))
            .unwrap_or(true)
    }

    /// Get the insurance pool state for a specific token.
    pub fn insurance_get_pool(env: Env, token: Address) -> Option<InsurancePool> {
        env.storage()
            .persistent()
            .get(&DataKey::Insurance(InsuranceKey::Token(token)))
    }

    /// Get a specific insurance claim by ID.
    pub fn insurance_get_claim(env: Env, claim_id: u64) -> InsuranceClaim {
        env.storage()
            .persistent()
            .get(&DataKey::Insurance(InsuranceKey::Claim(claim_id)))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::ClaimNotFound))
    }

    /// Get creator's contribution amount for a specific token.
    pub fn insurance_get_contribution(env: Env, creator: Address, token: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Insurance(InsuranceKey::Contrib(creator, token)))
            .unwrap_or(0)
    }

    /// Get creator's coverage limit based on their contribution and tips received.
    pub fn insurance_get_coverage(env: Env, creator: Address, token: Address) -> i128 {
        let config: InsurancePoolConfig = env
            .storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Cfg))
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::InsPoolNotCfg));

        // Manual contribution coverage
        let contrib: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Insurance(InsuranceKey::Contrib(
                creator.clone(),
                token.clone(),
            )))
            .unwrap_or(0);

        // Automatic premium coverage estimate from tips received
        let total_received: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::CreatorTotal(creator, token))
            .unwrap_or(0);

        // Since CreatorTotal is net of fees and premiums, we approximate the original gross
        // to find the premium paid. Gross = Net / (1 - fee_bps - premium_bps)
        // For simplicity, we use Net * premium_bps as a conservative estimate of coverage earned.
        let premium_earned = (total_received * config.tip_premium_bps as i128) / 10_000;

        ((contrib + premium_earned) * config.payout_ratio_bps as i128) / 10_000
    }

    /// Get creator's active claim count for a specific token.
    pub fn insurance_get_active_claims(env: Env, creator: Address, token: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Insurance(InsuranceKey::ActiveClms(
                creator, token,
            )))
            .unwrap_or(0)
    }

    /// Get creator's total claim count for a specific token.
    pub fn insurance_get_total_claims(env: Env, creator: Address, token: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Insurance(InsuranceKey::TotalClms(creator, token)))
            .unwrap_or(0)
    }

    /// Check if insurance is available for a creator/token combination.
    pub fn insurance_has_coverage(env: Env, creator: Address, token: Address) -> bool {
        let contrib: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Insurance(InsuranceKey::Contrib(creator, token)))
            .unwrap_or(0);
        contrib > 0
    }

    /// Withdraw excess funds from the insurance pool (admin only).
    ///
    /// Allows admin to withdraw funds beyond a minimum reserve threshold.
    /// Emits `("pool_withdraw",)` with data `(token, amount)`.
    pub fn insurance_withdraw_excess(env: Env, admin: Address, token: Address, amount: i128) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }

        let pool_key = DataKey::Insurance(InsuranceKey::Token(token.clone()));
        let pool: InsurancePool = env
            .storage()
            .persistent()
            .get(&pool_key)
            .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::InsufficientReserves));

        // Keep minimum reserve (10% of total contributions)
        let min_reserve = pool.total_contributions / 10;
        if pool.total_reserves - amount < min_reserve {
            panic_with_error!(&env, InsuranceError::InsufficientReserves);
        }

        let mut updated_pool = pool.clone();
        updated_pool.total_reserves -= amount;
        env.storage().persistent().set(&pool_key, &updated_pool);

        // Transfer to admin
        token::Client::new(&env, &token).transfer(&env.current_contract_address(), &admin, &amount);

        env.events()
            .publish((symbol_short!("pol_wit"),), (token, amount));
    }

    /// Get the insurance admin address.
    pub fn insurance_get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::Admin))
            .unwrap_or_else(|| env.storage().instance().get(&DataKey::Admin).unwrap())
    }

    /// Get the maximum active claims per creator.
    pub fn insurance_get_max_active_claims(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::Insurance(InsuranceKey::MaxClms))
            .unwrap_or(3)
    }

    /// Process multiple insurance claims in batch (admin only).
    ///
    /// Allows efficient approval/payment of multiple claims at once.
    /// Emits `("claims_processed",)` with data `(approved_count, paid_count)`.
    pub fn insurance_process_claims_batch(
        env: Env,
        admin: Address,
        claim_ids: Vec<u64>,
        action: String,
    ) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        let mut approved_count: u32 = 0;
        let mut paid_count: u32 = 0;

        for i in 0..claim_ids.len() {
            let claim_id = claim_ids.get(i).unwrap();
            let claim: InsuranceClaim = env
                .storage()
                .persistent()
                .get(&DataKey::Insurance(InsuranceKey::Claim(claim_id)))
                .unwrap_or_else(|| panic_with_error!(&env, InsuranceError::ClaimNotFound));

            if action == String::from_str(&env, "approve") {
                if claim.status == ClaimStatus::Pending {
                    let mut updated_claim = claim.clone();
                    updated_claim.status = ClaimStatus::Approved;
                    updated_claim.updated_at = env.ledger().timestamp();
                    env.storage()
                        .persistent()
                        .set(&DataKey::Insurance(InsuranceKey::Claim(claim_id)), &claim);

                    // Update pool active claims
                    let pool_key = DataKey::Insurance(InsuranceKey::Token(claim.token.clone()));
                    let pool: InsurancePool = env.storage().persistent().get(&pool_key).unwrap();
                    let mut updated_pool = pool.clone();
                    updated_pool.active_claims -= 1;
                    env.storage().persistent().set(&pool_key, &updated_pool);

                    // Update creator active claims
                    let active_key = DataKey::Insurance(InsuranceKey::ActiveClms(
                        claim.creator.clone(),
                        claim.token.clone(),
                    ));
                    let active_claims: u32 =
                        env.storage().persistent().get(&active_key).unwrap_or(1);
                    env.storage()
                        .persistent()
                        .set(&active_key, &(active_claims - 1));

                    approved_count += 1;
                }
            } else if action == String::from_str(&env, "pay") {
                if claim.status == ClaimStatus::Approved {
                    let pool_key = DataKey::Insurance(InsuranceKey::Token(claim.token.clone()));
                    let pool: InsurancePool = env.storage().persistent().get(&pool_key).unwrap();

                    if claim.amount <= pool.total_reserves {
                        let mut updated_pool = pool.clone();
                        updated_pool.total_reserves -= claim.amount;
                        updated_pool.total_claims_paid += claim.amount;
                        updated_pool.active_claims -= 1;
                        updated_pool.last_payout_time = env.ledger().timestamp();
                        env.storage().persistent().set(&pool_key, &updated_pool);

                        let mut updated_claim = claim.clone();
                        updated_claim.status = ClaimStatus::Paid;
                        updated_claim.updated_at = env.ledger().timestamp();
                        env.storage()
                            .persistent()
                            .set(&DataKey::Insurance(InsuranceKey::Claim(claim_id)), &claim);

                        // Update creator last claim time
                        let last_claim_key = DataKey::Insurance(InsuranceKey::LastClm(
                            claim.creator.clone(),
                            claim.token.clone(),
                        ));
                        env.storage()
                            .persistent()
                            .set(&last_claim_key, &env.ledger().timestamp());

                        // Update creator active claims
                        let active_key = DataKey::Insurance(InsuranceKey::ActiveClms(
                            claim.creator.clone(),
                            claim.token.clone(),
                        ));
                        let active_claims: u32 =
                            env.storage().persistent().get(&active_key).unwrap_or(1);
                        env.storage()
                            .persistent()
                            .set(&active_key, &(active_claims - 1));

                        // Transfer funds
                        token::Client::new(&env, &claim.token).transfer(
                            &env.current_contract_address(),
                            &claim.creator,
                            &claim.amount,
                        );

                        paid_count += 1;
                    }
                }
            }
        }

        env.events()
            .publish((symbol_short!("clm_pro"),), (approved_count, paid_count));
    }

    /// Get all insurance claims for a specific creator and token.
    ///
    /// Returns a vector of claim IDs for the creator's claims.
    pub fn insurance_get_claims_by_creator(env: Env, creator: Address, token: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::Insurance(InsuranceKey::Clms(creator, token)))
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ── cross-chain bridge ───────────────────────────────────────────────────

    /// Sets the authorized bridge relayer and bridge token. Admin only.
    ///
    /// Emits `("bridge_cfg",)` with data `(relayer, token)`.
    pub fn set_bridge_relayer(env: Env, admin: Address, relayer: Address, token: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        env.storage()
            .instance()
            .set(&DataKey::Bridge(BridgeKey::Relayer), &relayer);
        env.storage()
            .instance()
            .set(&DataKey::Bridge(BridgeKey::Token), &token);
        env.storage()
            .instance()
            .set(&DataKey::Bridge(BridgeKey::Enabled), &true);
        env.events()
            .publish((symbol_short!("br_cfg"),), (relayer, token));
    }

    /// Processes a bridged tip submitted by an authorized relayer.
    ///
    /// Validates the tip, deducts bridge fees, transfers funds from the relayer
    /// into contract escrow, and credits the creator's balance.
    /// Emits `("bridge", creator)` with data `(source_chain, source_tx_hash, amount, fee)`.
    pub fn bridge_tip(env: Env, relayer: Address, tip: bridge::BridgeTip) {
        Self::require_not_paused(&env);
        if let Err(e) = bridge::relayer::process_bridge_tip(&env, &relayer, &tip) {
            panic_with_error!(&env, e);
        }
    }

    /// Sets the bridge fee in basis points. Admin only.
    ///
    /// Maximum fee is 500 bps (5%).
    /// Emits `("bridge_fee",)` with data `fee_bps`.
    pub fn set_bridge_fee(env: Env, admin: Address, fee_bps: u32) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        if fee_bps > 500 {
            panic_with_error!(&env, AuctionError::InvalidBridgeFee);
        }
        env.storage()
            .instance()
            .set(&DataKey::Bridge(BridgeKey::FeeBps), &fee_bps);
        env.events().publish((symbol_short!("br_fee"),), fee_bps);
    }

    /// Returns the current bridge fee in basis points.
    pub fn get_bridge_fee(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::Bridge(BridgeKey::FeeBps))
            .unwrap_or(0)
    }

    /// Enables or disables the bridge feature. Admin only.
    ///
    /// Emits `("bridge_en",)` with data `enabled`.
    pub fn enable_bridge(env: Env, admin: Address, enabled: bool) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        env.storage()
            .instance()
            .set(&DataKey::Bridge(BridgeKey::Enabled), &enabled);
        env.events().publish((symbol_short!("br_en"),), enabled);
    }

    /// Returns `true` if the bridge feature is enabled.
    pub fn is_bridge_enabled(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Bridge(BridgeKey::Enabled))
            .unwrap_or(false)
    }

    // ── options trading ──────────────────────────────────────────────────────

    /// Initialize options trading system with default pricing parameters.
    ///
    /// Admin only. Emits `("opt_init",)`.
    pub fn init_options_trading(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        let params = options::PricingParams {
            volatility_bps: options::DEFAULT_VOLATILITY_BPS,
            risk_free_rate_bps: options::DEFAULT_RISK_FREE_RATE_BPS,
            min_premium_bps: options::DEFAULT_MIN_PREMIUM_BPS,
            max_premium_bps: options::DEFAULT_MAX_PREMIUM_BPS,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Option(OptionKey::PricingParams), &params);

        env.storage()
            .instance()
            .set(&DataKey::Option(OptionKey::Ctr), &0u64);

        env.events().publish((symbol_short!("opt_init"),), ());
    }

    /// Write (create) a new option contract.
    ///
    /// Writer must provide collateral which is locked until expiration or exercise.
    /// Returns the option ID.
    /// Emits `("opt_write",)` with data `(option_id, writer, option_type, strike_price, amount, expiration)`.
    pub fn write_option(
        env: Env,
        writer: Address,
        option_type: options::OptionType,
        token: Address,
        strike_price: i128,
        amount: i128,
        expiration: u64,
    ) -> u64 {
        Self::require_not_paused(&env);
        writer.require_auth();

        // Validate inputs
        if strike_price <= 0 || amount <= 0 {
            panic_with_error!(&env, AuctionError::InvalidOptionParams);
        }

        let now = env.ledger().timestamp();
        if expiration <= now {
            panic_with_error!(&env, AuctionError::InvalidOptionParams);
        }

        // Check token is whitelisted
        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        // Calculate required collateral
        let collateral = options::calculate_collateral(option_type, strike_price, amount);

        // Generate option ID
        let option_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::Option(OptionKey::Ctr))
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::Option(OptionKey::Ctr), &(option_id + 1));

        // Create option contract
        let option = options::OptionContract {
            option_id,
            option_type,
            writer: writer.clone(),
            holder: None,
            token: token.clone(),
            strike_price,
            premium: 0,
            amount,
            expiration,
            created_at: now,
            status: options::OptionStatus::Active,
            collateral,
        };

        // Lock collateral
        token::Client::new(&env, &token).transfer(
            &writer,
            &env.current_contract_address(),
            &collateral,
        );

        // Update storage
        env.storage()
            .persistent()
            .set(&DataKey::Option(OptionKey::Record(option_id)), &option);

        options::add_written_option(&env, &writer, option_id);
        options::add_active_option(&env, option_id);

        // Update writer position
        let mut position = options::get_position(&env, &writer);
        position.written_count += 1;
        position.total_collateral += collateral;
        options::update_position(&env, &position);

        // Update locked collateral tracking
        let current_locked = options::get_locked_collateral(&env, &writer, &token);
        options::update_locked_collateral(&env, &writer, &token, current_locked + collateral);

        env.events().publish(
            (symbol_short!("opt_wrt"),),
            (
                option_id,
                writer,
                option_type,
                strike_price,
                amount,
                expiration,
            ),
        );

        option_id
    }

    /// Buy an option by paying the premium to the writer.
    ///
    /// Premium is calculated based on current pricing parameters.
    /// Emits `("opt_buy",)` with data `(option_id, buyer, premium)`.
    pub fn buy_option(env: Env, buyer: Address, option_id: u64, spot_price: i128) {
        Self::require_not_paused(&env);
        buyer.require_auth();

        // Get option
        let mut option: options::OptionContract = env
            .storage()
            .persistent()
            .get(&DataKey::Option(OptionKey::Record(option_id)))
            .unwrap_or_else(|| panic_with_error!(&env, AuctionError::OptionNotFound));

        // Verify option is available
        if option.holder.is_some() {
            panic_with_error!(&env, AuctionError::OptionAlreadySold);
        }

        if option.status != options::OptionStatus::Active {
            panic_with_error!(&env, AuctionError::OptionNotActive);
        }

        let now = env.ledger().timestamp();
        if now >= option.expiration {
            panic_with_error!(&env, AuctionError::OptionExpired);
        }

        // Calculate premium
        let params = options::get_pricing_params(&env);
        let time_to_expiry = option.expiration.saturating_sub(now);
        let premium = options::pricing::calculate_premium(
            &env,
            option.option_type,
            spot_price,
            option.strike_price,
            option.amount,
            time_to_expiry,
            &params,
        );

        // Transfer premium from buyer to writer
        token::Client::new(&env, &option.token).transfer(&buyer, &option.writer, &premium);

        // Update option
        option.holder = Some(buyer.clone());
        option.premium = premium;
        env.storage()
            .persistent()
            .set(&DataKey::Option(OptionKey::Record(option_id)), &option);

        // Update positions
        let mut writer_position = options::get_position(&env, &option.writer);
        writer_position.premiums_earned += premium;
        options::update_position(&env, &writer_position);

        let mut buyer_position = options::get_position(&env, &buyer);
        buyer_position.held_count += 1;
        buyer_position.premiums_paid += premium;
        options::update_position(&env, &buyer_position);

        // Add to buyer's held options
        options::add_held_option(&env, &buyer, option_id);

        env.events()
            .publish((symbol_short!("opt_buy"),), (option_id, buyer, premium));
    }

    /// Exercise an option contract.
    ///
    /// Only the holder can exercise. Option must be in the money.
    /// Returns the payoff amount.
    /// Emits `("opt_exer",)` with data `(option_id, holder, payoff)`.
    pub fn exercise_option(env: Env, holder: Address, option_id: u64, spot_price: i128) -> i128 {
        Self::require_not_paused(&env);
        holder.require_auth();

        let payoff = options::exercise::exercise_option(&env, &holder, option_id, spot_price);

        env.events()
            .publish((symbol_short!("opt_exer"),), (option_id, holder, payoff));

        payoff
    }

    /// Expire an option that has passed its expiration time.
    ///
    /// Returns collateral to writer. Can be called by anyone.
    /// Emits `("opt_exp",)` with data `option_id`.
    pub fn expire_option(env: Env, option_id: u64) {
        options::exercise::expire_option(&env, option_id);

        env.events().publish((symbol_short!("opt_exp"),), option_id);
    }

    /// Cancel an unsold option (writer only).
    ///
    /// Returns collateral to writer. Only works if option has no holder yet.
    /// Emits `("opt_canc",)` with data `option_id`.
    pub fn cancel_option(env: Env, writer: Address, option_id: u64) {
        Self::require_not_paused(&env);
        writer.require_auth();

        options::exercise::cancel_option(&env, &writer, option_id);

        env.events()
            .publish((symbol_short!("opt_canc"),), option_id);
    }

    /// Get option contract details by ID.
    pub fn get_option(env: Env, option_id: u64) -> Option<options::OptionContract> {
        options::get_option(&env, option_id)
    }

    /// Get all options written by an address.
    pub fn get_written_options(env: Env, writer: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::Option(OptionKey::Written(writer)))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Get all options held by an address.
    pub fn get_held_options(env: Env, holder: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::Option(OptionKey::Held(holder)))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Get position summary for an address.
    pub fn get_option_position(env: Env, address: Address) -> options::OptionPosition {
        options::get_position(&env, &address)
    }

    /// Get all active options.
    pub fn get_active_options(env: Env) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::Option(OptionKey::Active))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Calculate option premium for given parameters.
    ///
    /// Useful for price discovery before writing an option.
    pub fn calculate_option_premium(
        env: Env,
        option_type: options::OptionType,
        spot_price: i128,
        strike_price: i128,
        amount: i128,
        time_to_expiry: u64,
    ) -> i128 {
        let params = options::get_pricing_params(&env);
        options::pricing::calculate_premium(
            &env,
            option_type,
            spot_price,
            strike_price,
            amount,
            time_to_expiry,
            &params,
        )
    }

    /// Update option pricing parameters (admin only).
    ///
    /// Emits `("opt_prm",)` with data `params`.
    pub fn update_option_pricing(env: Env, admin: Address, params: options::PricingParams) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        options::update_pricing_params(&env, &params);

        env.events().publish((symbol_short!("opt_prm"),), params);
    }

    /// Get current option pricing parameters.
    pub fn get_option_pricing_params(env: Env) -> options::PricingParams {
        options::get_pricing_params(&env)
    }

    /// Batch expire multiple options.
    ///
    /// Useful for cleaning up expired options. Returns count of expired options.
    /// Emits `("opt_bexp",)` with data `expired_count`.
    pub fn batch_expire_options(env: Env, option_ids: Vec<u64>) -> u32 {
        let mut ids_vec: soroban_sdk::Vec<u64> = soroban_sdk::Vec::new(&env);
        for i in 0..option_ids.len() {
            ids_vec.push_back(option_ids.get(i).unwrap());
        }

        let mut expired_count = 0u32;
        for i in 0..ids_vec.len() {
            let option_id = ids_vec.get(i).unwrap();
            if let Some(option) = options::get_option(&env, option_id) {
                if option.status == options::OptionStatus::Active {
                    let now = env.ledger().timestamp();
                    if now >= option.expiration {
                        options::exercise::expire_option(&env, option_id);
                        expired_count += 1;
                    }
                }
            }
        }

        env.events()
            .publish((symbol_short!("opt_bexp"),), expired_count);

        expired_count
    }

    // ── index funds ──────────────────────────────────────────────────────────

    /// Create a new tip index fund with a basket of creators.
    /// `components` must have at least 2 entries and weights must sum to 10_000 bps.
    pub fn create_index_fund(
        env: Env,
        manager: Address,
        token: Address,
        name: String,
        components: Vec<index_fund::IndexComponent>,
    ) -> u64 {
        manager.require_auth();
        Self::require_not_paused(&env);

        if components.len() < 2 {
            panic_with_error!(&env, TipJarError::IndexFundTooFewCreators);
        }
        if !index_fund::composition::validate_weights(&components) {
            panic_with_error!(&env, TipJarError::InvalidIndexWeights);
        }

        let fund_id =
            index_fund::composition::create_index_fund(&env, &manager, &token, name, components);

        env.events()
            .publish((symbol_short!("idx_new"),), (manager, fund_id));

        fund_id
    }

    /// Update the composition of an existing index fund (manager only).
    pub fn update_index_composition(
        env: Env,
        caller: Address,
        fund_id: u64,
        new_components: Vec<index_fund::IndexComponent>,
    ) {
        caller.require_auth();
        Self::require_not_paused(&env);

        if new_components.len() < 2 {
            panic_with_error!(&env, TipJarError::IndexFundTooFewCreators);
        }
        if !index_fund::composition::validate_weights(&new_components) {
            panic_with_error!(&env, TipJarError::InvalidIndexWeights);
        }

        index_fund::composition::update_composition(&env, fund_id, &caller, new_components);

        env.events()
            .publish((symbol_short!("idx_upd"),), (caller, fund_id));
    }

    /// Rebalance a fund's creator allocations to match current target weights.
    pub fn rebalance_index_fund(env: Env, caller: Address, fund_id: u64) {
        caller.require_auth();
        Self::require_not_paused(&env);

        index_fund::rebalance::rebalance(&env, fund_id, &caller);

        env.events()
            .publish((symbol_short!("idx_reb"),), (caller, fund_id));
    }

    /// Deposit tokens into an index fund and receive shares.
    /// Returns the number of shares minted.
    pub fn deposit_index_fund(env: Env, depositor: Address, fund_id: u64, amount: i128) -> i128 {
        Self::require_not_paused(&env);

        if amount < index_fund::MIN_DEPOSIT {
            panic_with_error!(&env, TipJarError::IndexDepositTooSmall);
        }

        let shares = index_fund::shares::deposit(&env, fund_id, &depositor, amount);

        env.events().publish(
            (symbol_short!("idx_dep"),),
            (depositor, fund_id, amount, shares),
        );

        shares
    }

    /// Withdraw from an index fund by redeeming shares.
    /// Returns the token amount returned to the holder.
    pub fn withdraw_index_fund(env: Env, holder: Address, fund_id: u64, shares: i128) -> i128 {
        Self::require_not_paused(&env);

        if shares <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }

        let amount_out = index_fund::shares::withdraw(&env, fund_id, &holder, shares);

        env.events().publish(
            (symbol_short!("idx_wdr"),),
            (holder, fund_id, shares, amount_out),
        );

        amount_out
    }

    /// Get the current NAV (net asset value) per share for a fund.
    pub fn get_index_fund_nav(env: Env, fund_id: u64) -> i128 {
        index_fund::shares::get_nav(&env, fund_id)
    }

    /// Get the share balance of a holder in a fund.
    pub fn get_index_fund_shares(env: Env, fund_id: u64, holder: Address) -> i128 {
        index_fund::shares::get_shares(&env, fund_id, &holder)
    }

    /// Get the current creator allocations for a fund.
    pub fn get_index_fund_allocations(env: Env, fund_id: u64) -> Vec<(Address, i128)> {
        index_fund::rebalance::get_allocations(&env, fund_id)
    }

    // ── prediction markets ───────────────────────────────────────────────────

    /// Set the platform fee for prediction markets. Admin only.
    ///
    /// `fee_bps` must be ≤ 1000 (10 %). Emits `("pm_fee",)`.
    pub fn set_pred_market_fee(env: Env, admin: Address, fee_bps: u32) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        if fee_bps > 1000 {
            panic_with_error!(&env, TipJarError::FeeExceedsMaximum);
        }
        prediction_market::set_fee_bps(&env, fee_bps);
        env.events().publish((symbol_short!("pm_fee"),), fee_bps);
    }

    /// Create a new prediction market for a creator success metric.
    ///
    /// - `creator`    – the creator whose metric is being predicted.
    /// - `resolver`   – address authorised to settle the market.
    /// - `question`   – human-readable description of the prediction.
    /// - `token`      – token used for betting.
    /// - `closes_at`  – unix timestamp after which no new bets are accepted.
    /// - `resolves_at`– unix timestamp by which the resolver must settle.
    ///
    /// Returns the new market ID. Emits `("pm_create",)`.
    pub fn create_pred_market(
        env: Env,
        creator: Address,
        resolver: Address,
        question: String,
        token: Address,
        closes_at: u64,
        resolves_at: u64,
    ) -> u64 {
        Self::require_not_paused(&env);
        creator.require_auth();

        let now = env.ledger().timestamp();
        if closes_at <= now {
            panic_with_error!(&env, TipJarError::InvalidUnlockTime);
        }
        if resolves_at < closes_at {
            panic_with_error!(&env, TipJarError::InvalidUnlockTime);
        }
        if !env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false)
        {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        let market_id = prediction_market::create_market(
            &env,
            &creator,
            &resolver,
            question.clone(),
            &token,
            closes_at,
            resolves_at,
        );

        env.events().publish(
            (symbol_short!("pm_create"),),
            (market_id, creator, resolver, token, closes_at, resolves_at),
        );

        market_id
    }

    /// Place a bet on a prediction market outcome.
    ///
    /// Transfers `amount` tokens from `bettor` to the contract.
    /// Emits `("pm_bet",)`.
    pub fn place_pred_bet(
        env: Env,
        bettor: Address,
        market_id: u64,
        outcome: prediction_market::Outcome,
        amount: i128,
    ) {
        Self::require_not_paused(&env);
        bettor.require_auth();

        if amount < prediction_market::MIN_BET_AMOUNT {
            panic_with_error!(&env, TipJarError::PredBetTooSmall);
        }

        let market = prediction_market::get_market(&env, market_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::PredMarketNotFound));

        if market.status != prediction_market::MarketStatus::Open {
            panic_with_error!(&env, TipJarError::PredMarketNotOpen);
        }
        if env.ledger().timestamp() >= market.closes_at {
            panic_with_error!(&env, TipJarError::PredMarketClosed);
        }

        prediction_market::place_bet(&env, &bettor, market_id, outcome, amount);

        env.events().publish(
            (symbol_short!("pm_bet"),),
            (market_id, bettor, outcome, amount),
        );
    }

    /// Close the betting window of a prediction market.
    ///
    /// Can be called by the resolver at any time, or by anyone once `closes_at` has passed.
    /// Emits `("pm_close",)`.
    pub fn close_pred_market(env: Env, caller: Address, market_id: u64) {
        caller.require_auth();

        let market = prediction_market::get_market(&env, market_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::PredMarketNotFound));

        if market.status != prediction_market::MarketStatus::Open {
            panic_with_error!(&env, TipJarError::PredMarketNotOpen);
        }

        prediction_market::close_market(&env, &caller, market_id);

        env.events()
            .publish((symbol_short!("pm_close"),), market_id);
    }

    /// Resolve a prediction market with the winning outcome.
    ///
    /// Only the designated resolver may call this. Emits `("pm_resolve",)`.
    pub fn resolve_pred_market(
        env: Env,
        resolver: Address,
        market_id: u64,
        winning_outcome: prediction_market::Outcome,
    ) {
        resolver.require_auth();

        let market = prediction_market::get_market(&env, market_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::PredMarketNotFound));

        if market.status == prediction_market::MarketStatus::Resolved
            || market.status == prediction_market::MarketStatus::Cancelled
        {
            panic_with_error!(&env, TipJarError::PredMarketAlreadySettled);
        }
        if resolver != market.resolver {
            panic_with_error!(&env, TipJarError::PredMarketNotResolver);
        }

        prediction_market::resolve_market(&env, &resolver, market_id, winning_outcome);

        env.events()
            .publish((symbol_short!("pm_res"),), (market_id, winning_outcome));
    }

    /// Cancel a prediction market and enable full refunds.
    ///
    /// Callable by the resolver or the contract admin. Emits `("pm_cancel",)`.
    pub fn cancel_pred_market(env: Env, caller: Address, market_id: u64) {
        caller.require_auth();

        let market = prediction_market::get_market(&env, market_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::PredMarketNotFound));

        if market.status == prediction_market::MarketStatus::Resolved
            || market.status == prediction_market::MarketStatus::Cancelled
        {
            panic_with_error!(&env, TipJarError::PredMarketAlreadySettled);
        }

        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        prediction_market::cancel_market(&env, &caller, market_id, &admin);

        env.events()
            .publish((symbol_short!("pm_cancel"),), market_id);
    }

    /// Claim winnings (or a refund for a cancelled market).
    ///
    /// Returns the payout amount. Emits `("pm_claim",)`.
    pub fn claim_pred_winnings(env: Env, bettor: Address, market_id: u64) -> i128 {
        bettor.require_auth();

        let market = prediction_market::get_market(&env, market_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::PredMarketNotFound));

        if market.status != prediction_market::MarketStatus::Resolved
            && market.status != prediction_market::MarketStatus::Cancelled
        {
            panic_with_error!(&env, TipJarError::PredMarketNotSettled);
        }

        let position = prediction_market::get_bettor_position(&env, market_id, &bettor);
        if position.yes_amount == 0 && position.no_amount == 0 {
            panic_with_error!(&env, TipJarError::PredNoPosition);
        }
        if position.settled {
            panic_with_error!(&env, TipJarError::PredAlreadyClaimed);
        }

        let payout = prediction_market::claim_winnings(&env, &bettor, market_id);

        env.events()
            .publish((symbol_short!("pm_claim"),), (market_id, bettor, payout));

        payout
    }

    // ── prediction market queries ────────────────────────────────────────────

    /// Get a prediction market by ID.
    pub fn get_pred_market(
        env: Env,
        market_id: u64,
    ) -> Option<prediction_market::PredictionMarket> {
        prediction_market::get_market(&env, market_id)
    }

    /// Get the current odds for a market as `(yes_probability, no_probability)`
    /// both scaled by `ODDS_PRECISION` (1_000_000 = 100 %).
    pub fn get_pred_market_odds(env: Env, market_id: u64) -> (i128, i128) {
        let market = prediction_market::get_market(&env, market_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::PredMarketNotFound));
        prediction_market::odds::market_odds(&market)
    }

    /// Get the payout multiplier for a specific outcome, scaled by `ODDS_PRECISION`.
    pub fn get_pred_market_multiplier(
        env: Env,
        market_id: u64,
        outcome: prediction_market::Outcome,
    ) -> i128 {
        let market = prediction_market::get_market(&env, market_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::PredMarketNotFound));
        prediction_market::odds::payout_multiplier(&market, outcome)
    }

    /// Get a bettor's position in a market.
    pub fn get_pred_bettor_position(
        env: Env,
        market_id: u64,
        bettor: Address,
    ) -> prediction_market::BettorPosition {
        prediction_market::get_bettor_position(&env, market_id, &bettor)
    }

    /// Get all market IDs for a creator.
    pub fn get_creator_pred_markets(env: Env, creator: Address) -> Vec<u64> {
        prediction_market::get_creator_markets(&env, &creator)
    }

    /// Get all market IDs a bettor has participated in.
    pub fn get_bettor_pred_markets(env: Env, bettor: Address) -> Vec<u64> {
        prediction_market::get_bettor_markets(&env, &bettor)
    }

    /// Get all currently active (open) market IDs.
    pub fn get_active_pred_markets(env: Env) -> Vec<u64> {
        prediction_market::get_active_markets(&env)
    }

    // ── futures contracts ────────────────────────────────────────────────────

    /// Update the global futures configuration. Admin only.
    ///
    /// `initial_margin_bps` must be > `maintenance_margin_bps`.
    /// Emits `("ft_cfg",)`.
    pub fn set_futures_config(
        env: Env,
        admin: Address,
        initial_margin_bps: u32,
        maintenance_margin_bps: u32,
        liquidation_penalty_bps: u32,
    ) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        if maintenance_margin_bps >= initial_margin_bps {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        let cfg = futures::FuturesConfig {
            initial_margin_bps,
            maintenance_margin_bps,
            liquidation_penalty_bps,
        };
        futures::save_config(&env, &cfg);
        env.events().publish(
            (symbol_short!("ft_cfg"),),
            (initial_margin_bps, maintenance_margin_bps),
        );
    }

    /// Open a new futures contract (long side).
    ///
    /// The caller posts initial margin and specifies the contract price, size,
    /// and settlement date. Returns the new contract ID.
    /// Emits `("ft_open",)`.
    pub fn open_futures(
        env: Env,
        long_party: Address,
        token: Address,
        contract_price: i128,
        size: i128,
        settles_at: u64,
    ) -> u64 {
        Self::require_not_paused(&env);
        long_party.require_auth();

        if contract_price <= 0 {
            panic_with_error!(&env, TipJarError::FuturesInvalidPrice);
        }
        if size < futures::MIN_CONTRACT_SIZE {
            panic_with_error!(&env, TipJarError::FuturesSizeTooSmall);
        }
        if settles_at <= env.ledger().timestamp() {
            panic_with_error!(&env, TipJarError::InvalidUnlockTime);
        }
        if !env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false)
        {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        let contract_id =
            futures::open_long(&env, &long_party, &token, contract_price, size, settles_at);

        env.events().publish(
            (symbol_short!("ft_open"),),
            (
                contract_id,
                long_party,
                token,
                contract_price,
                size,
                settles_at,
            ),
        );

        contract_id
    }

    /// Match the short side of an existing unmatched futures contract.
    ///
    /// The short party posts initial margin equal to the long party's margin.
    /// Emits `("ft_match",)`.
    pub fn match_futures(env: Env, short_party: Address, contract_id: u64) {
        Self::require_not_paused(&env);
        short_party.require_auth();

        let fc = futures::get_contract(&env, contract_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::FuturesNotFound));

        if fc.status != futures::FuturesStatus::Active {
            panic_with_error!(&env, TipJarError::FuturesNotActive);
        }
        if fc.short_party.is_some() {
            panic_with_error!(&env, TipJarError::FuturesAlreadyMatched);
        }

        futures::match_short(&env, &short_party, contract_id);

        env.events()
            .publish((symbol_short!("ft_match"),), (contract_id, short_party));
    }

    /// Update the mark price for a futures contract (oracle / admin only).
    ///
    /// Recalculates unrealised P&L for both sides.
    /// Emits `("ft_mark",)`.
    pub fn update_futures_mark_price(env: Env, admin: Address, contract_id: u64, new_price: i128) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        if new_price <= 0 {
            panic_with_error!(&env, TipJarError::FuturesInvalidPrice);
        }

        let fc = futures::get_contract(&env, contract_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::FuturesNotFound));
        if fc.status != futures::FuturesStatus::Active {
            panic_with_error!(&env, TipJarError::FuturesNotActive);
        }

        futures::update_mark_price(&env, contract_id, new_price);

        env.events()
            .publish((symbol_short!("ft_mark"),), (contract_id, new_price));
    }

    /// Add margin to a futures position to avoid liquidation.
    ///
    /// `side` must match the caller's role in the contract.
    /// Emits `("ft_margin",)`.
    pub fn add_futures_margin(
        env: Env,
        trader: Address,
        contract_id: u64,
        side: futures::Side,
        amount: i128,
    ) {
        Self::require_not_paused(&env);
        trader.require_auth();

        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }

        let fc = futures::get_contract(&env, contract_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::FuturesNotFound));
        if fc.status != futures::FuturesStatus::Active {
            panic_with_error!(&env, TipJarError::FuturesNotActive);
        }

        futures::add_margin(&env, &trader, contract_id, side, amount);

        env.events()
            .publish((symbol_short!("ft_margin"),), (contract_id, trader, amount));
    }

    /// Liquidate an under-margined futures position.
    ///
    /// Any caller may trigger liquidation when a side's effective margin
    /// falls below the maintenance margin. The liquidator receives the
    /// liquidation penalty. Returns the penalty paid.
    /// Emits `("ft_liq",)`.
    pub fn liquidate_futures(env: Env, liquidator: Address, contract_id: u64) -> i128 {
        liquidator.require_auth();

        let fc = futures::get_contract(&env, contract_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::FuturesNotFound));

        if fc.status != futures::FuturesStatus::Active {
            panic_with_error!(&env, TipJarError::FuturesNotActive);
        }
        if fc.short_party.is_none() {
            panic_with_error!(&env, TipJarError::FuturesNotMatched);
        }

        // Verify at least one side is actually liquidatable
        if !futures::margin::long_is_liquidatable(&fc)
            && !futures::margin::short_is_liquidatable(&fc)
        {
            panic_with_error!(&env, TipJarError::FuturesPositionHealthy);
        }

        let penalty = futures::liquidate(&env, &liquidator, contract_id);

        env.events().publish(
            (symbol_short!("ft_liq"),),
            (contract_id, liquidator, penalty),
        );

        penalty
    }

    /// Mark a futures contract as pending settlement once its date has passed.
    ///
    /// Callable by anyone after `settles_at`. Emits `("ft_pend",)`.
    pub fn mark_futures_pending(env: Env, contract_id: u64) {
        let fc = futures::get_contract(&env, contract_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::FuturesNotFound));

        if fc.status != futures::FuturesStatus::Active {
            panic_with_error!(&env, TipJarError::FuturesNotActive);
        }
        if env.ledger().timestamp() < fc.settles_at {
            panic_with_error!(&env, TipJarError::FuturesNotDue);
        }

        futures::settlement::mark_pending(&env, contract_id);

        env.events()
            .publish((symbol_short!("ft_pend"),), contract_id);
    }

    /// Settle a futures contract at the given final price.
    ///
    /// Admin / oracle provides the final settlement price. Both parties
    /// receive their payouts. Returns `(long_payout, short_payout)`.
    /// Emits `("ft_settle",)`.
    pub fn settle_futures(
        env: Env,
        admin: Address,
        contract_id: u64,
        final_price: i128,
    ) -> (i128, i128) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        if final_price <= 0 {
            panic_with_error!(&env, TipJarError::FuturesInvalidPrice);
        }

        let fc = futures::get_contract(&env, contract_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::FuturesNotFound));

        if fc.status == futures::FuturesStatus::Settled
            || fc.status == futures::FuturesStatus::Liquidated
            || fc.status == futures::FuturesStatus::Cancelled
        {
            panic_with_error!(&env, TipJarError::FuturesAlreadyClosed);
        }
        if fc.short_party.is_none() {
            panic_with_error!(&env, TipJarError::FuturesNotMatched);
        }
        if env.ledger().timestamp() < fc.settles_at {
            panic_with_error!(&env, TipJarError::FuturesNotDue);
        }

        let (long_payout, short_payout) =
            futures::settlement::settle(&env, contract_id, final_price);

        env.events().publish(
            (symbol_short!("ft_settle"),),
            (contract_id, final_price, long_payout, short_payout),
        );

        (long_payout, short_payout)
    }

    /// Cancel an unmatched futures contract and refund the long party's margin.
    ///
    /// Only the long party (contract creator) may cancel before a short is matched.
    /// Emits `("ft_cancel",)`.
    pub fn cancel_futures(env: Env, caller: Address, contract_id: u64) {
        caller.require_auth();

        let fc = futures::get_contract(&env, contract_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::FuturesNotFound));

        if fc.status != futures::FuturesStatus::Active {
            panic_with_error!(&env, TipJarError::FuturesNotActive);
        }
        if fc.short_party.is_some() {
            panic_with_error!(&env, TipJarError::FuturesAlreadyMatched);
        }
        if fc.long_party != caller {
            panic_with_error!(&env, TipJarError::FuturesUnauthorized);
        }

        futures::cancel_contract(&env, &caller, contract_id);

        env.events()
            .publish((symbol_short!("ft_cancel"),), contract_id);
    }

    // ── futures queries ──────────────────────────────────────────────────────

    /// Get a futures contract by ID.
    pub fn get_futures_contract(env: Env, contract_id: u64) -> Option<futures::FuturesContract> {
        futures::get_contract(&env, contract_id)
    }

    /// Get the aggregated position summary for a trader.
    pub fn get_futures_position(env: Env, trader: Address) -> futures::FuturesPosition {
        futures::get_position(&env, &trader)
    }

    /// Get all contract IDs for a trader.
    pub fn get_trader_futures(env: Env, trader: Address) -> Vec<u64> {
        futures::get_trader_contracts(&env, &trader)
    }

    /// Get all active futures contract IDs.
    pub fn get_active_futures(env: Env) -> Vec<u64> {
        futures::get_active_contracts(&env)
    }

    /// Compute the expected payout for a side at a hypothetical final price.
    pub fn compute_futures_payout(
        env: Env,
        contract_id: u64,
        final_price: i128,
        side: futures::Side,
    ) -> i128 {
        let fc = futures::get_contract(&env, contract_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::FuturesNotFound));
        futures::settlement::compute_payout(
            fc.long_margin,
            fc.short_margin,
            fc.contract_price,
            fc.size,
            final_price,
            side,
        )
    }

    /// Get the current futures configuration.
    pub fn get_futures_config(env: Env) -> futures::FuturesConfig {
        futures::get_config(&env)
    }

    // ── volatility index ─────────────────────────────────────────────────────

    /// Update the global volatility module configuration. Admin only.
    ///
    /// Emits `("tvi_cfg",)`.
    pub fn set_volatility_config(
        env: Env,
        admin: Address,
        default_window_size: u32,
        min_observation_interval: u64,
    ) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        if default_window_size < volatility::MIN_WINDOW_SIZE
            || default_window_size > volatility::MAX_WINDOW_SIZE
        {
            panic_with_error!(&env, TipJarError::VolInvalidWindow);
        }
        let cfg = volatility::VolatilityConfig {
            default_window_size,
            min_observation_interval,
        };
        volatility::save_config(&env, &cfg);
        env.events().publish(
            (symbol_short!("tvi_cfg"),),
            (default_window_size, min_observation_interval),
        );
    }

    /// Create a new volatility index for a (creator, token) pair.
    ///
    /// `window_size` is the number of observations in the rolling window
    /// (2 – 256). Pass 0 to use the global default.
    /// Returns the new index ID. Emits `("tvi_new",)`.
    pub fn create_volatility_index(
        env: Env,
        creator: Address,
        token: Address,
        window_size: u32,
    ) -> u64 {
        creator.require_auth();

        let effective_window = if window_size == 0 {
            volatility::get_config(&env).default_window_size
        } else {
            window_size
        };

        if effective_window < volatility::MIN_WINDOW_SIZE
            || effective_window > volatility::MAX_WINDOW_SIZE
        {
            panic_with_error!(&env, TipJarError::VolInvalidWindow);
        }
        if !env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false)
        {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        let index_id = volatility::create_index(&env, &creator, &token, effective_window);

        env.events().publish(
            (symbol_short!("tvi_new"),),
            (index_id, creator, token, effective_window),
        );

        index_id
    }

    /// Record a new tip-amount observation on a volatility index.
    ///
    /// Recomputes rolling mean, variance, and volatility in basis points.
    /// Stores a history snapshot. Emits `("tvi_upd",)`.
    pub fn record_volatility_observation(env: Env, index_id: u64, amount: i128) {
        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }

        let idx = volatility::get_index(&env, index_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::VolIndexNotFound));

        if !idx.active {
            panic_with_error!(&env, TipJarError::VolIndexNotActive);
        }

        let cfg = volatility::get_config(&env);
        let now = env.ledger().timestamp();
        if now < idx.last_update + cfg.min_observation_interval {
            panic_with_error!(&env, TipJarError::VolObsTooFrequent);
        }

        let updated = volatility::record_observation(&env, index_id, amount);

        env.events().publish(
            (symbol_short!("tvi_upd"),),
            (
                index_id,
                amount,
                updated.mean,
                updated.variance,
                updated.volatility_bps,
            ),
        );
    }

    /// Deactivate a volatility index. Only the creator may call this.
    ///
    /// Emits `("tvi_off",)`.
    pub fn deactivate_volatility_index(env: Env, creator: Address, index_id: u64) {
        creator.require_auth();

        let idx = volatility::get_index(&env, index_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::VolIndexNotFound));

        if idx.creator != creator {
            panic_with_error!(&env, TipJarError::VolUnauthorized);
        }
        if !idx.active {
            panic_with_error!(&env, TipJarError::VolIndexNotActive);
        }

        volatility::deactivate_index(&env, &creator, index_id);

        env.events().publish((symbol_short!("tvi_off"),), index_id);
    }

    // ── volatility queries ───────────────────────────────────────────────────

    /// Get the current state of a volatility index.
    pub fn get_volatility_index(env: Env, index_id: u64) -> Option<volatility::VolatilityIndex> {
        volatility::get_index(&env, index_id)
    }

    /// Get the most-recent volatility snapshot for an index.
    pub fn get_latest_volatility(
        env: Env,
        index_id: u64,
    ) -> Option<volatility::VolatilitySnapshot> {
        volatility::history::get_latest_snapshot(&env, index_id)
    }

    /// Get up to `limit` most-recent volatility snapshots (newest first).
    pub fn get_volatility_history(
        env: Env,
        index_id: u64,
        limit: u32,
    ) -> Vec<volatility::VolatilitySnapshot> {
        volatility::history::get_recent_snapshots(&env, index_id, limit)
    }

    /// Get volatility snapshots within a time range `[start_ts, end_ts]`.
    pub fn get_volatility_in_range(
        env: Env,
        index_id: u64,
        start_ts: u64,
        end_ts: u64,
        limit: u32,
    ) -> Vec<volatility::VolatilitySnapshot> {
        volatility::history::get_snapshots_in_range(&env, index_id, start_ts, end_ts, limit)
    }

    /// Get all volatility index IDs for a creator.
    pub fn get_creator_volatility_indices(env: Env, creator: Address) -> Vec<u64> {
        volatility::get_creator_indices(&env, &creator)
    }

    /// Get the current observations in the rolling window for an index.
    pub fn get_volatility_window(env: Env, index_id: u64) -> Vec<volatility::VolObservation> {
        let idx = volatility::get_index(&env, index_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::VolIndexNotFound));
        volatility::collect_window(&env, &idx)
    }

    /// Compute max drawdown in basis points for the current window.
    pub fn get_volatility_max_drawdown(env: Env, index_id: u64) -> i128 {
        let idx = volatility::get_index(&env, index_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::VolIndexNotFound));
        let window = volatility::collect_window(&env, &idx);
        volatility::calculator::max_drawdown_bps(&window)
    }

    /// Compute rate of change in basis points across the current window.
    pub fn get_volatility_rate_of_change(env: Env, index_id: u64) -> i128 {
        let idx = volatility::get_index(&env, index_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::VolIndexNotFound));
        let window = volatility::collect_window(&env, &idx);
        volatility::calculator::rate_of_change_bps(&window)
    }

    // ── AMM: pool management ─────────────────────────────────────────────────

    /// Create a new constant-product liquidity pool for a token pair.
    ///
    /// The creator seeds the pool with initial liquidity and receives LP shares.
    /// Returns `(pool_id, shares_minted)`. Emits `("amm_new",)`.
    pub fn amm_create_pool(
        env: Env,
        creator: Address,
        token_a: Address,
        token_b: Address,
        amount_a: i128,
        amount_b: i128,
        fee_bps: u32,
    ) -> (u64, i128) {
        Self::require_not_paused(&env);
        creator.require_auth();

        if !env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::TokenWhitelist(token_a.clone()))
            .unwrap_or(false)
        {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }
        if !env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::TokenWhitelist(token_b.clone()))
            .unwrap_or(false)
        {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        let fee = if fee_bps == 0 {
            amm::DEFAULT_FEE_BPS
        } else {
            fee_bps
        };
        let (pool_id, shares) = amm::pool::create_pool(
            &env,
            &creator,
            &token_a,
            &token_b,
            amount_a,
            amount_b,
            Some(fee),
        );

        env.events().publish(
            (symbol_short!("amm_new"),),
            (pool_id, creator, token_a, token_b, amount_a, amount_b, fee),
        );

        (pool_id, shares)
    }

    /// Add liquidity to an existing AMM pool.
    ///
    /// Deposits are adjusted to maintain the current pool ratio.
    /// `amount_a_min` / `amount_b_min` guard against slippage.
    /// Returns `AddLiquidityResult`. Emits `("amm_add",)`.
    pub fn amm_add_liquidity(
        env: Env,
        provider: Address,
        pool_id: u64,
        amount_a_desired: i128,
        amount_b_desired: i128,
        amount_a_min: i128,
        amount_b_min: i128,
    ) -> amm::AddLiquidityResult {
        Self::require_not_paused(&env);
        provider.require_auth();

        amm::get_pool(&env, pool_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::AmmPoolNotFound));

        let result = amm::pool::add_liquidity(
            &env,
            pool_id,
            &provider,
            amount_a_desired,
            amount_b_desired,
            amount_a_min,
            amount_b_min,
        );

        env.events().publish(
            (symbol_short!("amm_add"),),
            (
                pool_id,
                provider,
                result.amount_a,
                result.amount_b,
                result.shares_minted,
            ),
        );

        result
    }

    /// Remove liquidity from an AMM pool by burning LP shares.
    ///
    /// Automatically claims pending fee rewards.
    /// Returns `RemoveLiquidityResult`. Emits `("amm_rem",)`.
    pub fn amm_remove_liquidity(
        env: Env,
        provider: Address,
        pool_id: u64,
        shares: i128,
        amount_a_min: i128,
        amount_b_min: i128,
    ) -> amm::RemoveLiquidityResult {
        Self::require_not_paused(&env);
        provider.require_auth();

        let result = amm::pool::remove_liquidity(
            &env,
            pool_id,
            &provider,
            shares,
            amount_a_min,
            amount_b_min,
        );

        env.events().publish(
            (symbol_short!("amm_rem"),),
            (
                pool_id,
                provider,
                result.amount_a,
                result.amount_b,
                result.rewards_claimed,
            ),
        );

        result
    }

    // ── AMM: swaps ───────────────────────────────────────────────────────────

    /// Swap an exact input amount for as many output tokens as possible.
    ///
    /// `min_amount_out` enforces slippage tolerance.
    /// Returns `SwapResult`. Emits `("amm_swap",)`.
    pub fn amm_swap(
        env: Env,
        sender: Address,
        pool_id: u64,
        token_in: Address,
        amount_in: i128,
        min_amount_out: i128,
    ) -> amm::SwapResult {
        Self::require_not_paused(&env);
        sender.require_auth();

        if amount_in <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }

        let result = amm::swap::swap(&env, pool_id, &sender, &token_in, amount_in, min_amount_out);

        env.events().publish(
            (symbol_short!("amm_swap"),),
            (
                pool_id,
                sender,
                token_in,
                amount_in,
                result.amount_out,
                result.fee_amount,
            ),
        );

        result
    }

    // ── AMM: rewards ─────────────────────────────────────────────────────────

    /// Claim accumulated swap-fee rewards without removing liquidity.
    ///
    /// Returns the amount of token A transferred. Emits `("amm_clm",)`.
    pub fn amm_claim_rewards(env: Env, provider: Address, pool_id: u64) -> i128 {
        provider.require_auth();

        amm::get_pool(&env, pool_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::AmmPoolNotFound));

        let claimed = amm::pool::claim_rewards(&env, pool_id, &provider);

        env.events()
            .publish((symbol_short!("amm_clm"),), (pool_id, provider, claimed));

        claimed
    }

    /// Update the swap fee for a pool. Admin only. Emits `("amm_fee",)`.
    pub fn amm_set_pool_fee(env: Env, admin: Address, pool_id: u64, fee_bps: u32) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        if fee_bps > amm::MAX_FEE_BPS {
            panic_with_error!(&env, TipJarError::AmmFeeTooHigh);
        }
        amm::pool::set_pool_fee(&env, pool_id, fee_bps);
        env.events()
            .publish((symbol_short!("amm_fee"),), (pool_id, fee_bps));
    }

    // ── AMM: queries ─────────────────────────────────────────────────────────

    /// Get pool state by ID.
    pub fn amm_get_pool(env: Env, pool_id: u64) -> Option<amm::LiquidityPool> {
        amm::get_pool(&env, pool_id)
    }

    /// Look up a pool ID by token pair (order-independent).
    pub fn amm_get_pool_id(env: Env, token_a: Address, token_b: Address) -> Option<u64> {
        amm::get_pool_id_by_tokens(&env, &token_a, &token_b)
    }

    /// Get the expected output for a swap (view, no state change).
    pub fn amm_get_amount_out(env: Env, pool_id: u64, token_in: Address, amount_in: i128) -> i128 {
        amm::swap::get_amount_out(&env, pool_id, &token_in, amount_in)
    }

    /// Get the required input for a desired output (view, no state change).
    pub fn amm_get_amount_in(env: Env, pool_id: u64, token_in: Address, amount_out: i128) -> i128 {
        amm::swap::get_amount_in(&env, pool_id, &token_in, amount_out)
    }

    /// Get the spot price of `token_in` × 1_000_000.
    pub fn amm_spot_price(env: Env, pool_id: u64, token_in: Address) -> i128 {
        amm::pricing::spot_price(&env, pool_id, &token_in)
    }

    /// Get the price impact in basis points for a hypothetical swap.
    pub fn amm_price_impact(env: Env, pool_id: u64, token_in: Address, amount_in: i128) -> i128 {
        amm::pricing::get_price_impact(&env, pool_id, &token_in, amount_in)
    }

    /// Get the LP share balance for a provider.
    pub fn amm_get_shares(env: Env, pool_id: u64, provider: Address) -> i128 {
        amm::pool::get_provider_shares(&env, pool_id, &provider)
    }

    /// Get pending (unclaimed) fee rewards for a provider.
    pub fn amm_get_pending_rewards(env: Env, pool_id: u64, provider: Address) -> i128 {
        amm::pool::get_pending_rewards(&env, pool_id, &provider)
    }

    /// Get the constant-product invariant k = reserve_a × reserve_b.
    pub fn amm_get_invariant(env: Env, pool_id: u64) -> i128 {
        amm::pricing::get_invariant(&env, pool_id)
    }

    /// Get total fees collected by a pool since creation.
    pub fn amm_total_fees(env: Env, pool_id: u64) -> i128 {
        amm::pricing::total_fees_collected(&env, pool_id)
    }

    /// Get LP share value as `(token_a_per_share, token_b_per_share)` × 1_000_000.
    pub fn amm_share_value(env: Env, pool_id: u64) -> (i128, i128) {
        amm::pricing::share_value(&env, pool_id)
    }

    // ── Quadratic Funding ─────────────────────────────────────────────────────

    /// Creates a new quadratic funding round.
    ///
    /// `admin` deposits `matching_pool` tokens immediately into escrow.
    /// The round accepts contributions for `duration_seconds` seconds.
    /// Returns the new round ID.
    ///
    /// Emits `("qf_new",)` with data `(round_id, admin, token, matching_pool)`.
    pub fn qf_create_round(
        env: Env,
        admin: Address,
        token: Address,
        matching_pool: i128,
        duration_seconds: u64,
    ) -> u64 {
        Self::require_not_paused(&env);
        admin.require_auth();
        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }
        quadratic_funding::create_round(&env, &admin, &token, matching_pool, duration_seconds)
    }

    /// Contributes `amount` of the round's token to `project` in `round_id`.
    ///
    /// Each address may contribute at most once per project per round (Sybil resistance).
    /// Emits `("qf_con",)` with data `(round_id, contributor, project, amount)`.
    pub fn qf_contribute(
        env: Env,
        contributor: Address,
        round_id: u64,
        project: Address,
        amount: i128,
    ) {
        Self::require_not_paused(&env);
        contributor.require_auth();
        quadratic_funding::contribute(&env, &contributor, round_id, &project, amount);
    }

    /// Finalizes a round after its end time. Admin only.
    ///
    /// Emits `("qf_fin",)` with data `(round_id,)`.
    pub fn qf_finalize_round(env: Env, admin: Address, round_id: u64) {
        Self::require_not_paused(&env);
        admin.require_auth();
        quadratic_funding::finalize_round(&env, &admin, round_id);
    }

    /// Distributes matching funds to projects and returns contributions to donors.
    ///
    /// Uses the quadratic formula: each project's share ∝ (Σ√contribution_i)².
    /// Admin only; round must be Finalized.
    /// Emits `("qf_dist",)` per project and `("qf_done",)` on completion.
    pub fn qf_distribute_matching(env: Env, admin: Address, round_id: u64) {
        Self::require_not_paused(&env);
        admin.require_auth();
        quadratic_funding::distribute_matching(&env, &admin, round_id);
    }

    /// Returns the funding round record, or `None` if not found.
    pub fn qf_get_round(env: Env, round_id: u64) -> Option<quadratic_funding::FundingRound> {
        quadratic_funding::get_round(&env, round_id)
    }

    /// Returns the contribution record for a specific contributor/project/round.
    pub fn qf_get_contribution(
        env: Env,
        round_id: u64,
        project: Address,
        contributor: Address,
    ) -> Option<quadratic_funding::Contribution> {
        quadratic_funding::get_contribution(&env, round_id, &project, &contributor)
    }

    /// Returns the estimated matching amount for `project` in `round_id`.
    pub fn qf_get_match_estimate(env: Env, round_id: u64, project: Address) -> i128 {
        quadratic_funding::get_match_estimate(&env, round_id, &project)
    }

    // ── Retroactive Public Goods Funding ─────────────────────────────────────

    /// Creates a retroactive funding round.
    ///
    /// Transfers `reward_pool` tokens from `admin` into the contract.
    /// `criteria` weights must sum to 10 000 bps.
    /// Emits `("retro_new",)` with `(round_id, token, reward_pool, end_time)`.
    pub fn retro_create_round(
        env: Env,
        admin: Address,
        token: Address,
        reward_pool: i128,
        start_time: u64,
        end_time: u64,
        criteria: retroactive_funding::EvalCriteria,
    ) -> u64 {
        Self::require_not_paused(&env);
        retroactive_funding::create_round(&env, &admin, &token, reward_pool, start_time, end_time, criteria)
    }

    /// Nominates a creator as eligible for rewards in a round. Admin only.
    ///
    /// Emits `("retro_nom",)` with `(round_id, creator)`.
    pub fn retro_nominate_creator(env: Env, admin: Address, round_id: u64, creator: Address) {
        Self::require_not_paused(&env);
        retroactive_funding::nominate_creator(&env, &admin, round_id, &creator);
    }

    /// Casts a vote for a nominated creator in an active round.
    ///
    /// Each voter may vote once per round. Vote weight is derived from the voter's
    /// own tip history (sqrt-weighted).
    /// Emits `("retro_vot",)` with `(round_id, voter, creator, weight)`.
    pub fn retro_cast_vote(env: Env, voter: Address, round_id: u64, creator: Address) {
        Self::require_not_paused(&env);
        retroactive_funding::cast_vote(&env, &voter, round_id, &creator);
    }

    /// Finalizes a round after the voting window closes. Admin only.
    ///
    /// Emits `("retro_fin",)` with `(round_id, total_votes)`.
    pub fn retro_finalize_round(env: Env, admin: Address, round_id: u64) {
        Self::require_not_paused(&env);
        retroactive_funding::finalize_round(&env, &admin, round_id);
    }

    /// Computes a creator's impact score for a round.
    ///
    /// Score combines historical tips, tip count, and vote weight per `EvalCriteria`.
    pub fn retro_compute_impact_score(env: Env, round_id: u64, creator: Address) -> i128 {
        retroactive_funding::compute_impact_score(&env, round_id, &creator)
    }

    /// Claims the retroactive reward for a creator in a finalized round.
    ///
    /// Reward is proportional to the creator's impact score relative to all nominees.
    /// Emits `("retro_clm",)` with `(round_id, creator, amount)`.
    pub fn retro_claim_reward(env: Env, creator: Address, round_id: u64) -> i128 {
        Self::require_not_paused(&env);
        retroactive_funding::claim_reward(&env, &creator, round_id)
    }

    /// Returns a retroactive funding round by ID.
    pub fn retro_get_round(env: Env, round_id: u64) -> retroactive_funding::RetroRound {
        retroactive_funding::get_round(&env, round_id)
    }

    /// Returns the nominated creators for a round.
    pub fn retro_get_round_creators(env: Env, round_id: u64) -> Vec<Address> {
        retroactive_funding::get_round_creators(&env, round_id)
    }

    /// Returns the aggregated vote weight for a creator in a round.
    pub fn retro_get_creator_votes(env: Env, round_id: u64, creator: Address) -> i128 {
        retroactive_funding::get_creator_votes(&env, round_id, &creator)
    }

    /// Returns whether a voter has already voted in a round.
    pub fn retro_has_voted(env: Env, round_id: u64, voter: Address) -> bool {
        retroactive_funding::has_voted(&env, round_id, &voter)
    }

    // ── Conviction Voting ────────────────────────────────────────────────────

    /// Initialize conviction voting system with default configuration.
    pub fn init_conviction_voting(env: Env) {
        governance::conviction::init_conviction_voting(&env);
    }

    /// Cast a conviction vote on a proposal.
    /// Voting power accumulates over time based on lock duration.
    /// Emits `("conv_vote",)` with data `(voter, proposal_id, effective_voting_power)`.
    pub fn cast_conviction_vote(
        env: Env,
        voter: Address,
        proposal_id: u64,
        choice: governance::VoteChoice,
        base_voting_power: i128,
    ) {
        governance::conviction_integration::cast_conviction_vote(
            &env,
            &voter,
            proposal_id,
            choice,
            base_voting_power,
        );
    }

    /// Change an existing conviction vote (e.g., increase voting power).
    /// Applies decay penalty to accumulated conviction.
    /// Emits `("conv_chg",)` with data `(voter, proposal_id, new_effective_voting_power)`.
    pub fn change_conviction_vote(
        env: Env,
        voter: Address,
        proposal_id: u64,
        new_choice: governance::VoteChoice,
        new_base_voting_power: i128,
    ) {
        governance::conviction_integration::change_conviction_vote(
            &env,
            &voter,
            proposal_id,
            new_choice,
            new_base_voting_power,
        );
    }

    /// Get effective voting power for a voter on a proposal (includes conviction multiplier).
    pub fn get_conviction_voting_power(env: Env, proposal_id: u64, voter: Address) -> i128 {
        governance::conviction_integration::get_effective_voting_power(&env, proposal_id, &voter)
    }

    /// Get detailed conviction voting information for a voter on a proposal.
    pub fn get_conviction_voting_details(
        env: Env,
        proposal_id: u64,
        voter: Address,
    ) -> Option<governance::conviction_integration::ConvictionVotingDetails> {
        governance::conviction_integration::get_conviction_voting_details(&env, proposal_id, &voter)
    }

    /// Get total conviction accumulated by a voter across all proposals.
    pub fn get_voter_total_conviction(env: Env, voter: Address) -> i128 {
        governance::conviction::get_voter_total_conviction(&env, &voter)
    }

    /// Get current conviction voting configuration.
    pub fn get_conviction_config(env: Env) -> governance::conviction::ConvictionConfig {
        governance::conviction::get_conviction_config(&env)
    }

    /// Update conviction voting configuration (admin only).
    pub fn update_conviction_config(env: Env, config: governance::conviction::ConvictionConfig) {
        Self::require_admin(&env);
        governance::conviction::update_conviction_config(&env, &config);
    }

    /// Check if voter can create a proposal based on conviction voting.
    pub fn can_create_proposal_with_conviction(env: Env, voter: Address) -> bool {
        governance::conviction_integration::can_create_proposal_with_conviction(&env, &voter)
    }

    /// Get proposal threshold adjusted by voter's conviction.
    pub fn get_adjusted_proposal_threshold(env: Env, voter: Address) -> i128 {
        governance::conviction_integration::get_adjusted_proposal_threshold(&env, &voter)
    }

    // ── payment channels ─────────────────────────────────────────────────────

    /// Opens a bidirectional payment channel between `party_a` and `party_b`.
    ///
    /// Both parties' deposits are transferred into contract escrow immediately.
    /// `dispute_window` is the seconds a counterparty has to submit a newer state
    /// during a unilateral close.
    ///
    /// Emits `("ch_open",)` with data `(party_a, party_b, token, total_deposit)`.
    pub fn open_channel(
        env: Env,
        party_a: Address,
        party_b: Address,
        token: Address,
        deposit_a: i128,
        deposit_b: i128,
        dispute_window: u64,
    ) {
        Self::require_not_paused(&env);
        party_a.require_auth();
        party_b.require_auth();

        if deposit_a <= 0 || deposit_b <= 0 {
            panic_with_error!(&env, ChannelError::InvalidDeposit);
        }
        if dispute_window == 0 {
            panic_with_error!(&env, ChannelError::InvalidDisputeWindow);
        }

        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        let key = DataKey::PaymentChannel(party_a.clone(), party_b.clone(), token.clone());
        if env.storage().persistent().has(&key) {
            panic_with_error!(&env, ChannelError::ChannelNotOpen);
        }

        let channel = payment_channel::open(
            &env,
            &party_a,
            &party_b,
            &token,
            deposit_a,
            deposit_b,
            dispute_window,
        );

        // CEI: state before external calls
        env.storage().persistent().set(&key, &channel);

        let tok = token::Client::new(&env, &token);
        tok.transfer(&party_a, &env.current_contract_address(), &deposit_a);
        tok.transfer(&party_b, &env.current_contract_address(), &deposit_b);

        env.events().publish(
            (symbol_short!("ch_open"),),
            (party_a, party_b, token, deposit_a + deposit_b),
        );
    }

    /// Updates the channel's latest agreed balance state.
    ///
    /// Both parties must authorise. `new_balance_a` is party_a's new balance;
    /// `nonce` must be strictly greater than the current nonce.
    ///
    /// Emits `("ch_upd",)` with data `(party_a, party_b, token, new_balance_a, nonce)`.
    pub fn update_channel_state(
        env: Env,
        party_a: Address,
        party_b: Address,
        token: Address,
        new_balance_a: i128,
        nonce: u64,
    ) {
        Self::require_not_paused(&env);
        party_a.require_auth();
        party_b.require_auth();

        let key = DataKey::PaymentChannel(party_a.clone(), party_b.clone(), token.clone());
        let mut channel: payment_channel::PaymentChannel = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, ChannelError::ChannelNotFound));

        if channel.status != payment_channel::ChannelStatus::Open {
            panic_with_error!(&env, ChannelError::ChannelNotOpen);
        }
        if nonce <= channel.nonce {
            panic_with_error!(&env, ChannelError::StaleNonce);
        }
        if new_balance_a < 0 || new_balance_a > channel.total_deposit {
            panic_with_error!(&env, ChannelError::InvalidChannelBalance);
        }

        channel.balance_a = new_balance_a;
        channel.nonce = nonce;
        env.storage().persistent().set(&key, &channel);

        env.events().publish(
            (symbol_short!("ch_upd"),),
            (party_a, party_b, token, new_balance_a, nonce),
        );
    }

    /// Cooperatively closes a channel. Both parties must authorise.
    ///
    /// Distributes funds according to the latest agreed state and marks the channel closed.
    /// Emits `("ch_coop",)` with data `(party_a, party_b, token, balance_a, balance_b)`.
    pub fn cooperative_close(env: Env, party_a: Address, party_b: Address, token: Address) {
        Self::require_not_paused(&env);
        party_a.require_auth();
        party_b.require_auth();

        let key = DataKey::PaymentChannel(party_a.clone(), party_b.clone(), token.clone());
        let mut channel: payment_channel::PaymentChannel = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, ChannelError::ChannelNotFound));

        if channel.status != payment_channel::ChannelStatus::Open {
            panic_with_error!(&env, ChannelError::ChannelNotOpen);
        }

        let bal_a = channel.balance_a;
        let bal_b = payment_channel::balance_b(&channel);

        channel.status = payment_channel::ChannelStatus::Closed;
        env.storage().persistent().set(&key, &channel);

        let tok = token::Client::new(&env, &token);
        if bal_a > 0 {
            tok.transfer(&env.current_contract_address(), &party_a, &bal_a);
        }
        if bal_b > 0 {
            tok.transfer(&env.current_contract_address(), &party_b, &bal_b);
        }

        env.events().publish(
            (symbol_short!("ch_coop"),),
            (party_a, party_b, token, bal_a, bal_b),
        );
    }

    /// Initiates or finalises a unilateral (dispute) close.
    ///
    /// **First call** (by either party): sets status to `Disputed` and starts the
    /// dispute window using the caller's proposed `balance_a` / `nonce`.
    ///
    /// **Second call** (by the counterparty, within the window): if the submitted
    /// `nonce` is strictly higher, the newer state is applied before closing.
    ///
    /// **After the window**: anyone may call to finalise the close with the last
    /// submitted state.
    ///
    /// Emits `("ch_disp",)` on initiation and `("ch_fin",)` on finalisation.
    pub fn dispute_close(
        env: Env,
        caller: Address,
        party_a: Address,
        party_b: Address,
        token: Address,
        claimed_balance_a: i128,
        nonce: u64,
    ) {
        Self::require_not_paused(&env);
        caller.require_auth();

        let key = DataKey::PaymentChannel(party_a.clone(), party_b.clone(), token.clone());
        let mut channel: payment_channel::PaymentChannel = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, ChannelError::ChannelNotFound));

        if caller != channel.party_a && caller != channel.party_b {
            panic_with_error!(&env, ChannelError::NotChannelParty);
        }

        let now = env.ledger().timestamp();

        match channel.status {
            payment_channel::ChannelStatus::Open => {
                // Initiate dispute
                if claimed_balance_a < 0 || claimed_balance_a > channel.total_deposit {
                    panic_with_error!(&env, ChannelError::InvalidChannelBalance);
                }
                if nonce < channel.nonce {
                    panic_with_error!(&env, ChannelError::StaleNonce);
                }
                channel.status = payment_channel::ChannelStatus::Disputed;
                channel.dispute_started_at = now;
                channel.disputer = Some(caller.clone());
                channel.balance_a = claimed_balance_a;
                channel.nonce = nonce;
                env.storage().persistent().set(&key, &channel);

                env.events().publish(
                    (symbol_short!("ch_disp"),),
                    (caller, party_a, party_b, token, claimed_balance_a, nonce),
                );
            }
            payment_channel::ChannelStatus::Disputed => {
                let window_end = channel.dispute_started_at + channel.dispute_window;

                if now < window_end {
                    // Counterparty submitting a newer state
                    if nonce <= channel.nonce {
                        panic_with_error!(&env, ChannelError::StaleNonce);
                    }
                    if claimed_balance_a < 0 || claimed_balance_a > channel.total_deposit {
                        panic_with_error!(&env, ChannelError::InvalidChannelBalance);
                    }
                    channel.balance_a = claimed_balance_a;
                    channel.nonce = nonce;
                    env.storage().persistent().set(&key, &channel);

                    env.events().publish(
                        (symbol_short!("ch_disp"),),
                        (caller, party_a, party_b, token, claimed_balance_a, nonce),
                    );
                } else {
                    // Window elapsed — finalise
                    let bal_a = channel.balance_a;
                    let bal_b = payment_channel::balance_b(&channel);

                    channel.status = payment_channel::ChannelStatus::Closed;
                    env.storage().persistent().set(&key, &channel);

                    let tok = token::Client::new(&env, &token);
                    if bal_a > 0 {
                        tok.transfer(&env.current_contract_address(), &party_a, &bal_a);
                    }
                    if bal_b > 0 {
                        tok.transfer(&env.current_contract_address(), &party_b, &bal_b);
                    }

                    env.events().publish(
                        (symbol_short!("ch_fin"),),
                        (party_a, party_b, token, bal_a, bal_b),
                    );
                }
            }
            payment_channel::ChannelStatus::Closed => {
                panic_with_error!(&env, ChannelError::ChannelNotOpen);
            }
        }
    }

    /// Returns the payment channel between `party_a`, `party_b`, and `token`.
    pub fn get_channel(
        env: Env,
        party_a: Address,
        party_b: Address,
        token: Address,
    ) -> Option<payment_channel::PaymentChannel> {
        env.storage()
            .persistent()
            .get(&DataKey::PaymentChannel(party_a, party_b, token))
    }

    // ── tip state channels ───────────────────────────────────────────────────

    /// Opens a tip state channel between `tipper` and `creator`.
    ///
    /// Transfers `deposit` from `tipper` into contract escrow.
    /// Emits `("tch_open",)` with `(tipper, creator, token, deposit)`.
    pub fn open_tip_channel(
        env: Env,
        tipper: Address,
        creator: Address,
        token: Address,
        deposit: i128,
        dispute_window: u64,
    ) {
        Self::require_not_paused(&env);
        tipper.require_auth();

        if deposit <= 0 {
            panic_with_error!(&env, TipChannelError::InvalidDeposit);
        }
        if dispute_window == 0 {
            panic_with_error!(&env, TipChannelError::InvalidDisputeWindow);
        }

        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, TipJarError::TokenNotWhitelisted);
        }

        let key = DataKey::TipChannel(tipper.clone(), creator.clone(), token.clone());
        if env.storage().persistent().has(&key) {
            panic_with_error!(&env, TipChannelError::ChannelNotOpen);
        }

        state_channel::open(&env, &tipper, &creator, &token, deposit, dispute_window);
    }

    /// Records an off-chain tip state update. Both parties must authorise.
    ///
    /// `new_tipped_amount` is the new cumulative total; `nonce` must be strictly greater.
    /// Emits `("tch_tip",)` with `(tipper, creator, token, tip_amount, nonce)`.
    pub fn record_channel_tip(
        env: Env,
        tipper: Address,
        creator: Address,
        token: Address,
        new_tipped_amount: i128,
        nonce: u64,
    ) {
        Self::require_not_paused(&env);
        tipper.require_auth();
        creator.require_auth();

        let key = DataKey::TipChannel(tipper.clone(), creator.clone(), token.clone());
        if !env.storage().persistent().has(&key) {
            panic_with_error!(&env, TipChannelError::ChannelNotFound);
        }

        let channel = state_channel::load(&env, &tipper, &creator, &token);
        if channel.status != state_channel::TipChannelStatus::Open {
            panic_with_error!(&env, TipChannelError::ChannelNotOpen);
        }
        if nonce <= channel.nonce {
            panic_with_error!(&env, TipChannelError::StaleNonce);
        }
        if new_tipped_amount < channel.tipped_amount {
            panic_with_error!(&env, TipChannelError::TippedAmountDecreased);
        }
        if new_tipped_amount > channel.deposit {
            panic_with_error!(&env, TipChannelError::ExceedsDeposit);
        }

        state_channel::record_tip(&env, &tipper, &creator, &token, new_tipped_amount, nonce);
    }

    /// Cooperatively settles a tip channel. Both parties must authorise.
    ///
    /// Distributes `tipped_amount` to creator and remainder to tipper.
    /// Emits `("tch_setl",)` with `(tipper, creator, token, to_creator, to_tipper)`.
    pub fn settle_tip_channel(env: Env, tipper: Address, creator: Address, token: Address) {
        Self::require_not_paused(&env);
        tipper.require_auth();
        creator.require_auth();

        let key = DataKey::TipChannel(tipper.clone(), creator.clone(), token.clone());
        if !env.storage().persistent().has(&key) {
            panic_with_error!(&env, TipChannelError::ChannelNotFound);
        }

        let channel = state_channel::load(&env, &tipper, &creator, &token);
        if channel.status != state_channel::TipChannelStatus::Open {
            panic_with_error!(&env, TipChannelError::ChannelNotOpen);
        }

        state_channel::settle(&env, &tipper, &creator, &token);
    }

    /// Initiates or finalises a unilateral dispute close on a tip channel.
    ///
    /// First call: sets status to `Disputed`, records claimed state.
    /// Second call (counterparty, within window): may submit a newer state.
    /// After window: anyone may finalise.
    /// Emits `("tch_disp",)` on initiation/update and `("tch_fin",)` on finalisation.
    pub fn dispute_tip_channel(
        env: Env,
        caller: Address,
        tipper: Address,
        creator: Address,
        token: Address,
        claimed_tipped_amount: i128,
        nonce: u64,
    ) {
        Self::require_not_paused(&env);
        caller.require_auth();

        let key = DataKey::TipChannel(tipper.clone(), creator.clone(), token.clone());
        if !env.storage().persistent().has(&key) {
            panic_with_error!(&env, TipChannelError::ChannelNotFound);
        }

        let channel = state_channel::load(&env, &tipper, &creator, &token);
        if caller != channel.tipper && caller != channel.creator {
            panic_with_error!(&env, TipChannelError::NotChannelParty);
        }
        if channel.status == state_channel::TipChannelStatus::Settled {
            panic_with_error!(&env, TipChannelError::AlreadySettled);
        }
        if claimed_tipped_amount < 0 || claimed_tipped_amount > channel.deposit {
            panic_with_error!(&env, TipChannelError::ExceedsDeposit);
        }

        state_channel::dispute(
            &env,
            &caller,
            &tipper,
            &creator,
            &token,
            claimed_tipped_amount,
            nonce,
        );
    }

    /// Returns the tip state channel for `(tipper, creator, token)`.
    pub fn get_tip_channel(
        env: Env,
        tipper: Address,
        creator: Address,
        token: Address,
    ) -> Option<state_channel::TipStateChannel> {
        env.storage()
            .persistent()
            .get(&DataKey::TipChannel(tipper, creator, token))
    }

    /// Returns all recorded tip entries for a tip state channel.
    pub fn get_tip_channel_tips(
        env: Env,
        tipper: Address,
        creator: Address,
        token: Address,
    ) -> Vec<state_channel::ChannelTip> {
        state_channel::get_channel_tips(&env, &tipper, &creator, &token)
    }

    // ── meta-transactions (gasless tipping) ──────────────────────────────────

    /// Registers a trusted relayer for meta-transactions. Admin only.
    ///
    /// Emits `("mtx_reg",)` with `(relayer)`.
    pub fn register_meta_relayer(env: Env, admin: Address, relayer: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        if meta_tx::is_trusted_relayer(&env, &relayer) {
            panic_with_error!(&env, MetaTxError::RelayerAlreadyRegistered);
        }

        meta_tx::register_relayer(&env, &relayer);

        env.events()
            .publish((symbol_short!("mtx_reg"),), (relayer,));
    }

    /// Removes a trusted relayer. Admin only.
    ///
    /// Emits `("mtx_rem",)` with `(relayer)`.
    pub fn remove_meta_relayer(env: Env, admin: Address, relayer: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        if !meta_tx::is_trusted_relayer(&env, &relayer) {
            panic_with_error!(&env, MetaTxError::RelayerNotFound);
        }

        meta_tx::remove_relayer(&env, &relayer);

        env.events()
            .publish((symbol_short!("mtx_rem"),), (relayer,));
    }

    /// Executes a meta-transaction tip on behalf of a user.
    ///
    /// The relayer submits a signed `MetaTipRequest`. The contract verifies:
    ///   - Relayer is trusted
    ///   - Request has not expired
    ///   - Nonce matches the stored per-sender nonce
    ///   - Signature is valid
    ///   - Request has not been replayed
    ///
    /// On success, executes the tip (or channel open) and increments the sender's nonce.
    /// Returns the meta-tx record ID.
    ///
    /// Emits `("mtx_exec",)` with `(record_id, from, relayer, to, amount, nonce)`.
    pub fn execute_meta_tip(env: Env, relayer: Address, request: meta_tx::MetaTipRequest) -> u64 {
        Self::require_not_paused(&env);
        relayer.require_auth();

        // Validate amount
        if request.amount <= 0 {
            panic_with_error!(&env, MetaTxError::InvalidAmount);
        }

        // Validate token whitelist
        let whitelisted: bool = env
            .storage()
            .instance()
            .get(&DataKey::TokenWhitelist(request.token.clone()))
            .unwrap_or(false);
        if !whitelisted {
            panic_with_error!(&env, MetaTxError::TokenNotWhitelisted);
        }

        // Verify signature and replay protection
        let hash = meta_tx::verify(&env, &relayer, &request);

        // Execute the action based on request type
        match request.action {
            meta_tx::MetaTipAction::Tip => {
                // Transfer tokens from `from` to contract
                token::Client::new(&env, &request.token).transfer(
                    &request.from,
                    &env.current_contract_address(),
                    &request.amount,
                );

                // Credit creator balance
                let key = DataKey::CreatorBalance(request.to.clone(), request.token.clone());
                let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
                env.storage()
                    .persistent()
                    .set(&key, &(current + request.amount));

                // Update creator total
                let total_key = DataKey::CreatorTotal(request.to.clone(), request.token.clone());
                let total: i128 = env.storage().persistent().get(&total_key).unwrap_or(0);
                env.storage()
                    .persistent()
                    .set(&total_key, &(total + request.amount));

                // Emit tip event
                env.events().publish(
                    (symbol_short!("tip"),),
                    (
                        request.from.clone(),
                        request.to.clone(),
                        request.token.clone(),
                        request.amount,
                    ),
                );
            }
            meta_tx::MetaTipAction::OpenChannel => {
                // Open a tip state channel
                let key = DataKey::TipChannel(
                    request.from.clone(),
                    request.to.clone(),
                    request.token.clone(),
                );
                if env.storage().persistent().has(&key) {
                    panic_with_error!(&env, TipChannelError::ChannelNotOpen);
                }

                // Default dispute window: 1 hour
                let dispute_window = 3600u64;
                state_channel::open(
                    &env,
                    &request.from,
                    &request.to,
                    &request.token,
                    request.amount,
                    dispute_window,
                );
            }
        }

        // Finalize: mark consumed, bump nonce, store record
        meta_tx::finalize(&env, &relayer, &request, &hash)
    }

    /// Returns the current meta-transaction nonce for a sender.
    pub fn get_meta_nonce(env: Env, sender: Address) -> u64 {
        meta_tx::get_nonce(&env, &sender)
    }

    /// Returns whether a relayer is trusted for meta-transactions.
    pub fn is_meta_relayer(env: Env, relayer: Address) -> bool {
        meta_tx::is_trusted_relayer(&env, &relayer)
    }

    /// Returns a meta-transaction execution record by ID.
    pub fn get_meta_record(env: Env, record_id: u64) -> Option<meta_tx::MetaTxRecord> {
        meta_tx::get_record(&env, record_id)
    }

    // ── commit-reveal (fair auctions & voting) ───────────────────────────────

    /// Creates a new commit-reveal round.
    ///
    /// Returns the round ID.
    /// Emits `("cr_new",)` with `(round_id, creator, timestamp)`.
    pub fn create_commit_reveal_round(
        env: Env,
        creator: Address,
        round_type: commit_reveal::RoundType,
        commit_duration: u64,
        reveal_duration: u64,
        description: String,
        token: Option<Address>,
        min_bid: i128,
    ) -> u64 {
        Self::require_not_paused(&env);
        creator.require_auth();

        if commit_duration < commit_reveal::MIN_COMMIT_DURATION
            || commit_duration > commit_reveal::MAX_COMMIT_DURATION
        {
            panic_with_error!(&env, CommitRevealError::InvalidCommitDuration);
        }
        if reveal_duration < commit_reveal::MIN_REVEAL_DURATION
            || reveal_duration > commit_reveal::MAX_REVEAL_DURATION
        {
            panic_with_error!(&env, CommitRevealError::InvalidRevealDuration);
        }

        commit_reveal::create_round(
            &env,
            &creator,
            round_type,
            commit_duration,
            reveal_duration,
            description,
            token,
            min_bid,
        )
    }

    /// Submits a commitment during the commit phase.
    ///
    /// `commitment_hash` should be SHA256(value || salt).
    /// Emits `("cr_cmt",)` with `(round_id, participant, commitment_hash)`.
    pub fn commit_to_round(
        env: Env,
        round_id: u64,
        participant: Address,
        commitment_hash: BytesN<32>,
    ) {
        Self::require_not_paused(&env);
        participant.require_auth();

        let round = commit_reveal::get_round(&env, round_id);
        if round.is_none() {
            panic_with_error!(&env, CommitRevealError::RoundNotFound);
        }
        let round = round.unwrap();

        if round.status != commit_reveal::RoundStatus::Committing {
            panic_with_error!(&env, CommitRevealError::NotInCommitPhase);
        }

        let now = env.ledger().timestamp();
        if now < round.commit_start {
            panic_with_error!(&env, CommitRevealError::CommitPhaseNotStarted);
        }
        if now >= round.commit_end {
            panic_with_error!(&env, CommitRevealError::CommitPhaseEnded);
        }

        let key = DataKey::CommitRevealCommitment(round_id, participant.clone());
        if env.storage().persistent().has(&key) {
            panic_with_error!(&env, CommitRevealError::AlreadyCommitted);
        }

        commit_reveal::commit(&env, round_id, &participant, commitment_hash);
    }

    /// Advances a round from Committing to Revealing status.
    ///
    /// Can be called by anyone once the commit phase has ended.
    /// Emits `("cr_rvl",)` with `(round_id)`.
    pub fn start_reveal_phase(env: Env, round_id: u64) {
        Self::require_not_paused(&env);

        let round = commit_reveal::get_round(&env, round_id);
        if round.is_none() {
            panic_with_error!(&env, CommitRevealError::RoundNotFound);
        }
        let round = round.unwrap();

        if round.status != commit_reveal::RoundStatus::Committing {
            panic_with_error!(&env, CommitRevealError::NotInCommitPhase);
        }

        let now = env.ledger().timestamp();
        if now < round.commit_end {
            panic_with_error!(&env, CommitRevealError::CommitPhaseNotEnded);
        }

        commit_reveal::start_reveal_phase(&env, round_id);
    }

    /// Reveals a commitment during the reveal phase.
    ///
    /// Verifies that `SHA256(value || salt) == commitment_hash`.
    /// Emits `("cr_rvld",)` with `(round_id, participant, value)`.
    pub fn reveal_commitment(
        env: Env,
        round_id: u64,
        participant: Address,
        value: i128,
        salt: BytesN<32>,
    ) {
        Self::require_not_paused(&env);
        participant.require_auth();

        let round = commit_reveal::get_round(&env, round_id);
        if round.is_none() {
            panic_with_error!(&env, CommitRevealError::RoundNotFound);
        }
        let round = round.unwrap();

        if round.status != commit_reveal::RoundStatus::Revealing {
            panic_with_error!(&env, CommitRevealError::NotInRevealPhase);
        }

        let now = env.ledger().timestamp();
        if now >= round.reveal_end {
            panic_with_error!(&env, CommitRevealError::RevealPhaseEnded);
        }

        let commit_key = DataKey::CommitRevealCommitment(round_id, participant.clone());
        let commitment: Option<commit_reveal::Commitment> =
            env.storage().persistent().get(&commit_key);
        if commitment.is_none() {
            panic_with_error!(&env, CommitRevealError::NoCommitment);
        }
        let commitment = commitment.unwrap();

        if commitment.revealed {
            panic_with_error!(&env, CommitRevealError::AlreadyRevealed);
        }

        let computed = commit_reveal::compute_commitment(&env, value, &salt);
        if computed != commitment.commitment_hash {
            panic_with_error!(&env, CommitRevealError::InvalidReveal);
        }

        commit_reveal::reveal(&env, round_id, &participant, value, salt);
    }

    /// Finalizes a round after the reveal phase ends.
    ///
    /// For auctions: determines the winner (highest bid).
    /// Can be called by anyone once the reveal phase has ended.
    /// Emits `("cr_fin",)` with `(round_id, winner, winning_bid)`.
    pub fn finalize_commit_reveal_round(env: Env, round_id: u64) {
        Self::require_not_paused(&env);

        let round = commit_reveal::get_round(&env, round_id);
        if round.is_none() {
            panic_with_error!(&env, CommitRevealError::RoundNotFound);
        }
        let round = round.unwrap();

        if round.status != commit_reveal::RoundStatus::Revealing {
            panic_with_error!(&env, CommitRevealError::NotInRevealPhase);
        }

        let now = env.ledger().timestamp();
        if now < round.reveal_end {
            panic_with_error!(&env, CommitRevealError::RevealPhaseNotEnded);
        }

        commit_reveal::finalize_round(&env, round_id);
    }

    /// Cancels a round. Creator or admin only.
    ///
    /// Emits `("cr_cncl",)` with `(round_id)`.
    pub fn cancel_commit_reveal_round(env: Env, caller: Address, round_id: u64) {
        Self::require_not_paused(&env);
        caller.require_auth();

        let round = commit_reveal::get_round(&env, round_id);
        if round.is_none() {
            panic_with_error!(&env, CommitRevealError::RoundNotFound);
        }
        let round = round.unwrap();

        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if caller != round.creator && caller != admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        if round.status == commit_reveal::RoundStatus::Finalized
            || round.status == commit_reveal::RoundStatus::Cancelled
        {
            panic_with_error!(&env, CommitRevealError::RoundNotActive);
        }

        commit_reveal::cancel_round(&env, round_id);
    }

    /// Returns a commit-reveal round by ID.
    pub fn get_commit_reveal_round(
        env: Env,
        round_id: u64,
    ) -> Option<commit_reveal::CommitRevealRound> {
        commit_reveal::get_round(&env, round_id)
    }

    /// Returns a commitment for a participant in a round.
    pub fn get_commitment(
        env: Env,
        round_id: u64,
        participant: Address,
    ) -> Option<commit_reveal::Commitment> {
        commit_reveal::get_commitment(&env, round_id, &participant)
    }

    /// Returns a reveal for a participant in a round.
    pub fn get_reveal(
        env: Env,
        round_id: u64,
        participant: Address,
    ) -> Option<commit_reveal::Reveal> {
        commit_reveal::get_reveal(&env, round_id, &participant)
    }

    /// Returns all round IDs created by a creator.
    pub fn get_creator_commit_reveal_rounds(env: Env, creator: Address) -> Vec<u64> {
        commit_reveal::get_creator_rounds(&env, &creator)
    }

    // ── zero-knowledge proofs (private tip verification) ─────────────────────

    /// Registers a new ZK circuit verification key.
    ///
    /// Returns the circuit ID.
    /// Emits `("zk_reg",)` with `(circuit_id, owner, vk_hash)`.
    pub fn register_zk_circuit(
        env: Env,
        owner: Address,
        circuit_type: zk_proof::CircuitType,
        vk_hash: BytesN<32>,
        description: String,
    ) -> u64 {
        Self::require_not_paused(&env);
        owner.require_auth();

        zk_proof::register_circuit(&env, &owner, circuit_type, vk_hash, description)
    }

    /// Deactivates a ZK circuit. Owner only.
    ///
    /// Emits `("zk_deact",)` with `(circuit_id)`.
    pub fn deactivate_zk_circuit(env: Env, owner: Address, circuit_id: u64) {
        Self::require_not_paused(&env);
        owner.require_auth();

        let circuit = zk_proof::get_circuit(&env, circuit_id);
        if circuit.is_none() {
            panic_with_error!(&env, ZkProofError::CircuitNotFound);
        }
        let circuit = circuit.unwrap();

        if circuit.owner != owner {
            panic_with_error!(&env, ZkProofError::NotCircuitOwner);
        }

        zk_proof::deactivate_circuit(&env, circuit_id);
    }

    /// Submits a zero-knowledge proof for verification.
    ///
    /// Returns the proof ID.
    /// Emits `("zk_sub",)` with `(proof_id, prover, circuit_id, nullifier)`.
    pub fn submit_zk_proof(
        env: Env,
        prover: Address,
        circuit_id: u64,
        proof_data: Bytes,
        public_inputs: Vec<BytesN<32>>,
        private_commitment: BytesN<32>,
        nullifier: BytesN<32>,
        metadata: String,
    ) -> u64 {
        Self::require_not_paused(&env);
        prover.require_auth();

        let circuit = zk_proof::get_circuit(&env, circuit_id);
        if circuit.is_none() {
            panic_with_error!(&env, ZkProofError::CircuitNotFound);
        }
        let circuit = circuit.unwrap();

        if !circuit.active {
            panic_with_error!(&env, ZkProofError::CircuitNotActive);
        }

        if proof_data.len() > zk_proof::MAX_PROOF_SIZE {
            panic_with_error!(&env, ZkProofError::ProofTooLarge);
        }

        if public_inputs.len() > zk_proof::MAX_PUBLIC_INPUTS {
            panic_with_error!(&env, ZkProofError::TooManyPublicInputs);
        }

        let nullifier_key = DataKey::ZkNullifier(nullifier.clone());
        if env
            .storage()
            .persistent()
            .get(&nullifier_key)
            .unwrap_or(false)
        {
            panic_with_error!(&env, ZkProofError::NullifierUsed);
        }

        zk_proof::submit_proof(
            &env,
            &prover,
            circuit_id,
            proof_data,
            public_inputs,
            private_commitment,
            nullifier,
            metadata,
        )
    }

    /// Verifies a submitted ZK proof. Admin only.
    ///
    /// In production, this would call a ZK verifier. For this implementation,
    /// the admin manually marks proofs as valid/invalid.
    /// Emits `("zk_ver",)` with `(proof_id, is_valid)`.
    pub fn verify_zk_proof(env: Env, admin: Address, proof_id: u64, is_valid: bool) {
        Self::require_not_paused(&env);
        admin.require_auth();

        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        let proof = zk_proof::get_proof(&env, proof_id);
        if proof.is_none() {
            panic_with_error!(&env, ZkProofError::ProofNotFound);
        }
        let proof = proof.unwrap();

        if proof.status != zk_proof::ProofStatus::Pending {
            panic_with_error!(&env, ZkProofError::ProofNotPending);
        }

        zk_proof::verify_proof(&env, proof_id, is_valid);
    }

    /// Revokes a ZK proof. Admin only.
    ///
    /// Emits `("zk_rvk",)` with `(proof_id)`.
    pub fn revoke_zk_proof(env: Env, admin: Address, proof_id: u64) {
        Self::require_not_paused(&env);
        admin.require_auth();

        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }

        let proof = zk_proof::get_proof(&env, proof_id);
        if proof.is_none() {
            panic_with_error!(&env, ZkProofError::ProofNotFound);
        }

        zk_proof::revoke_proof(&env, proof_id);
    }

    /// Creates a private tip using a ZK proof.
    ///
    /// Returns the private tip ID.
    /// Emits `("zk_ptip",)` with `(tip_id, creator, proof_id)`.
    pub fn create_zk_private_tip(
        env: Env,
        creator: Address,
        proof_id: u64,
        amount_commitment: BytesN<32>,
        nullifier: BytesN<32>,
    ) -> u64 {
        Self::require_not_paused(&env);
        creator.require_auth();

        let proof = zk_proof::get_proof(&env, proof_id);
        if proof.is_none() {
            panic_with_error!(&env, ZkProofError::ProofNotFound);
        }
        let proof = proof.unwrap();

        if proof.status != zk_proof::ProofStatus::Verified {
            panic_with_error!(&env, ZkProofError::ProofNotVerified);
        }

        if proof.nullifier != nullifier {
            panic_with_error!(&env, ZkProofError::NullifierMismatch);
        }

        zk_proof::create_private_tip(&env, &creator, proof_id, amount_commitment, nullifier)
    }

    /// Claims/reveals a private tip. Creator only.
    ///
    /// Emits `("zk_clm",)` with `(tip_id, creator)`.
    pub fn claim_zk_private_tip(env: Env, creator: Address, tip_id: u64) {
        Self::require_not_paused(&env);
        creator.require_auth();

        let tip = zk_proof::get_private_tip(&env, tip_id);
        if tip.is_none() {
            panic_with_error!(&env, ZkProofError::PrivateTipNotFound);
        }
        let tip = tip.unwrap();

        if tip.creator != creator {
            panic_with_error!(&env, ZkProofError::NotTipCreator);
        }

        if tip.claimed {
            panic_with_error!(&env, ZkProofError::AlreadyClaimed);
        }

        zk_proof::claim_private_tip(&env, tip_id);
    }

    /// Returns a ZK circuit by ID.
    pub fn get_zk_circuit(env: Env, circuit_id: u64) -> Option<zk_proof::VerificationKey> {
        zk_proof::get_circuit(&env, circuit_id)
    }

    /// Returns a ZK proof by ID.
    pub fn get_zk_proof(env: Env, proof_id: u64) -> Option<zk_proof::ZkProof> {
        zk_proof::get_proof(&env, proof_id)
    }

    /// Returns a private tip by ID.
    pub fn get_zk_private_tip(env: Env, tip_id: u64) -> Option<zk_proof::PrivateTipProof> {
        zk_proof::get_private_tip(&env, tip_id)
    }

    /// Returns all circuit IDs owned by an address.
    pub fn get_owner_zk_circuits(env: Env, owner: Address) -> Vec<u64> {
        zk_proof::get_owner_circuits(&env, &owner)
    }

    /// Returns all proof IDs submitted by a prover.
    pub fn get_prover_zk_proofs(env: Env, prover: Address) -> Vec<u64> {
        zk_proof::get_prover_proofs(&env, &prover)
    }

    /// Returns all private tip IDs for a creator.
    pub fn get_creator_zk_private_tips(env: Env, creator: Address) -> Vec<u64> {
        zk_proof::get_creator_private_tips(&env, &creator)
    }

    // ── optimistic rollup ────────────────────────────────────────────────────

    /// Initialize the rollup with a designated sequencer.
    ///
    /// Admin only. Emits `("rl_init",)`.
    pub fn init_rollup(env: Env, admin: Address, sequencer: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        env.storage()
            .instance()
            .set(&DataKey::RollupSequencer, &sequencer);
        env.storage().instance().set(&DataKey::RollupEnabled, &true);
        env.storage()
            .instance()
            .set(&DataKey::RollupChallengePeriod, &rollup::CHALLENGE_PERIOD);
        env.storage()
            .instance()
            .set(&DataKey::RollupBatchCounter, &0u64);
        env.storage()
            .instance()
            .set(&DataKey::RollupPendingCount, &0u64);
        env.storage()
            .instance()
            .set(&DataKey::RollupFinalizedCount, &0u64);
        env.storage()
            .instance()
            .set(&DataKey::RollupChallengedCount, &0u64);
        env.events().publish((symbol_short!("rl_init"),), sequencer);
    }

    /// Submit a tip batch to the rollup. Sequencer only.
    ///
    /// The batch enters a challenge period before credits are applied.
    /// Returns the new batch ID.
    pub fn submit_rollup_batch(
        env: Env,
        sequencer: Address,
        state_root: BytesN<32>,
        creator: Address,
        token: Address,
        total_amount: i128,
        tip_count: u32,
    ) -> u64 {
        let enabled: bool = env
            .storage()
            .instance()
            .get(&DataKey::RollupEnabled)
            .unwrap_or(false);
        if !enabled {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        if total_amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        rollup::batch::submit_batch(
            &env,
            &sequencer,
            state_root,
            creator,
            token,
            total_amount,
            tip_count,
        )
    }

    /// Finalize a rollup batch after the challenge period. Permissionless.
    pub fn finalize_rollup_batch(env: Env, batch_id: u64) {
        rollup::batch::finalize_batch(&env, batch_id);
    }

    /// Submit a fraud proof challenging a pending batch. Permissionless.
    ///
    /// Returns `true` if the challenge was accepted (roots differ).
    pub fn submit_fraud_proof(
        env: Env,
        challenger: Address,
        batch_id: u64,
        claimed_root: BytesN<32>,
    ) -> bool {
        rollup::fraud_proof::submit_fraud_proof(&env, &challenger, batch_id, claimed_root)
    }

    /// Returns the fraud proof for a batch, if one was accepted.
    pub fn get_fraud_proof(env: Env, batch_id: u64) -> Option<rollup::FraudProof> {
        rollup::fraud_proof::get_fraud_proof(&env, batch_id)
    }

    /// Returns a rollup batch by ID.
    pub fn get_rollup_batch(env: Env, batch_id: u64) -> Option<rollup::RollupBatch> {
        env.storage()
            .persistent()
            .get(&DataKey::RollupBatch(batch_id))
    }

    /// Returns the current rollup state summary.
    pub fn get_rollup_state(env: Env) -> rollup::RollupState {
        let enabled: bool = env
            .storage()
            .instance()
            .get(&DataKey::RollupEnabled)
            .unwrap_or(false);
        let sequencer: Address = env
            .storage()
            .instance()
            .get(&DataKey::RollupSequencer)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::Unauthorized));
        let challenge_period: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RollupChallengePeriod)
            .unwrap_or(rollup::CHALLENGE_PERIOD);
        let pending_batches: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RollupPendingCount)
            .unwrap_or(0);
        let finalized_batches: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RollupFinalizedCount)
            .unwrap_or(0);
        let challenged_batches: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RollupChallengedCount)
            .unwrap_or(0);
        rollup::RollupState {
            enabled,
            sequencer,
            challenge_period,
            pending_batches,
            finalized_batches,
            challenged_batches,
        }
    }

    // ── fractional ownership ─────────────────────────────────────────────────

    /// Mint `total_supply` fractions for `creator`.
    pub fn mint_fractions(
        env: Env,
        creator: Address,
        total_supply: u64,
        buyout_price_per_fraction: i128,
    ) {
        fractional_ownership::mint_fractions(
            &env,
            &creator,
            total_supply,
            buyout_price_per_fraction,
        );
    }

    /// Record `amount` of revenue for `creator`'s fraction pool.
    pub fn accrue_revenue(env: Env, creator: Address, amount: i128) {
        fractional_ownership::accrue_revenue(&env, &creator, amount);
    }

    /// Claim accumulated revenue for `holder` in `creator`'s pool.
    pub fn claim_fraction_revenue(env: Env, creator: Address, holder: Address) -> i128 {
        fractional_ownership::claim_revenue(&env, &creator, &holder)
    }

    /// Transfer `amount` fractions from `from` to `to`.
    pub fn transfer_fractions(env: Env, creator: Address, from: Address, to: Address, amount: u64) {
        fractional_ownership::transfer_fractions(&env, &creator, &from, &to, amount);
    }

    /// Buy out all fractions not held by `buyer`. Returns total cost.
    pub fn buyout_fractions(env: Env, creator: Address, buyer: Address) -> i128 {
        fractional_ownership::buyout(&env, &creator, &buyer)
    }

    /// Returns the fraction pool for `creator`.
    pub fn get_fraction_pool(
        env: Env,
        creator: Address,
    ) -> Option<fractional_ownership::FractionPool> {
        fractional_ownership::get_pool(&env, &creator)
    }

    /// Returns the fraction position for `holder` in `creator`'s pool.
    pub fn get_fraction_position(
        env: Env,
        creator: Address,
        holder: Address,
    ) -> fractional_ownership::FractionPosition {
        fractional_ownership::get_position(&env, &creator, &holder)
    }

    // ── royalty splits ───────────────────────────────────────────────────────

    /// Create or replace a split configuration for `split_id`.
    pub fn set_split(
        env: Env,
        split_id: Address,
        owner: Address,
        recipients: Vec<royalty::SplitRecipient>,
    ) {
        royalty::set_split(&env, &split_id, &owner, recipients);
    }

    /// Modify an existing split configuration (owner only).
    pub fn modify_split(
        env: Env,
        split_id: Address,
        owner: Address,
        recipients: Vec<royalty::SplitRecipient>,
    ) {
        royalty::modify_split(&env, &split_id, &owner, recipients);
    }

    /// Distribute `amount` according to the split config for `split_id`.
    pub fn distribute_split(env: Env, split_id: Address, amount: i128) -> i128 {
        royalty::distribute(&env, &split_id, amount)
    }

    /// Returns the split config for `split_id`.
    pub fn get_split(env: Env, split_id: Address) -> Option<royalty::SplitConfig> {
        royalty::get_split(&env, &split_id)
    }

    /// Returns a split history entry by index.
    pub fn get_split_history_entry(
        env: Env,
        split_id: Address,
        index: u64,
    ) -> Option<royalty::SplitHistoryEntry> {
        royalty::get_history_entry(&env, &split_id, index)
    }

    /// Returns the total number of distribution history entries for `split_id`.
    pub fn get_split_history_count(env: Env, split_id: Address) -> u64 {
        royalty::get_history_count(&env, &split_id)
    }

    /// Returns the accumulated split balance for `recipient`.
    pub fn get_split_balance(env: Env, recipient: Address) -> i128 {
        royalty::get_balance(&env, &recipient)
    }

    // ── Cross-Contract Calls (#215) ──────────────────────────────────────────

    /// Route a single cross-contract call.
    ///
    /// Returns the call ID.
    pub fn cc_route_call(
        env: Env,
        caller: Address,
        target: Address,
        call_type: cross_contract::CallType,
        args: soroban_sdk::Bytes,
    ) -> u64 {
        cross_contract::route_call(&env, &caller, &target, call_type, args)
    }

    /// Execute a batch of cross-contract calls.
    ///
    /// Returns the batch ID.
    pub fn cc_route_batch(
        env: Env,
        caller: Address,
        targets: Vec<Address>,
        call_types: Vec<cross_contract::CallType>,
        args_list: Vec<soroban_sdk::Bytes>,
    ) -> u64 {
        cross_contract::route_batch(&env, &caller, targets, call_types, args_list)
    }

    /// Get a cross-contract call record by ID.
    pub fn cc_get_call(env: Env, call_id: u64) -> Option<cross_contract::CrossCall> {
        cross_contract::get_call(&env, call_id)
    }

    /// Get a batch record by ID.
    pub fn cc_get_batch(env: Env, batch_id: u64) -> Option<cross_contract::CallBatch> {
        cross_contract::get_batch(&env, batch_id)
    }

    /// Get all call IDs initiated by a caller.
    pub fn cc_get_caller_calls(env: Env, caller: Address) -> Vec<u64> {
        cross_contract::get_caller_calls(&env, &caller)
    }

    // ── Tip Sharding (#269) ──────────────────────────────────────────────────

    /// Initialize the sharding system.
    pub fn shard_init(env: Env, admin: Address, shard_count: u32, auto_rebalance: bool) {
        sharding::init_sharding(&env, &admin, shard_count, auto_rebalance)
    }

    /// Record a tip being processed by its assigned shard.
    ///
    /// Returns the shard ID.
    pub fn shard_record_tip(env: Env, sender: Address, amount: i128, tip_id: u64) -> u32 {
        sharding::record_tip_in_shard(&env, &sender, amount, tip_id)
    }

    /// Initiate a cross-shard transfer.
    ///
    /// Returns the transfer ID.
    pub fn shard_cross_transfer(
        env: Env,
        from_shard: u32,
        to_shard: u32,
        tip_id: u64,
        amount: i128,
        token: Address,
    ) -> u64 {
        sharding::cross_shard_transfer(&env, from_shard, to_shard, tip_id, amount, &token)
    }

    /// Finalize a cross-shard transfer.
    pub fn shard_finalize_transfer(env: Env, transfer_id: u64) {
        sharding::finalize_transfer(&env, transfer_id)
    }

    /// Trigger a manual rebalance of shards.
    pub fn shard_rebalance(env: Env, admin: Address) {
        sharding::rebalance(&env, &admin)
    }

    /// Get the state of a specific shard.
    pub fn shard_get_state(env: Env, shard_id: u32) -> sharding::ShardState {
        sharding::get_shard(&env, shard_id)
    }

    /// Get the sharding configuration.
    pub fn shard_get_config(env: Env) -> sharding::ShardConfig {
        sharding::get_config_pub(&env)
    }

    /// Get the shard assignment for an address.
    pub fn shard_get_assignment(env: Env, addr: Address) -> u32 {
        sharding::get_assignment(&env, &addr)
    }

    /// Get a cross-shard transfer record.
    pub fn shard_get_transfer(env: Env, transfer_id: u64) -> Option<sharding::ShardTransfer> {
        sharding::get_transfer(&env, transfer_id)
    }

    // ── Tip Validity Proofs (#270) ───────────────────────────────────────────

    /// Generate a validity proof for a tip.
    ///
    /// Returns the proof ID.
    pub fn vp_generate(
        env: Env,
        sender: Address,
        creator: Address,
        token: Address,
        amount: i128,
        tip_id: u64,
        witness: soroban_sdk::Bytes,
    ) -> u64 {
        validity_proof::generate_proof(&env, &sender, &creator, &token, amount, tip_id, witness)
    }

    /// Verify a validity proof.
    ///
    /// Returns true if valid.
    pub fn vp_verify(env: Env, proof_id: u64) -> bool {
        validity_proof::verify_proof(&env, proof_id)
    }

    /// Batch-verify multiple proofs.
    ///
    /// Returns the count of valid proofs.
    pub fn vp_batch_verify(env: Env, proof_ids: Vec<u64>) -> u32 {
        validity_proof::batch_verify(&env, proof_ids)
    }

    /// Aggregate multiple proofs into a single aggregate proof.
    ///
    /// Returns the aggregate ID.
    pub fn vp_aggregate(env: Env, proof_ids: Vec<u64>) -> u64 {
        validity_proof::aggregate_proofs(&env, proof_ids)
    }

    /// Revoke a proof.
    pub fn vp_revoke(env: Env, admin: Address, proof_id: u64) {
        validity_proof::revoke_proof(&env, &admin, proof_id)
    }

    /// Get a validity proof by ID.
    pub fn vp_get_proof(env: Env, proof_id: u64) -> Option<validity_proof::TipProof> {
        validity_proof::get_proof(&env, proof_id)
    }

    /// Get the proof ID for a given tip ID.
    pub fn vp_get_proof_for_tip(env: Env, tip_id: u64) -> Option<u64> {
        validity_proof::get_proof_for_tip(&env, tip_id)
    }

    /// Get an aggregated proof by ID.
    pub fn vp_get_aggregate(env: Env, agg_id: u64) -> Option<validity_proof::AggregatedProof> {
        validity_proof::get_aggregate(&env, agg_id)
    }

    // ── Tip Verkle Trees (#273) ──────────────────────────────────────────────

    /// Create a new Verkle tree.
    ///
    /// Returns the tree ID.
    pub fn vk_create_tree(env: Env, owner: Address) -> u64 {
        verkle_tree::create_tree(&env, &owner)
    }

    /// Insert or update a leaf in a Verkle tree.
    ///
    /// Returns the new root commitment.
    pub fn vk_update_leaf(
        env: Env,
        tree_id: u64,
        creator: Address,
        token: Address,
        value: soroban_sdk::Bytes,
    ) -> BytesN<32> {
        verkle_tree::update_leaf(&env, tree_id, &creator, &token, value)
    }

    /// Generate a Verkle proof for a leaf.
    ///
    /// Returns the proof ID.
    pub fn vk_generate_proof(env: Env, tree_id: u64, leaf_index: u32) -> u64 {
        verkle_tree::generate_proof(&env, tree_id, leaf_index)
    }

    /// Verify a Verkle proof against the current tree root.
    ///
    /// Returns true if valid.
    pub fn vk_verify_proof(env: Env, proof_id: u64) -> bool {
        verkle_tree::verify_proof(&env, proof_id)
    }

    /// Get a Verkle tree by ID.
    pub fn vk_get_tree(env: Env, tree_id: u64) -> Option<verkle_tree::VerkleTree> {
        verkle_tree::get_tree(&env, tree_id)
    }

    /// Get a Verkle leaf by tree ID and index.
    pub fn vk_get_leaf(env: Env, tree_id: u64, index: u32) -> Option<verkle_tree::VerkleLeaf> {
        verkle_tree::get_leaf(&env, tree_id, index)
    }

    /// Get a Verkle proof by ID.
    pub fn vk_get_proof(env: Env, proof_id: u64) -> Option<verkle_tree::VerkleProof> {
        verkle_tree::get_proof(&env, proof_id)
    }

    /// Get all tree IDs owned by an address.
    pub fn vk_get_owner_trees(env: Env, owner: Address) -> Vec<u64> {
        verkle_tree::get_owner_trees(&env, &owner)
    }

    /// Deactivate a Verkle tree.
    pub fn vk_deactivate_tree(env: Env, owner: Address, tree_id: u64) {
        verkle_tree::deactivate_tree(&env, &owner, tree_id)
    }
}























    // ── Time-Lock Puzzles ────────────────────────────────────────────────────

    /// Create a time-lock puzzle wrapping a tip.
    ///
    /// The caller must supply a `commitment` = SHA-256(secret XOR nonce) computed
    /// off-chain. The `secret` and `nonce` are never stored on-chain; only the
    /// commitment is persisted. The tip amount is transferred from `creator` to
    /// the contract and held until the puzzle is solved.
    ///
    /// Returns the new puzzle ID.
    pub fn create_time_lock_puzzle(
        env: Env,
        creator: Address,
        recipient: Address,
        token: Address,
        amount: i128,
        commitment: BytesN<32>,
        unlock_time: u64,
        difficulty: time_lock_puzzle::PuzzleDifficulty,
        custom_iterations: Option<u64>,
    ) -> u64 {
        Self::require_not_paused(&env);
        creator.require_auth();

        if amount <= 0 {
            panic_with_error!(&env, TipJarError::InvalidAmount);
        }
        if unlock_time <= env.ledger().timestamp() {
            panic_with_error!(&env, TipJarError::InvalidUnlockTime);
        }

        // Transfer tip amount from creator to contract.
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&creator, &env.current_contract_address(), &amount);

        let iterations =
            time_lock_puzzle::puzzle::calculate_iterations(difficulty, custom_iterations);

        let id = time_lock_puzzle::next_puzzle_id(&env);
        let puzzle = time_lock_puzzle::TimeLockPuzzle {
            id,
            creator: creator.clone(),
            recipient: recipient.clone(),
            token,
            amount,
            commitment,
            iterations,
            unlock_time,
            created_at: env.ledger().timestamp(),
            difficulty,
            status: time_lock_puzzle::PuzzleStatus::Active,
        };

        time_lock_puzzle::save_puzzle(&env, &puzzle);
        time_lock_puzzle::add_creator_puzzle(&env, &creator, id);
        time_lock_puzzle::add_recipient_puzzle(&env, &recipient, id);

        env.events().publish(
            (symbol_short!("tlp_new"),),
            (creator, recipient, id, amount, unlock_time),
        );

        id
    }

    /// Solve a time-lock puzzle and release the tip to the recipient.
    ///
    /// The solver supplies the original `secret` and `nonce` used to generate
    /// the commitment. On success the tip is transferred to the recipient.
    pub fn solve_time_lock_puzzle(env: Env, puzzle_id: u64, secret: BytesN<32>, nonce: BytesN<32>) {
        Self::require_not_paused(&env);

        let puzzle = time_lock_puzzle::get_puzzle(&env, puzzle_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::PuzzleNotFound));

        use time_lock_puzzle::solver::{solve_puzzle, SolveResult};
        match solve_puzzle(&env, puzzle_id, &secret, &nonce) {
            SolveResult::Success => {}
            SolveResult::TooEarly => panic_with_error!(&env, TipJarError::PuzzleTooEarly),
            SolveResult::WrongSolution => {
                panic_with_error!(&env, TipJarError::PuzzleWrongSolution)
            }
            SolveResult::NotActive => match puzzle.status {
                time_lock_puzzle::PuzzleStatus::Solved => {
                    panic_with_error!(&env, TipJarError::PuzzleAlreadySolved)
                }
                time_lock_puzzle::PuzzleStatus::Cancelled => {
                    panic_with_error!(&env, TipJarError::PuzzleCancelled)
                }
                _ => panic_with_error!(&env, TipJarError::PuzzleNotActive),
            },
        }

        // Release tip to recipient.
        let token_client = token::Client::new(&env, &puzzle.token);
        token_client.transfer(
            &env.current_contract_address(),
            &puzzle.recipient,
            &puzzle.amount,
        );

        env.events().publish(
            (symbol_short!("tlp_slv"),),
            (puzzle_id, puzzle.recipient, puzzle.amount),
        );
    }

    /// Cancel an active time-lock puzzle and refund the tip to the creator.
    ///
    /// Only the puzzle creator may cancel.
    pub fn cancel_time_lock_puzzle(env: Env, creator: Address, puzzle_id: u64) {
        Self::require_not_paused(&env);
        creator.require_auth();

        let puzzle = time_lock_puzzle::get_puzzle(&env, puzzle_id)
            .unwrap_or_else(|| panic_with_error!(&env, TipJarError::PuzzleNotFound));

        if puzzle.creator != creator {
            panic_with_error!(&env, TipJarError::PuzzleUnauthorized);
        }

        if !time_lock_puzzle::solver::cancel_puzzle(&env, puzzle_id) {
            match puzzle.status {
                time_lock_puzzle::PuzzleStatus::Solved => {
                    panic_with_error!(&env, TipJarError::PuzzleAlreadySolved)
                }
                _ => panic_with_error!(&env, TipJarError::PuzzleNotActive),
            }
        }

        // Refund tip to creator.
        let token_client = token::Client::new(&env, &puzzle.token);
        token_client.transfer(
            &env.current_contract_address(),
            &puzzle.creator,
            &puzzle.amount,
        );

        env.events().publish(
            (symbol_short!("tlp_cnl"),),
            (puzzle_id, creator, puzzle.amount),
        );
    }

    /// Get a time-lock puzzle record by ID.
    pub fn get_time_lock_puzzle(
        env: Env,
        puzzle_id: u64,
    ) -> Option<time_lock_puzzle::TimeLockPuzzle> {
        time_lock_puzzle::get_puzzle(&env, puzzle_id)
    }

    /// Get the status of a time-lock puzzle.
    pub fn get_puzzle_status(env: Env, puzzle_id: u64) -> Option<time_lock_puzzle::PuzzleStatus> {
        time_lock_puzzle::solver::get_puzzle_status(&env, puzzle_id)
    }

    /// Check whether a puzzle's unlock time has been reached.
    pub fn is_puzzle_unlocked(env: Env, puzzle_id: u64) -> bool {
        time_lock_puzzle::solver::is_puzzle_unlocked(&env, puzzle_id)
    }

    /// List all puzzle IDs created by an address.
    pub fn get_creator_puzzles(env: Env, creator: Address) -> Vec<u64> {
        time_lock_puzzle::get_creator_puzzles(&env, &creator)
    }

    /// List all puzzle IDs targeting a recipient.
    pub fn get_recipient_puzzles(env: Env, recipient: Address) -> Vec<u64> {
        time_lock_puzzle::get_recipient_puzzles(&env, &recipient)
    }

    // ── Decentralized Identity (#308) ─────────────────────────────────────────

    /// Register a new DID for the caller.
    ///
    /// Emits `("did_reg",)` with `(did_id, owner)`.
    pub fn did_register(
        env: Env,
        owner: Address,
        did_string: String,
        method: did::DidMethod,
    ) -> u64 {
        Self::require_not_paused(&env);
        owner.require_auth();
        did::register_did(&env, &owner, did_string, method)
    }

    /// Update an existing DID record. Only the owner may update.
    pub fn did_update(env: Env, owner: Address, did_id: u64, did_string: String) {
        Self::require_not_paused(&env);
        owner.require_auth();
        did::update_did(&env, &owner, did_id, did_string);
    }

    /// Add a claim to a DID.
    pub fn did_add_claim(
        env: Env,
        owner: Address,
        did_id: u64,
        claim_type: String,
        claim_value: String,
    ) -> u64 {
        Self::require_not_paused(&env);
        owner.require_auth();
        // Verify caller owns the DID.
        let d = did::get_did(&env, did_id).unwrap_or_else(|| panic_with_error!(&env, TipJarError::Unauthorized));
        if d.owner != owner {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        did::add_claim(&env, did_id, claim_type, claim_value)
    }

    /// Verify a DID claim. Admin or designated verifier only.
    pub fn did_verify_claim(env: Env, verifier: Address, claim_id: u64) {
        verifier.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if verifier != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        did::verify_claim(&env, &verifier, claim_id);
    }

    /// Get a DID record.
    pub fn did_get(env: Env, did_id: u64) -> Option<did::Did> {
        did::get_did(&env, did_id)
    }

    /// Get all DID IDs for an owner.
    pub fn did_get_owner_dids(env: Env, owner: Address) -> Vec<u64> {
        did::get_owner_dids(&env, &owner)
    }

    /// Get all claim IDs for a DID.
    pub fn did_get_claims(env: Env, did_id: u64) -> Vec<u64> {
        did::get_did_claims(&env, did_id)
    }

    /// Get a claim record.
    pub fn did_get_claim(env: Env, claim_id: u64) -> Option<did::IdentityClaim> {
        did::get_claim(&env, claim_id)
    }

    // ── Verifiable Credentials (#309) ─────────────────────────────────────────

    /// Issue a verifiable credential. Admin only.
    ///
    /// Emits `("vc_issue",)` with `(id, issuer, subject)`.
    pub fn vc_issue(
        env: Env,
        issuer: Address,
        subject: Address,
        schema: String,
        credential_data: String,
        expires_at: Option<u64>,
    ) -> u64 {
        Self::require_not_paused(&env);
        issuer.require_auth();
        verifiable_credentials::issue_credential(
            &env,
            &issuer,
            &subject,
            schema,
            credential_data,
            expires_at,
        )
    }

    /// Verify a credential is active and not expired.
    pub fn vc_verify(env: Env, credential_id: u64) -> bool {
        verifiable_credentials::verify_credential(&env, credential_id)
    }

    /// Revoke a credential. Only the issuer may revoke.
    ///
    /// Emits `("vc_rev",)` with `(credential_id, issuer)`.
    pub fn vc_revoke(env: Env, issuer: Address, credential_id: u64) {
        Self::require_not_paused(&env);
        issuer.require_auth();
        verifiable_credentials::revoke_credential(&env, &issuer, credential_id);
    }

    /// Get a credential record.
    pub fn vc_get(env: Env, credential_id: u64) -> Option<verifiable_credentials::VerifiableCredential> {
        verifiable_credentials::get_credential(&env, credential_id)
    }

    /// Get all credential IDs for a subject.
    pub fn vc_get_subject_credentials(env: Env, subject: Address) -> Vec<u64> {
        verifiable_credentials::get_subject_credentials(&env, &subject)
    }

    /// Get all credential IDs issued by an issuer.
    pub fn vc_get_issuer_credentials(env: Env, issuer: Address) -> Vec<u64> {
        verifiable_credentials::get_issuer_credentials(&env, &issuer)
    }

    // ── Reputation Tokens (#310) ──────────────────────────────────────────────

    /// Mint reputation tokens for an account based on tip amount. Internal use.
    ///
    /// Emits `("rep_mint",)` with `(account, gain, new_score)`.
    pub fn rep_mint(env: Env, caller: Address, account: Address, amount: i128) {
        Self::require_not_paused(&env);
        caller.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if caller != stored_admin {
            panic_with_error!(&env, TipJarError::Unauthorized);
        }
        reputation_tokens::mint(&env, &account, amount);
    }

    /// Trigger reputation decay for an account.
    ///
    /// Emits `("rep_dcy",)` with `(account, delta, new_score)`.
    pub fn rep_decay(env: Env, account: Address) {
        reputation_tokens::decay(&env, &account);
    }

    /// Get the current reputation score for an account.
    pub fn rep_get_score(env: Env, account: Address) -> i128 {
        reputation_tokens::get_score(&env, &account)
    }

    /// Get the full reputation token record.
    pub fn rep_get_token(env: Env, account: Address) -> reputation_tokens::ReputationToken {
        reputation_tokens::get_token(&env, &account)
    }

    /// Get reputation history for an account.
    pub fn rep_get_history(env: Env, account: Address) -> Vec<reputation_tokens::RepTokenHistory> {
        reputation_tokens::get_history(&env, &account)
    }

    // ── Composable NFTs (#311) ────────────────────────────────────────────────

    /// Mint a new composable NFT tip receipt.
    ///
    /// Emits `("nft_mint",)` with `(id, owner)`.
    pub fn nft_mint(env: Env, owner: Address, metadata: String) -> u64 {
        Self::require_not_paused(&env);
        owner.require_auth();
        composable_nft::mint(&env, &owner, metadata)
    }

    /// Compose a child NFT into a parent.
    ///
    /// Emits `("nft_comp",)` with `(parent_id, child_id, owner)`.
    pub fn nft_compose(env: Env, owner: Address, parent_id: u64, child_id: u64) {
        Self::require_not_paused(&env);
        owner.require_auth();
        composable_nft::compose(&env, &owner, parent_id, child_id);
    }

    /// Decompose a child NFT from its parent.
    ///
    /// Emits `("nft_dcp",)` with `(parent_id, child_id, owner)`.
    pub fn nft_decompose(env: Env, owner: Address, child_id: u64) {
        Self::require_not_paused(&env);
        owner.require_auth();
        composable_nft::decompose(&env, &owner, child_id);
    }

    /// Get an NFT record.
    pub fn nft_get(env: Env, nft_id: u64) -> Option<composable_nft::ComposableNft> {
        composable_nft::get_nft(&env, nft_id)
    }

    /// Get all NFT IDs owned by an address.
    pub fn nft_get_owner_nfts(env: Env, owner: Address) -> Vec<u64> {
        composable_nft::get_owner_nfts(&env, &owner)
    }

    /// Get child NFT IDs for a parent.
    pub fn nft_get_children(env: Env, parent_id: u64) -> Vec<u64> {
        composable_nft::get_children(&env, parent_id)
    }

    /// Get composition history for an NFT.
    pub fn nft_get_history(env: Env, nft_id: u64) -> Vec<composable_nft::CompositionEvent> {
        composable_nft::get_composition_history(&env, nft_id)
    }
}
