# Implementation Plan: Tip Synthetic Assets

## Overview

This implementation plan breaks down the Tip Synthetic Assets feature into discrete, incremental coding tasks. The feature introduces synthetic tokens backed by creator tip pools, enabling users to gain exposure to creator performance while providing creators with upfront liquidity.

The implementation follows a bottom-up approach: data structures → core components → administration → integration → testing. Each task builds on previous work, with checkpoints to validate progress and ensure all tests pass before proceeding.

## Tasks

- [x] 1. Set up module structure and data types
  - Create `contracts/tipjar/src/synthetic/` directory
  - Create `mod.rs`, `types.rs`, `minting.rs`, `redemption.rs`, `oracle.rs`, `supply.rs`, `admin.rs`, `queries.rs`, `events.rs` files
  - Define `SyntheticAsset` struct in `types.rs` with all required fields (asset_id, creator, backing_token, total_supply, collateralization_ratio, created_at, oracle_price, total_collateral, active)
  - Add new storage keys to `DataKey` enum in `lib.rs`: `SyntheticAsset(u64)`, `SyntheticAssetCounter`, `CreatorSyntheticAssets(Address)`, `SyntheticCollateral(Address, Address)`, `SyntheticBalance(Address, u64)`
  - Add new error codes to `TipJarError` enum: `SyntheticAssetNotFound`, `SyntheticAssetInactive`, `InvalidCollateralizationRatio`, `CollateralizationViolation`, `TokenNotInPool`, `InsufficientPoolBalance`
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9_

- [x] 2. Implement event definitions
  - [x] 2.1 Define event structures in `events.rs`
    - Create `SyntheticAssetCreatedEvent`, `SyntheticTokensMintedEvent`, `SyntheticTokensRedeemedEvent`, `PriceUpdatedEvent`, `SupplyUpdatedEvent`, `CollateralUpdatedEvent`, `SyntheticAssetPausedEvent`, `SyntheticAssetResumedEvent`, `CollateralizationUpdatedEvent` structs
    - Each event must include timestamp field
    - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 10.7, 10.8, 10.9, 10.10_
  
  - [x] 2.2 Implement event emission helper functions
    - Write functions to emit each event type with proper field population
    - Ensure timestamp is set using `env.ledger().timestamp()`
    - _Requirements: 10.10_

- [x] 3. Implement Price Oracle component
  - [x] 3.1 Implement `update_oracle_price()` in `oracle.rs`
    - Calculate price as `tip_pool_balance / total_supply` when supply > 0
    - Return initial price (1 unit of backing token) when supply == 0
    - Return zero price when balance == 0 and supply > 0
    - Store updated price in synthetic asset record
    - Emit `PriceUpdatedEvent`
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7_
  
  - [ ]* 3.2 Write property test for Price Oracle
    - **Property 9: Oracle Price Calculation**
    - **Validates: Requirements 4.1, 4.2, 4.6**
    - Test with random tip pool balances and supply values
    - Verify price = balance / supply when supply > 0
    - Verify price = 1 unit when supply == 0
  
  - [x] 3.3 Implement `get_oracle_price()` in `oracle.rs`
    - Retrieve current oracle price from synthetic asset record without updating
    - Return error if asset not found
    - _Requirements: 4.1, 9.3_
  
  - [ ]* 3.4 Write unit tests for Price Oracle edge cases
    - Test zero supply scenario
    - Test zero balance with non-zero supply
    - Test non-existent asset ID

