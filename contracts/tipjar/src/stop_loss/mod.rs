//! Tip Stop Loss Orders
//!
//! Allows users to place stop loss orders that automatically close positions
//! when a price falls below (or rises above) a trigger price.
//! Supports standard stop loss, stop limit, and trailing stop orders.

use soroban_sdk::{contracttype, panic_with_error, symbol_short, token, Address, Env, Vec};

// ── Types ────────────────────────────────────────────────────────────────────

/// Kind of stop loss order.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum StopOrderKind {
    /// Triggers a market sell when price ≤ stop_price.
    StopLoss,
    /// Triggers a limit sell at limit_price when price ≤ stop_price.
    StopLimit,
    /// Stop price trails the market by `trail_amount`; triggers on reversal.
    TrailingStop,
}

/// Status of a stop loss order.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum StopOrderStatus {
    Active,
    Triggered,
    Executed,
    Cancelled,
}

/// A stop loss order record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StopOrder {
    pub order_id: u64,
    pub owner: Address,
    /// Token being protected (sold on trigger).
    pub token: Address,
    /// Amount of token held in escrow for this order.
    pub amount: i128,
    /// Price (× PRICE_PRECISION) at which the order triggers.
    pub stop_price: i128,
    /// For StopLimit: the minimum acceptable execution price.
    pub limit_price: i128,
    /// For TrailingStop: distance below the peak price that triggers execution.
    pub trail_amount: i128,
    /// Highest observed price since order creation (used for trailing stops).
    pub peak_price: i128,
    pub kind: StopOrderKind,
    pub status: StopOrderStatus,
    pub created_at: u64,
    pub triggered_at: u64,
    pub executed_at: u64,
}

/// Storage sub-keys for stop loss data.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StopLossKey {
    /// Order record keyed by order_id.
    Order(u64),
    /// Global order ID counter.
    Counter,
    /// List of order IDs owned by an address.
    OwnerOrders(Address),
    /// List of all active order IDs (for price monitoring).
    ActiveOrders,
}

// ── Price precision ──────────────────────────────────────────────────────────

/// All prices are stored as integer multiples of PRICE_PRECISION.
pub const PRICE_PRECISION: i128 = 1_000_000;

