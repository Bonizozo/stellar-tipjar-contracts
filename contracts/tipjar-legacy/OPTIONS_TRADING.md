# Tip Options Trading System

## Overview

The Tip Options Trading system allows users to trade call and put options on tip tokens within the Stellar TipJar contract. This feature enables sophisticated financial strategies for creators and supporters, including hedging, speculation, and income generation through premium collection.

## Features

### 1. Option Contracts
- **Call Options**: Right to buy tokens at a strike price
- **Put Options**: Right to sell tokens at a strike price
- **Collateralized**: All options are fully collateralized to ensure settlement
- **Time-bound**: Options have expiration dates

### 2. Option Pricing
- Simplified Black-Scholes-inspired pricing model
- Factors considered:
  - Spot price vs. strike price (moneyness)
  - Time to expiration
  - Volatility
  - Risk-free rate
- Configurable pricing parameters (admin only)

### 3. Exercise Functionality
- Holders can exercise in-the-money options
- Automatic settlement with collateral
- Payoff calculation based on spot price at exercise

### 4. Expiration Handling
- Options automatically expire after expiration time
- Collateral returned to writer upon expiration
- Batch expiration support for efficiency

### 5. Position Tracking
- Track written options per address
- Track held options per address
- Monitor collateral locked
- Track premiums earned and paid

## Contract Functions

### Initialization

#### `init_options_trading(admin: Address)`
Initialize the options trading system with default pricing parameters.

**Access**: Admin only

**Emits**: `("opt_init",)`

### Writing Options

#### `write_option(writer: Address, option_type: OptionType, token: Address, strike_price: i128, amount: i128, expiration: u64) -> u64`
Create a new option contract by locking collateral.

**Parameters**:
- `writer`: Address creating the option
- `option_type`: `Call` or `Put`
- `token`: Underlying token address
- `strike_price`: Strike price in base units
- `amount`: Number of tokens covered
- `expiration`: Unix timestamp for expiration

**Returns**: Option ID

**Collateral Requirements**:
- Call: 100% of token amount
- Put: 100% of strike_price × amount

**Emits**: `("opt_wrt",)` with `(option_id, writer, option_type, strike_price, amount, expiration)`

### Buying Options

#### `buy_option(buyer: Address, option_id: u64, spot_price: i128)`
Purchase an option by paying the premium to the writer.

**Parameters**:
- `buyer`: Address buying the option
- `option_id`: ID of the option to buy
- `spot_price`: Current market price for premium calculation

**Premium Calculation**: Automatically calculated based on:
- Intrinsic value (immediate exercise value)
- Time value (volatility × time × moneyness)
- Min/max bounds from pricing parameters

**Emits**: `("opt_buy",)` with `(option_id, buyer, premium)`

### Exercising Options

#### `exercise_option(holder: Address, option_id: u64, spot_price: i128) -> i128`
Exercise an in-the-money option.

**Parameters**:
- `holder`: Address exercising the option
- `option_id`: ID of the option to exercise
- `spot_price`: Current market price for settlement

**Returns**: Payoff amount

**Requirements**:
- Caller must be the holder
- Option must be active
- Option must not be expired
- Option must be in the money

**Settlement**:
- **Call**: Holder receives tokens, pays strike price to writer
- **Put**: Holder delivers tokens, receives strike price from collateral

**Emits**: `("opt_exer",)` with `(option_id, holder, payoff)`

### Expiration

#### `expire_option(option_id: u64)`
Expire an option that has passed its expiration time.

**Parameters**:
- `option_id`: ID of the option to expire

**Access**: Anyone can call after expiration

**Effects**:
- Returns collateral to writer
- Marks option as expired
- Updates position tracking

**Emits**: `("opt_exp",)` with `option_id`

#### `batch_expire_options(option_ids: Vec<u64>) -> u32`
Expire multiple options in a single transaction.

**Parameters**:
- `option_ids`: Vector of option IDs to expire

**Returns**: Count of successfully expired options

**Emits**: `("opt_bexp",)` with `expired_count`

### Cancellation

#### `cancel_option(writer: Address, option_id: u64)`
Cancel an unsold option before it's purchased.

**Parameters**:
- `writer`: Address that wrote the option
- `option_id`: ID of the option to cancel

**Requirements**:
- Caller must be the writer
- Option must not have a holder yet
- Option must be active

**Effects**:
- Returns collateral to writer
- Marks option as cancelled

**Emits**: `("opt_canc",)` with `option_id`

### Query Functions

#### `get_option(option_id: u64) -> Option<OptionContract>`
Get option contract details by ID.

#### `get_written_options(writer: Address) -> Vec<u64>`
Get all option IDs written by an address.

#### `get_held_options(holder: Address) -> Vec<u64>`
Get all option IDs held by an address.

#### `get_option_position(address: Address) -> OptionPosition`
Get position summary for an address including:
- Written count
- Held count
- Total collateral locked
- Premiums earned
- Premiums paid

#### `get_active_options() -> Vec<u64>`
Get all currently active option IDs.

#### `calculate_option_premium(option_type: OptionType, spot_price: i128, strike_price: i128, amount: i128, time_to_expiry: u64) -> i128`
Calculate premium for given parameters without creating an option.

### Admin Functions

#### `update_option_pricing(admin: Address, params: PricingParams)`
Update pricing parameters.