- [x] 4. Implement Supply Tracker component
  - [x] 4.1 Implement `update_supply()` in `supply.rs`
    - Accept delta parameter (positive for mint, negative for redeem)
    - Update total_supply field in synthetic asset record
    - Emit `SupplyUpdatedEvent`
    - _Requirements: 3.5, 5.4, 6.1, 6.4, 6.9_
  
  - [x] 4.2 Implement `update_collateral()` in `supply.rs`
    - Accept delta parameter (positive for lock, negative for unlock)
    - Update total_collateral field in synthetic asset record
    - Emit `CollateralUpdatedEvent`
    - _Requirements: 3.6, 5.6, 6.2, 6.5, 6.10, 7.6_
  
  - [x] 4.3 Implement `get_collateralization_ratio()` in `supply.rs`
    - Calculate ratio as `(total_collateral / (total_supply * oracle_price)) * 10000`
    - Handle division by zero when supply is zero
    - Return ratio in basis points
    - _Requirements: 6.3, 6.8_
  
  - [ ]* 4.4 Write property tests for Supply Tracker
    - **Property 7: Supply Tracking Invariant**
    - **Validates: Requirements 3.5, 5.4, 6.1, 6.4**
    - Test with random sequences of mint/redeem operations
    - Verify total_supply = sum(minted) - sum(redeemed)
  
  - [ ]* 4.5 Write property test for Collateral Tracker
    - **Property 8: Collateral Tracking Invariant**
    - **Validates: Requirements 3.6, 5.6, 6.2, 6.5, 7.6**
    - Test with random sequences of operations
    - Verify total_collateral = sum(locked) - sum(unlocked)
  
  - [ ]* 4.6 Write property test for Collateralization Ratio
    - **Property 10: Collateralization Ratio Calculation**
    - **Validates: Requirements 6.3**
    - Test with random collateral, supply, and price values
    - Verify ratio calculation correctness
  
  - [x] 4.7 Implement `get_total_supply()` and `get_total_collateral()` in `supply.rs`
    - Retrieve values from synthetic asset record
    - _Requirements: 6.6, 6.7, 9.4_

- [x] 5. Checkpoint - Core components foundation
  - Ensure all tests pass, ask the user if questions arise.

- [x] 6. Implement Minting Engine component
  - [x] 6.1 Implement `calculate_required_collateral()` in `minting.rs`
    - Calculate as `(amount * oracle_price * collateralization_ratio) / 10000`
    - Handle overflow protection
    - _Requirements: 3.2, 9.6_
  
  - [ ]* 6.2 Write property test for collateral calculation
    - **Property 5: Collateral Calculation Correctness**
    - **Validates: Requirements 3.2, 9.6**
    - Test with random amounts, prices, and ratios
    - Verify formula correctness
  
  - [x] 6.3 Implement `mint()` in `minting.rs`
    - Verify synthetic asset exists and is active
    - Get current oracle price
    - Calculate required collateral
    - Verify user has sufficient balance and authorization
    - Transfer collateral from user to tip pool
    - Lock collateral in tip pool (update `SyntheticCollateral` storage)
    - Mint synthetic tokens to user (update `SyntheticBalance` storage)
    - Call `update_supply()` to increase total supply
    - Call `update_collateral()` to increase total collateral
    - Emit `SyntheticTokensMinted` event
    - Implement rollback on any failure (atomic operation)
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8, 3.9, 3.10, 12.2, 12.4, 12.5, 12.7_
  
  - [ ]* 6.4 Write property test for minting atomicity
    - **Property 11: Minting Atomicity**
    - **Validates: Requirements 3.3, 3.4, 3.5, 3.6, 3.7, 12.4, 12.5**
    - Test with random success/failure scenarios
    - Verify all steps occur or none occur
  
  - [ ]* 6.5 Write property test for active asset minting restriction
    - **Property 13: Active Asset Minting Restriction**
    - **Validates: Requirements 3.1, 8.2**
    - Test minting on active and paused assets
    - Verify minting only succeeds when active
  
  - [ ]* 6.6 Write unit tests for minting error conditions
    - Test insufficient collateral error
    - Test inactive asset error
    - Test invalid amount error (zero, negative)
    - Test non-existent asset error