// ── Errors ───────────────────────────────────────────────────────────────────

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StopLossError {
    OrderNotFound = 700,
    OrderNotActive = 701,
    Unauthorized = 702,
    InvalidAmount = 703,
    InvalidStopPrice = 704,
    InvalidLimitPrice = 705,
    InvalidTrailAmount = 706,
    PriceNotTriggered = 707,
    LimitPriceNotMet = 708,
    TokenNotWhitelisted = 709,
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Places a new stop loss order. Tokens are transferred into escrow.
///
/// Returns the new order ID.
/// Emits `("sl_place",)` with data `(order_id, owner, token, amount, stop_price, kind)`.
pub fn place_order(
    env: &Env,
    owner: &Address,
    token: &Address,
    amount: i128,
    stop_price: i128,
    limit_price: i128,
    trail_amount: i128,
    kind: StopOrderKind,
    current_price: i128,
) -> u64 {
    owner.require_auth();

    if amount <= 0 {
        panic_with_error!(env, StopLossError::InvalidAmount);
    }
    if stop_price <= 0 {
        panic_with_error!(env, StopLossError::InvalidStopPrice);
    }
    if matches!(kind, StopOrderKind::StopLimit) && limit_price <= 0 {
        panic_with_error!(env, StopLossError::InvalidLimitPrice);
    }
    if matches!(kind, StopOrderKind::TrailingStop) && trail_amount <= 0 {
        panic_with_error!(env, StopLossError::InvalidTrailAmount);
    }

    let order_id: u64 = env
        .storage()
        .instance()
        .get(&crate::DataKey::StopLoss(StopLossKey::Counter))
        .unwrap_or(0u64);
    env.storage()
        .instance()
        .set(&crate::DataKey::StopLoss(StopLossKey::Counter), &(order_id + 1));

    let now = env.ledger().timestamp();
    let peak = if current_price > 0 { current_price } else { stop_price };

    let order = StopOrder {
        order_id,
        owner: owner.clone(),
        token: token.clone(),
        amount,
        stop_price,
        limit_price,
        trail_amount,
        peak_price: peak,
        kind,
        status: StopOrderStatus::Active,
        created_at: now,
        triggered_at: 0,
        executed_at: 0,
    };

    env.storage()
        .persistent()
        .set(&crate::DataKey::StopLoss(StopLossKey::Order(order_id)), &order);

    _add_owner_order(env, owner, order_id);
    _add_active_order(env, order_id);

    // Transfer tokens into escrow
    token::Client::new(env, token).transfer(owner, &env.current_contract_address(), &amount);

    env.events().publish(
        (symbol_short!("sl_place"),),
        (order_id, owner.clone(), token.clone(), amount, stop_price, kind),
    );

    order_id
}

/// Updates the peak price for a trailing stop order and adjusts stop_price accordingly.
///
/// Should be called whenever a new market price is observed.
/// Emits `("sl_trail",)` with data `(order_id, new_stop_price)`.
pub fn update_price(env: &Env, order_id: u64, current_price: i128) {
    let mut order: StopOrder = _get_order_or_panic(env, order_id);

    if order.status != StopOrderStatus::Active {
        return;
    }

    if !matches!(order.kind, StopOrderKind::TrailingStop) {
        return;
    }

    if current_price > order.peak_price {
        order.peak_price = current_price;
        // Raise the stop price to trail the new peak
        let new_stop = current_price.saturating_sub(order.trail_amount);
        if new_stop > order.stop_price {
            order.stop_price = new_stop;
            env.storage().persistent().set(
                &crate::DataKey::StopLoss(StopLossKey::Order(order_id)),
                &order,
            );
            env.events()
                .publish((symbol_short!("sl_trail"),), (order_id, new_stop));
        }
    }
}

/// Checks whether `current_price` triggers the order and marks it as Triggered.
///
/// Returns `true` if the order was triggered.
/// Emits `("sl_trig",)` with data `(order_id, current_price)`.
pub fn check_trigger(env: &Env, order_id: u64, current_price: i128) -> bool {
    let mut order: StopOrder = _get_order_or_panic(env, order_id);

    if order.status != StopOrderStatus::Active {
        return false;
    }

    // Update trailing stop peak before checking trigger
    if matches!(order.kind, StopOrderKind::TrailingStop) && current_price > order.peak_price {
        order.peak_price = current_price;
        let new_stop = current_price.saturating_sub(order.trail_amount);
        if new_stop > order.stop_price {
            order.stop_price = new_stop;
        }
    }

    if current_price <= order.stop_price {
        order.status = StopOrderStatus::Triggered;
        order.triggered_at = env.ledger().timestamp();
        env.storage().persistent().set(
            &crate::DataKey::StopLoss(StopLossKey::Order(order_id)),
            &order,
        );
        env.events()
            .publish((symbol_short!("sl_trig"),), (order_id, current_price));
        return true;
    }

    false
}

/// Executes a triggered stop loss order, transferring escrowed tokens to `recipient`.
///
/// For StopLimit orders, `execution_price` must be ≥ `limit_price`.
/// Emits `("sl_exec",)` with data `(order_id, amount, execution_price)`.
pub fn execute_order(env: &Env, caller: &Address, order_id: u64, execution_price: i128) {
    caller.require_auth();

    let mut order: StopOrder = _get_order_or_panic(env, order_id);

    if order.status != StopOrderStatus::Triggered {
        panic_with_error!(env, StopLossError::OrderNotActive);
    }

    // For stop-limit, verify execution price meets the limit
    if matches!(order.kind, StopOrderKind::StopLimit) {
        if execution_price < order.limit_price {
            panic_with_error!(env, StopLossError::LimitPriceNotMet);
        }
    }

    order.status = StopOrderStatus::Executed;
    order.executed_at = env.ledger().timestamp();
    env.storage()
        .persistent()
        .set(&crate::DataKey::StopLoss(StopLossKey::Order(order_id)), &order);

    _remove_active_order(env, order_id);

    // Transfer escrowed tokens back to owner (position closed)
    token::Client::new(env, &order.token).transfer(
        &env.current_contract_address(),
        &order.owner,
        &order.amount,
    );

    env.events().publish(
        (symbol_short!("sl_exec"),),
        (order_id, order.amount, execution_price),
    );
}

/// Cancels an active stop loss order and refunds escrowed tokens to the owner.
///
/// Only the order owner may cancel.
/// Emits `("sl_cncl",)` with data `(order_id,)`.
pub fn cancel_order(env: &Env, owner: &Address, order_id: u64) {
    owner.require_auth();

    let mut order: StopOrder = _get_order_or_panic(env, order_id);

    if order.owner != *owner {
        panic_with_error!(env, StopLossError::Unauthorized);
    }
    if order.status != StopOrderStatus::Active && order.status != StopOrderStatus::Triggered {
        panic_with_error!(env, StopLossError::OrderNotActive);
    }

    order.status = StopOrderStatus::Cancelled;
    env.storage()
        .persistent()
        .set(&crate::DataKey::StopLoss(StopLossKey::Order(order_id)), &order);

    _remove_active_order(env, order_id);

    // Refund escrowed tokens
    token::Client::new(env, &order.token).transfer(
        &env.current_contract_address(),
        &order.owner,
        &order.amount,
    );

    env.events()
        .publish((symbol_short!("sl_cncl"),), (order_id,));
}

/// Returns a stop order by ID.
pub fn get_order(env: &Env, order_id: u64) -> Option<StopOrder> {
    env.storage()
        .persistent()
        .get(&crate::DataKey::StopLoss(StopLossKey::Order(order_id)))
}

/// Returns all order IDs owned by `owner`.
pub fn get_owner_orders(env: &Env, owner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&crate::DataKey::StopLoss(StopLossKey::OwnerOrders(owner.clone())))
        .unwrap_or_else(|| Vec::new(env))
}

