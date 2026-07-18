# Requirements Document

## Introduction

The Tip Lottery system extends the TipJar Soroban smart contract with a lottery mechanism that rewards tippers with bonus prizes. When tippers send tips, they are automatically entered into an active lottery round. An admin-controlled round lifecycle manages prize pool accumulation, verifiable random winner selection via Soroban's PRNG (`env.prng()`), and on-chain prize distribution. All lottery state transitions emit events for off-chain indexing.

The system integrates with the existing `tip` function, the platform fee mechanism, the pause/circuit-breaker guards, and the `DataKey` persistent storage layout already present in `contracts/tipjar/src/lib.rs`.

## Glossary

- **Lottery**: A time-bounded round in which tippers earn entries and one or more winners are selected at random to receive prizes.
- **Lottery_Module**: The on-chain component (new `lottery` sub-module) responsible for all lottery logic.
- **Round**: A single lottery instance identified by a monotonically increasing `round_id: u64`.
- **Entry**: A record associating a tipper `Address` with a `round_id` and the number of tickets earned.
- **Ticket**: A unit of lottery participation. One ticket is awarded per qualifying tip; additional tickets may be awarded proportionally to tip size.
- **Prize_Pool**: The accumulated token balance designated as prizes for a given round, funded by a configurable share of tips and/or direct contributions.
- **Winner**: A tipper selected by the PRNG to receive a share of the Prize_Pool.
- **Admin**: The address stored under `DataKey::Admin`; the sole authority for creating rounds and resolving draws.
- **PRNG**: Soroban's per-ledger pseudo-random number generator accessed via `env.prng()`.
- **Fee_Basis_Points**: A value in basis points (1 bp = 0.01%) used to express ratios; 10 000 bp = 100%.
- **Round_Status**: An enum with variants `Open`, `Drawing`, `Closed`.

---

## Requirements

### Requirement 1: Lottery Round Lifecycle Management

**User Story:** As an Admin, I want to create, open, and close lottery rounds with configurable parameters, so that I can control when tippers can enter and when winners are drawn.

#### Acceptance Criteria

1. THE Lottery_Module SHALL expose a `create_lottery_round` function that accepts `admin: Address`, `token: Address`, `ticket_price_min: i128`, `max_winners: u32`, `prize_pool_fee_bps: u32`, and `end_ledger: u64`, and returns the new `round_id: u64`.
2. WHEN `create_lottery_round` is called, THE Lottery_Module SHALL require authentication from the caller and verify the caller matches the stored `DataKey::Admin` address.
3. WHEN `create_lottery_round` is called, THE Lottery_Module SHALL reject the call with `LotteryError::ContractPaused` if the contract is currently paused.
4. WHEN `create_lottery_round` is called with `ticket_price_min <= 0`, THE Lottery_Module SHALL reject the call with `LotteryError::InvalidTicketPrice`.
5. WHEN `create_lottery_round` is called with `max_winners == 0`, THE Lottery_Module SHALL reject the call with `LotteryError::InvalidWinnerCount`.
6. WHEN `create_lottery_round` is called with `prize_pool_fee_bps > 5000`, THE Lottery_Module SHALL reject the call with `LotteryError::InvalidPrizePoolFee` (cap at 50% of tips).
7. WHEN `create_lottery_round` is called with `end_ledger` less than or equal to the current ledger sequence, THE Lottery_Module SHALL reject the call with `LotteryError::InvalidEndLedger`.
8. WHEN a round is successfully created, THE Lottery_Module SHALL store the round with `Round_Status::Open` and emit a `lottery_created` event containing `(round_id, token, max_winners, prize_pool_fee_bps, end_ledger)`.
9. THE Lottery_Module SHALL maintain a global `LotteryRoundCounter` in instance storage to generate unique, sequential `round_id` values.
10. WHEN `create_lottery_round` is called while another round for the same token is in `Round_Status::Open` or `Round_Status::Drawing`, THE Lottery_Module SHALL reject the call with `LotteryError::ActiveRoundExists`.

---

### Requirement 2: Automatic Lottery Entry on Tip

**User Story:** As a tipper, I want to be automatically entered into the active lottery round when I send a qualifying tip, so that I have a chance to win bonus prizes without extra steps.

#### Acceptance Criteria

1. WHEN the `tip` function executes successfully and an `Open` lottery round exists for the tipped token, THE Lottery_Module SHALL record a lottery entry for the `sender`.
2. WHEN a tip amount is greater than or equal to the round's `ticket_price_min`, THE Lottery_Module SHALL award `floor(amount / ticket_price_min)` tickets to the sender for that round, with a minimum of 1 ticket.
3. WHEN a tip amount is less than `ticket_price_min`, THE Lottery_Module SHALL award 0 tickets and SHALL NOT create an entry for that tip.
4. WHEN a tipper already has an entry in the current round, THE Lottery_Module SHALL increment the existing ticket count rather than creating a duplicate entry.
5. WHEN a lottery entry is created or updated, THE Lottery_Module SHALL emit a `lottery_entry` event containing `(round_id, sender, tickets_awarded, total_tickets_for_sender)`.
6. WHILE the contract is paused, THE Lottery_Module SHALL NOT process lottery entries (enforced by the existing pause guard on `tip`).
7. IF no `Open` round exists for the tipped token at the time of the tip, THE Lottery_Module SHALL skip lottery entry processing without error.