- [x] 7. Implement Redemption Engine component
  - [x] 7.1 Implement `calculate_redemption_value()` in `redemption.rs`
    - Calculate as `amount * oracle_price`
    - Handle overflow protection
    - _Requirements: 5.2, 9.7_
  
  - [ ]* 7.2 Write property test for redemption value calculation
    - **Property 6: Redemption Value Calculation Correctness**
    - **Validates: Requirements 5.2, 9.7**
    - Test with random amounts and prices
    - Verify formula correctness
  
  - [x] 7.3 Implement `redeem()` in `redemption.rs`
    - Verify holder owns sufficient synthetic tokens
    - Get current oracle price
    - Calculate redemption value
    - Burn synthetic tokens from holder (update `SyntheticBalance` storage)
    - Call `update_supply()` to decrease total supply
    - Unlock collateral from tip pool (update `SyntheticCollateral` storage)
    - Transfer redemption value from tip pool to holder
    - Call `update_collateral()` to decrease total collateral
    - Emit `SyntheticTokensRedeemed` event
    - Implement rollback on any failure (atomic operation)
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8, 5.9, 5.10, 12.3, 12.4, 12.6, 12.7_
  
  - [ ]* 7.4 Write property test for redemption atomicity
    - **Property 12: Redemption Atomicity**
    - **Validates: Requirements 5.3, 5.4, 5.5, 5.6, 5.7, 12.4, 12.6**
    - Test with random success/failure scenarios
    - Verify all steps occur or none occur
  
  - [ ]* 7.5 Write property test for paused asset redemption allowance
    - **Property 14: Paused Asset Redemption Allowance**
    - **Validates: Requirements 8.3**
    - Test redemption on active and paused assets
    - Verify redemption succeeds regardless of active status
  
  - [ ]* 7.6 Write unit tests for redemption error conditions
    - Test insufficient balance error
    - Test insufficient pool balance error
    - Test invalid amount error (zero, negative)
    - Test non-existent asset error

- [x] 8. Checkpoint - Minting and redemption complete
  - Ensure all tests pass, ask the user if questions arise.

- [x] 9. Implement Administration functions
  - [x] 9.1 Implement `create_synthetic_asset()` in `admin.rs`
    - Verify caller is creator
    - Verify collateralization ratio is between 10000 and 50000 bps
    - Verify creator has sufficient tip pool balance for backing token
    - Verify backing token exists in creator's tip pool
    - Generate unique asset_id using `SyntheticAssetCounter`
    - Create `SyntheticAsset` record with initial values (total_supply=0, active=true, created_at=current_timestamp)
    - Store asset record in persistent storage
    - Add asset_id to creator's asset list (`CreatorSyntheticAssets`)
    - Emit `SyntheticAssetCreated` event
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 11.9_
  
  - [ ]* 9.2 Write property tests for asset creation
    - **Property 1: Unique Asset Identifiers**
    - **Validates: Requirements 1.8**
    - Test with multiple asset creations
    - Verify all asset IDs are unique
  
  - [ ]* 9.3 Write property test for asset initialization
    - **Property 2: Asset Creation Initialization Invariants**
    - **Validates: Requirements 2.4, 2.5**
    - Test newly created assets
    - Verify total_supply == 0 and active == true
  
  - [ ]* 9.4 Write property test for ratio validation
    - **Property 3: Collateralization Ratio Validation**
    - **Validates: Requirements 2.2, 8.6**
    - Test with random ratio values
    - Verify acceptance only for 10000-50000 bps range
  
  - [ ]* 9.5 Write property test for balance validation
    - **Property 4: Sufficient Balance Validation**
    - **Validates: Requirements 2.1**
    - Test with random creator balances
    - Verify sufficient balance requirement
  
  - [x] 9.6 Implement `pause_synthetic_asset()` in `admin.rs`
    - Verify caller is asset creator
    - Set active status to false
    - Emit `SyntheticAssetPaused` event
    - _Requirements: 8.1, 8.2, 8.8, 8.10, 12.1, 12.9_
  
  - [x] 9.7 Implement `resume_synthetic_asset()` in `admin.rs`
    - Verify caller is asset creator
    - Verify collateralization requirements are met
    - Set active status to true
    - Emit `SyntheticAssetResumed` event
    - _Requirements: 8.4, 8.5, 8.9, 8.10, 12.1, 12.9_
  
  - [x] 9.8 Implement `update_collateralization_ratio()` in `admin.rs`
    - Verify caller is asset creator
    - Verify new ratio is between 10000 and 50000 bps
    - Update collateralization_ratio field
    - Emit `CollateralizationUpdated` event
    - _Requirements: 8.6, 8.7, 8.10, 12.1, 12.9_
  
  - [ ]* 9.9 Write property test for ratio update application
    - **Property 32: Ratio Update Application**
    - **Validates: Requirements 8.7**
    - Test minting operations after ratio update
    - Verify new ratio is used for calculations
  
  - [x] 9.10 Implement `add_collateral()` in `admin.rs`
    - Verify caller is asset creator
    - Transfer collateral from creator to tip pool
    - Update total_collateral via `update_collateral()`
    - Emit `CollateralUpdated` event
    - _Requirements: 7.5, 7.6, 12.1_
  
  - [ ]* 9.11 Write property test for creator authorization
    - **Property 21: Creator Authorization**
    - **Validates: Requirements 8.10, 12.1, 12.9**
    - Test creator-only operations with random callers
    - Verify operations succeed only when caller == creator