/// Returns all currently active order IDs.
pub fn get_active_orders(env: &Env) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&crate::DataKey::StopLoss(StopLossKey::ActiveOrders))
        .unwrap_or_else(|| Vec::new(env))
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn _get_order_or_panic(env: &Env, order_id: u64) -> StopOrder {
    env.storage()
        .persistent()
        .get(&crate::DataKey::StopLoss(StopLossKey::Order(order_id)))
        .unwrap_or_else(|| panic_with_error!(env, StopLossError::OrderNotFound))
}

fn _add_owner_order(env: &Env, owner: &Address, order_id: u64) {
    let mut orders: Vec<u64> = env
        .storage()
        .persistent()
        .get(&crate::DataKey::StopLoss(StopLossKey::OwnerOrders(owner.clone())))
        .unwrap_or_else(|| Vec::new(env));
    if !orders.contains(&order_id) {
        orders.push_back(order_id);
        env.storage().persistent().set(
            &crate::DataKey::StopLoss(StopLossKey::OwnerOrders(owner.clone())),
            &orders,
        );
    }
}

fn _add_active_order(env: &Env, order_id: u64) {
    let mut orders: Vec<u64> = env
        .storage()
        .persistent()
        .get(&crate::DataKey::StopLoss(StopLossKey::ActiveOrders))
        .unwrap_or_else(|| Vec::new(env));
    if !orders.contains(&order_id) {
        orders.push_back(order_id);
        env.storage()
            .persistent()
            .set(&crate::DataKey::StopLoss(StopLossKey::ActiveOrders), &orders);
    }
}

fn _remove_active_order(env: &Env, order_id: u64) {
    let orders: Vec<u64> = env
        .storage()
        .persistent()
        .get(&crate::DataKey::StopLoss(StopLossKey::ActiveOrders))
        .unwrap_or_else(|| Vec::new(env));
    let mut remaining = Vec::new(env);
    for id in orders.iter() {
        if id != order_id {
            remaining.push_back(id);
        }
    }
    env.storage()
        .persistent()
        .set(&crate::DataKey::StopLoss(StopLossKey::ActiveOrders), &remaining);
}