---

### Requirement 3: Prize Pool Funding

**User Story:** As an Admin, I want the prize pool to be funded automatically from a share of tips and optionally via direct contributions, so that winners receive meaningful rewards.

#### Acceptance Criteria

1. WHEN a qualifying tip is processed and a lottery entry is recorded, THE Lottery_Module SHALL transfer `floor(creator_amount * prize_pool_fee_bps / 10_000)` from the creator's credited amount into the round's Prize_Pool balance.
2. THE Lottery_Module SHALL store the Prize_Pool balance per `round_id` and `token` in persistent storage under `DataKey::Lottery(LotteryKey::PrizePool(round_id))`.
3. THE Lottery_Module SHALL expose a `contribute_to_prize_pool` function accepting `contributor: Address`, `round_id: u64`, `token: Address`, and `amount: i128` that transfers tokens directly from the contributor to the contract and adds them to the Prize_Pool.
4. WHEN `contribute_to_prize_pool` is called with `amount <= 0`, THE Lottery_Module SHALL reject the call with `LotteryError::InvalidAmount`.
5. WHEN `contribute_to_prize_pool` is called for a round that is not in `Round_Status::Open`, THE Lottery_Module SHALL reject the call with `LotteryError::RoundNotOpen`.
6. WHEN `contribute_to_prize_pool` is called for a token that does not match the round's configured token, THE Lottery_Module SHALL reject the call with `LotteryError::TokenMismatch`.
7. WHEN a direct contribution is accepted, THE Lottery_Module SHALL emit a `prize_contributed` event containing `(round_id, contributor, amount)`.

---

### Requirement 4: Verifiable Random Winner Selection

**User Story:** As a tipper, I want winners to be selected using Soroban's on-chain PRNG, so that the draw is verifiable and cannot be manipulated off-chain.

#### Acceptance Criteria

1. THE Lottery_Module SHALL expose a `draw_lottery_winners` function accepting `admin: Address` and `round_id: u64` that transitions the round to `Round_Status::Drawing` and selects winners.
2. WHEN `draw_lottery_winners` is called, THE Lottery_Module SHALL require authentication from the caller and verify the caller matches the stored `DataKey::Admin` address.
3. WHEN `draw_lottery_winners` is called for a round that is not in `Round_Status::Open`, THE Lottery_Module SHALL reject the call with `LotteryError::RoundNotOpen`.
4. WHEN `draw_lottery_winners` is called before the round's `end_ledger` has been reached, THE Lottery_Module SHALL reject the call with `LotteryError::RoundNotEnded`.
5. WHEN `draw_lottery_winners` is called for a round with zero entries, THE Lottery_Module SHALL transition the round to `Round_Status::Closed` without selecting winners and emit a `lottery_no_entries` event containing `round_id`.
6. WHEN `draw_lottery_winners` is called for a round with fewer unique entrants than `max_winners`, THE Lottery_Module SHALL select all unique entrants as winners.
7. THE Lottery_Module SHALL use `env.prng().u64_in_range(0, total_tickets - 1)` to perform weighted random selection, where each ticket represents one unit of weight in the cumulative ticket distribution.
8. THE Lottery_Module SHALL ensure each address is selected as a winner at most once per round, re-drawing if a duplicate is selected.
9. WHEN winners are selected, THE Lottery_Module SHALL store the winner list under `DataKey::Lottery(LotteryKey::Winners(round_id))` and transition the round to `Round_Status::Closed`.
10. WHEN the draw completes, THE Lottery_Module SHALL emit a `lottery_drawn` event containing `(round_id, winner_count, prize_pool_total)`.

---

### Requirement 5: Prize Distribution

**User Story:** As a lottery winner, I want to claim my prize share on-chain, so that I receive my bonus tokens directly to my wallet.

#### Acceptance Criteria