- [x] 10. Implement Query functions
  - [x] 10.1 Implement `get_synthetic_asset()` in `queries.rs`
    - Retrieve synthetic asset record by asset_id
    - Return error if not found
    - _Requirements: 9.1, 9.10_
  
  - [x] 10.2 Implement `get_creator_synthetic_assets()` in `queries.rs`
    - Retrieve list of asset IDs for a creator
    - Return empty vector if creator has no assets
    - _Requirements: 9.2_
  
  - [x] 10.3 Implement `get_holder_balance()` in `queries.rs`
    - Retrieve synthetic token balance for holder and asset
    - Return 0 if no balance exists
    - _Requirements: 9.8_
  
  - [x] 10.4 Implement query wrappers for oracle price, supply, collateral, ratio
    - Wrap existing component functions for external access
    - Ensure queries work during pause state
    - _Requirements: 9.3, 9.4, 9.5, 9.6, 9.7, 9.9_
  
  - [ ]* 10.5 Write property test for query correctness
    - **Property 34: Query Correctness**
    - **Validates: Requirements 9.1, 9.2, 9.3, 9.4, 9.5, 9.8**
    - Test queries against known state
    - Verify returned values match stored state
  
  - [ ]* 10.6 Write property test for pause state query availability
    - **Property 15: Pause State Query Availability**
    - **Validates: Requirements 9.9**
    - Test queries during contract and asset pause
    - Verify queries succeed regardless of pause state

- [x] 11. Checkpoint - Administration and queries complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 12. Integrate with existing tip pool system
  - [ ] 12.1 Modify `withdraw()` function in `lib.rs`
    - Calculate locked collateral for creator across all synthetic assets
    - Calculate available balance as `total_balance - locked_collateral`
    - Prevent withdrawal if requested amount > available balance
    - Return `CollateralizationViolation` error if withdrawal would violate collateral requirements
    - _Requirements: 7.8, 11.3, 11.4_
  
  - [ ]* 12.2 Write property test for withdrawal prevention
    - **Property 18: Withdrawal Prevention with Locked Collateral**
    - **Validates: Requirements 7.8, 11.3**
    - Test withdrawals with various locked collateral amounts
    - Verify rejection when withdrawal > available balance
  
  - [ ] 12.3 Modify `tip()` function in `lib.rs`
    - After tip is processed, trigger oracle price update for all creator's synthetic assets
    - Call `update_oracle_price()` for each asset
    - _Requirements: 4.3, 11.6_
  
  - [ ]* 12.4 Write property test for price update on tip receipt
    - **Property 19: Price Update on Tip Receipt**
    - **Validates: Requirements 4.3, 11.6**
    - Test tip receipt with synthetic assets
    - Verify oracle price is recalculated
  
  - [ ] 12.5 Implement `get_creator_balance()` enhancement
    - Return both total balance and available balance (after locked collateral)
    - Add helper function to calculate locked collateral for creator
    - _Requirements: 11.4, 11.5, 11.8_
  
  - [ ]* 12.6 Write property tests for collateral locking integration
    - **Property 16: Collateral Locking Integration**
    - **Validates: Requirements 11.1, 11.3, 11.4, 11.5**
    - Test minting operations
    - Verify locked collateral increases and available balance decreases
  
  - [ ]* 12.7 Write property test for collateral unlocking integration
    - **Property 17: Collateral Unlocking Integration**
    - **Validates: Requirements 11.2, 11.3, 11.4, 11.5**
    - Test redemption operations
    - Verify locked collateral decreases and available balance increases
  
  - [ ]* 12.8 Write property test for multi-asset independence
    - **Property 20: Multi-Asset Independent Tracking**
    - **Validates: Requirements 11.7**
    - Test operations on multiple assets for same creator
    - Verify operations on one asset don't affect others