**Parameters**:
- `admin`: Admin address
- `params`: New pricing parameters
  - `volatility_bps`: Volatility in basis points (e.g., 5000 = 50%)
  - `risk_free_rate_bps`: Risk-free rate in basis points
  - `min_premium_bps`: Minimum premium as % of strike
  - `max_premium_bps`: Maximum premium as % of strike

**Access**: Admin only

**Emits**: `("opt_prm",)` with `params`

#### `get_option_pricing_params() -> PricingParams`
Get current pricing parameters.

## Data Types

### OptionType
```rust
enum OptionType {
    Call,
    Put,
}
```

### OptionStatus
```rust
enum OptionStatus {
    Active,      // Option is active and tradeable
    Exercised,   // Option has been exercised
    Expired,     // Option has expired
    Cancelled,   // Option was cancelled by writer
}
```

### OptionContract
```rust
struct OptionContract {
    option_id: u64,
    option_type: OptionType,
    writer: Address,
    holder: Option<Address>,
    token: Address,
    strike_price: i128,
    premium: i128,
    amount: i128,
    expiration: u64,
    created_at: u64,
    status: OptionStatus,
    collateral: i128,
}
```

### OptionPosition
```rust
struct OptionPosition {
    address: Address,
    written_count: u32,
    held_count: u32,
    total_collateral: i128,
    premiums_earned: i128,
    premiums_paid: i128,
}
```

### PricingParams
```rust
struct PricingParams {
    volatility_bps: u32,
    risk_free_rate_bps: u32,
    min_premium_bps: u32,
    max_premium_bps: u32,
}
```

## Usage Examples

### Example 1: Writing and Selling a Call Option

```rust
// Writer creates a call option
let option_id = contract.write_option(
    &writer,
    &OptionType::Call,
    &token,
    &1_000_000,  // Strike price
    &10_000_000, // Amount
    &(now + 86400), // Expires in 1 day
);

// Buyer purchases the option
contract.buy_option(
    &buyer,
    &option_id,
    &1_200_000, // Current spot price
);

// Buyer exercises if profitable
let payoff = contract.exercise_option(
    &buyer,
    &option_id,
    &1_500_000, // Spot price at exercise
);
```

### Example 2: Writing a Put Option for Hedging

```rust
// Creator writes put option to hedge against price drops
let option_id = contract.write_option(
    &creator,
    &OptionType::Put,
    &token,
    &1_000_000,  // Floor price
    &50_000_000, // Amount to hedge
    &(now + 604800), // 1 week
);

// Supporter buys the put as insurance
contract.buy_option(
    &supporter,
    &option_id,
    &1_000_000,
);
```

### Example 3: Batch Expiration

```rust
// Get all active options
let active_options = contract.get_active_options();

// Filter expired options
let now = env.ledger().timestamp();
let mut expired_ids = Vec::new(&env);

for id in active_options.iter() {
    if let Some(option) = contract.get_option(&id) {
        if option.expiration <= now && option.status == OptionStatus::Active {
            expired_ids.push_back(id);
        }
    }
}

// Batch expire
let count = contract.batch_expire_options(&expired_ids);
```

## Pricing Model

The pricing model uses a simplified approach suitable for on-chain computation:

### Premium Calculation
```
Premium = Intrinsic Value + Time Value
```

### Intrinsic Value
- **Call**: max(spot - strike, 0) × amount
- **Put**: max(strike - spot, 0) × amount

### Time Value
```
Time Value = spot × amount × volatility × time_factor × atm_adjustment
```

Where:
- `time_factor`: Proportion of time to expiry (capped at 1 year)
- `atm_adjustment`: Reduction factor for out-of-the-money options
- `volatility`: Configured volatility parameter

### Bounds
- Minimum: `strike × amount × min_premium_bps / 10_000`
- Maximum: `strike × amount × max_premium_bps / 10_000`

## Security Considerations

1. **Full Collateralization**: All options are fully collateralized to prevent default risk
2. **Atomic Settlement**: Exercise and settlement happen atomically
3. **Access Control**: Only holders can exercise, only writers can cancel unsold options
4. **Expiration Checks**: Prevents exercise of expired options
5. **Moneyness Validation**: Prevents exercise of out-of-the-money options

## Gas Optimization

- Batch expiration for multiple options
- Efficient storage using persistent storage for long-lived data
- Position tracking aggregates to reduce reads

## Error Codes

- `OptionNotFound (87)`: Option ID does not exist
- `OptionNotActive (88)`: Option is not in active status
- `OptionExpired (89)`: Option has expired
- `OptionOutOfMoney (90)`: Option cannot be exercised (out of money)
- `NotOptionHolder (91)`: Caller is not the option holder
- `NotOptionWriter (92)`: Caller is not the option writer
- `OptionAlreadySold (93)`: Option already has a holder
- `InsufficientCollateral (94)`: Insufficient collateral for option
- `InvalidOptionParams (95)`: Invalid option parameters
- `OptionNotExpired (96)`: Option has not expired yet

## Future Enhancements

1. **American Options**: Allow early exercise before expiration
2. **Option Marketplace**: Secondary market for trading options
3. **Automated Market Making**: AMM for option liquidity
4. **Implied Volatility Oracle**: On-chain volatility estimation
5. **Option Strategies**: Pre-built strategies (spreads, straddles, etc.)
6. **Partial Exercise**: Exercise options in smaller increments
7. **Cash Settlement**: Settle in cash instead of physical delivery
8. **Greeks Calculation**: Delta, gamma, theta, vega for risk management