1. THE Lottery_Module SHALL expose a `claim_lottery_prize` function accepting `winner: Address` and `round_id: u64` that transfers the winner's prize share to the winner.
2. WHEN `claim_lottery_prize` is called, THE Lottery_Module SHALL require authentication from the `winner` address.
3. WHEN `claim_lottery_prize` is called for a round that is not in `Round_Status::Closed`, THE Lottery_Module SHALL reject the call with `LotteryError::RoundNotClosed`.
4. WHEN `claim_lottery_prize` is called by an address that is not in the winner list for the round, THE Lottery_Module SHALL reject the call with `LotteryError::NotAWinner`.
5. WHEN `claim_lottery_prize` is called by a winner who has already claimed, THE Lottery_Module SHALL reject the call with `LotteryError::PrizeAlreadyClaimed`.
6. THE Lottery_Module SHALL distribute the Prize_Pool equally among all winners: each winner receives `floor(prize_pool / winner_count)` tokens.
7. WHEN the last winner claims, THE Lottery_Module SHALL transfer any remainder (due to integer division) to the last claimant.
8. WHEN a prize is successfully claimed, THE Lottery_Module SHALL mark the winner's claim as paid, transfer tokens from the contract to the winner, and emit a `prize_claimed` event containing `(round_id, winner, amount)`.
9. IF the Prize_Pool balance for a round is zero at claim time, THE Lottery_Module SHALL reject the call with `LotteryError::EmptyPrizePool`.

---

### Requirement 6: Lottery Query Functions

**User Story:** As a developer or off-chain indexer, I want to query lottery state, so that I can display round information, entry status, and winner lists in a UI or analytics dashboard.

#### Acceptance Criteria

1. THE Lottery_Module SHALL expose a `get_lottery_round` function accepting `round_id: u64` that returns the full `LotteryRound` struct or panics with `LotteryError::RoundNotFound` if absent.
2. THE Lottery_Module SHALL expose a `get_lottery_entry` function accepting `round_id: u64` and `tipper: Address` that returns the `LotteryEntry` struct for that tipper, or panics with `LotteryError::EntryNotFound` if absent.
3. THE Lottery_Module SHALL expose a `get_lottery_winners` function accepting `round_id: u64` that returns a `Vec<Address>` of winners, or an empty `Vec` if the round has not been drawn.
4. THE Lottery_Module SHALL expose a `get_prize_pool` function accepting `round_id: u64` that returns the current `i128` Prize_Pool balance for that round.
5. THE Lottery_Module SHALL expose a `get_active_round` function accepting `token: Address` that returns the `round_id` of the currently `Open` round for that token, or `None` if no active round exists.

---

### Requirement 7: Lottery Events for Off-Chain Indexing

**User Story:** As an off-chain indexer, I want all lottery state transitions to emit structured events, so that I can reconstruct lottery history without querying contract storage directly.

#### Acceptance Criteria

1. THE Lottery_Module SHALL emit a `lottery_created` event with topic `(symbol_short!("lott_crt"), round_id)` and data `(token, max_winners, prize_pool_fee_bps, end_ledger)` when a round is created.
2. THE Lottery_Module SHALL emit a `lottery_entry` event with topic `(symbol_short!("lott_ent"), round_id)` and data `(sender, tickets_awarded, total_tickets_for_sender)` when an entry is recorded or updated.
3. THE Lottery_Module SHALL emit a `prize_contributed` event with topic `(symbol_short!("lott_con"), round_id)` and data `(contributor, amount)` when a direct contribution is made.
4. THE Lottery_Module SHALL emit a `lottery_drawn` event with topic `(symbol_short!("lott_drw"), round_id)` and data `(winner_count, prize_pool_total)` when winners are selected.
5. THE Lottery_Module SHALL emit a `lottery_no_entries` event with topic `(symbol_short!("lott_emp"), round_id)` and data `()` when a draw is attempted on a round with zero entries.
6. THE Lottery_Module SHALL emit a `prize_claimed` event with topic `(symbol_short!("lott_clm"), round_id)` and data `(winner, amount)` when a prize is claimed.
7. THE Lottery_Module SHALL use `env.events().publish(topic, data)` for all events, consistent with the existing event emission pattern in `lib.rs`.

---

### Requirement 8: Lottery Error Handling

**User Story:** As a contract integrator, I want all invalid lottery operations to return descriptive error codes, so that client applications can surface meaningful messages to users.

#### Acceptance Criteria

1. THE Lottery_Module SHALL define a `LotteryError` enum annotated with `#[contracterror]` containing at minimum: `RoundNotFound`, `RoundNotOpen`, `RoundNotClosed`, `RoundNotEnded`, `ActiveRoundExists`, `EntryNotFound`, `NotAWinner`, `PrizeAlreadyClaimed`, `EmptyPrizePool`, `InvalidTicketPrice`, `InvalidWinnerCount`, `InvalidPrizePoolFee`, `InvalidEndLedger`, `InvalidAmount`, `TokenMismatch`, `Unauthorized`, `ContractPaused`.
2. IF an arithmetic operation on prize amounts would overflow `i128`, THE Lottery_Module SHALL use Rust's `checked_add` / `checked_sub` / `checked_mul` and panic with a descriptive message rather than silently wrapping.
3. THE Lottery_Module SHALL assign non-overlapping `u32` discriminant values to `LotteryError` variants, starting at 200 to avoid collisions with existing error enums in `lib.rs`.