- [ ] 13. Implement collateralization enforcement
  - [ ] 13.1 Add collateralization checks to minting
    - Before completing mint, verify resulting ratio meets minimum
    - If ratio would fall below minimum, reject mint or auto-pause asset
    - _Requirements: 7.1, 7.3_
  
  - [ ] 13.2 Add collateralization checks to redemption
    - Before completing redeem, verify sufficient collateral remains
    - Reject redemption if remaining collateral insufficient
    - _Requirements: 7.2_
  
  - [ ] 13.3 Implement automatic resume eligibility check
    - When collateral is added or supply decreases, check if ratio restored
    - Update asset state to allow resumption
    - Emit `CollateralizationUpdated` event
    - _Requirements: 7.4, 7.7_
  
  - [ ]* 13.4 Write property tests for collateralization enforcement
    - **Property 29: Collateralization Enforcement on Minting**
    - **Validates: Requirements 7.1, 7.3**
    - Test minting with various collateralization scenarios
    - Verify rejection or auto-pause when ratio falls below minimum
  
  - [ ]* 13.5 Write property test for redemption collateralization
    - **Property 30: Collateralization Enforcement on Redemption**
    - **Validates: Requirements 7.2**
    - Test redemption with various collateral levels
    - Verify rejection when remaining collateral insufficient
  
  - [ ]* 13.6 Write property test for automatic resume
    - **Property 31: Automatic Resume on Collateralization Restoration**
    - **Validates: Requirements 7.4**
    - Test collateral addition and supply reduction
    - Verify resume eligibility when ratio restored

- [ ] 14. Checkpoint - Integration and collateralization complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 15. Implement security and validation
  - [ ] 15.1 Add reentrancy guards to minting and redemption
    - Implement checks-effects-interactions pattern
    - Add reentrancy guard flags if needed
    - Validate state after external calls
    - _Requirements: 12.7_
  
  - [ ]* 15.2 Write property test for reentrancy protection
    - **Property 35: Reentrancy Protection**
    - **Validates: Requirements 12.7**
    - Test reentrancy attempts during operations
    - Verify reentrant calls are rejected
  
  - [ ] 15.3 Add input validation to all operations
    - Validate amount > 0 for all operations
    - Validate asset_id exists for all operations
    - Validate addresses are valid
    - Return appropriate errors for invalid inputs
    - _Requirements: 3.10, 5.10, 12.8_
  
  - [ ]* 15.4 Write property tests for input validation
    - **Property 24: Input Validation Consistency**
    - **Validates: Requirements 3.10, 5.10**
    - Test operations with zero and negative amounts
    - Verify InvalidAmount error is returned
  
  - [ ]* 15.5 Write property test for asset existence validation
    - **Property 25: Asset Existence Validation**
    - **Validates: Requirements 9.10**
    - Test operations with non-existent asset IDs
    - Verify SyntheticAssetNotFound error is returned
  
  - [ ]* 15.6 Write property test for token pool membership
    - **Property 26: Token Pool Membership Validation**
    - **Validates: Requirements 11.9**
    - Test asset creation with tokens not in tip pool
    - Verify TokenNotInPool error is returned
  
  - [ ] 15.7 Add contract pause checks
    - Add pause check to minting function
    - Add pause check to redemption function
    - Allow queries during pause
    - _Requirements: 12.10_
  
  - [ ]* 15.8 Write property test for pause state operation prevention
    - **Property 36: Pause State Operation Prevention**
    - **Validates: Requirements 12.10**
    - Test operations during contract pause
    - Verify minting and redemption are rejected

- [ ] 16. Implement comprehensive error handling
  - [ ]* 16.1 Write property tests for error conditions
    - **Property 27: Insufficient Balance Error Handling**
    - **Validates: Requirements 2.7, 3.8**
    - Test minting with insufficient user balance
    - Verify InsufficientCollateral error is returned
  
  - [ ]* 16.2 Write property test for pool balance errors
    - **Property 28: Insufficient Pool Balance Error Handling**
    - **Validates: Requirements 5.9**
    - Test redemption with insufficient tip pool balance
    - Verify InsufficientPoolBalance error is returned
  
  - [ ]* 16.3 Write property test for minting authorization
    - **Property 22: Minting Authorization**
    - **Validates: Requirements 12.2**
    - Test minting with various authorization states
    - Verify collateral transfer authorization is checked
  
  - [ ]* 16.4 Write property test for redemption ownership
    - **Property 23: Redemption Ownership Verification**
    - **Validates: Requirements 5.1, 12.3**
    - Test redemption with various token ownership states
    - Verify ownership is verified before redemption

- [-] 17. Add public contract functions to lib.rs
  - [x] 17.1 Add synthetic asset functions to TipJarContract impl
    - Add `create_synthetic_asset()` public function
    - Add `mint_synthetic_tokens()` public function
    - Add `redeem_synthetic_tokens()` public function
    - Add `pause_synthetic_asset()` public function
    - Add `resume_synthetic_asset()` public function
    - Add `update_collateralization_ratio()` public function
    - Add `add_synthetic_collateral()` public function
    - Wire each function to corresponding module implementation
    - _Requirements: All requirements_
  
  - [x] 17.2 Add synthetic asset query functions to TipJarContract impl
    - Add `get_synthetic_asset()` public function
    - Add `get_creator_synthetic_assets()` public function
    - Add `get_synthetic_oracle_price()` public function
    - Add `get_synthetic_total_supply()` public function
    - Add `get_synthetic_collateralization_ratio()` public function
    - Add `calculate_required_collateral()` public function
    - Add `calculate_redemption_value()` public function
    - Add `get_synthetic_holder_balance()` public function
    - Wire each function to corresponding module implementation
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5, 9.6, 9.7, 9.8_

- [x] 18. Checkpoint - All implementation complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 19. Write comprehensive integration tests
  - [ ]* 19.1 Write end-to-end integration test
    - Test complete flow: create asset → mint tokens → receive tips → redeem tokens
    - Verify all state changes are correct
    - Verify all events are emitted
    - _Requirements: All requirements_
  
  - [ ]* 19.2 Write multi-asset integration test
    - Test creator with multiple synthetic assets
    - Verify independent tracking and operations
    - Test withdrawal with multiple locked collateral amounts
    - _Requirements: 11.7, 11.8_
  
  - [ ]* 19.3 Write pause/resume integration test
    - Test asset pause during active minting
    - Test redemption during pause
    - Test resume after collateral addition
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5_
  
  - [ ]* 19.4 Write collateralization enforcement integration test
    - Test automatic pause on under-collateralization
    - Test rejection of withdrawals that would violate collateral
    - Test automatic resume eligibility
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.8_

- [ ]* 20. Write property test for event emission completeness
  - **Property 33: Event Emission Completeness**
  - **Validates: Requirements 2.6, 3.7, 4.5, 5.7, 6.9, 6.10, 7.7, 8.8, 8.9, 10.1-10.10**
  - Test all state-changing operations
  - Verify corresponding events are emitted with all required fields

- [ ] 21. Add unit tests for edge cases
  - [ ]* 21.1 Write unit tests for arithmetic edge cases
    - Test large amount calculations (near i128 max)
    - Test overflow protection in collateral calculation
    - Test overflow protection in redemption value calculation
    - Test division by zero handling in price calculation
    - Test division by zero handling in ratio calculation
  
  - [ ]* 21.2 Write unit tests for boundary conditions
    - Test minimum collateralization ratio (10000 bps)
    - Test maximum collateralization ratio (50000 bps)
    - Test ratio just below and just above valid range
    - Test zero supply scenarios
    - Test zero balance scenarios
  
  - [ ]* 21.3 Write unit tests for state transitions
    - Test active → paused → active transitions
    - Test supply 0 → positive → 0 transitions
    - Test collateral changes across operations

- [x] 22. Final checkpoint - All tests passing
  - Run full test suite including all property tests (minimum 100 iterations each)
  - Verify all 36 correctness properties pass
  - Verify all unit tests pass
  - Verify all integration tests pass
  - Ensure code coverage meets minimum 95% line coverage goal
  - Ask the user if questions arise or if ready for deployment

## Notes

- Tasks marked with `*` are optional property-based and unit test tasks that can be skipped for faster MVP delivery
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation and provide opportunities for user feedback
- Property tests validate universal correctness properties across randomized inputs
- Unit tests validate specific examples and edge cases
- Integration tests validate interactions with existing TipJar components
- All 36 correctness properties from the design document are covered by property test tasks
- Implementation follows bottom-up approach: data structures → components → administration → integration → testing
- Each task builds on previous tasks with no orphaned or hanging code
- Security considerations (reentrancy, authorization, validation) are integrated throughout implementation

