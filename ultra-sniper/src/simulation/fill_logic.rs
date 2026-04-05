//! Phase 16 — Fill logic: full fill, partial fill, no-liquidity.
//!
//! Models realistic Polymarket fills:
//! - Full fill   : ask is within budget → buy requested size.
//! - Partial fill: ask is within budget but spread is wide → slight price penalty.
//! - No fill     : ask > 1.0 or budget insufficient.

use crate::simulation::orderbook::OrderBook;
use crate::execution::Side;

/// Slippage penalty applied to wide-spread books (> 4 cents spread).
const WIDE_SPREAD_SLIPPAGE: f64 = 0.005; // 0.5 cents

/// Result of attempting to fill an order.
#[derive(Debug, Clone, Copy)]
pub struct FillResult {
    /// Actual fill price (ask ± slippage).
    pub fill_price: f64,
    /// Number of contracts purchased.
    pub contracts:  f64,
    /// Total cost deducted from balance.
    pub cost:       f64,
    /// True when at least some contracts were purchased.
    pub filled:     bool,
    /// True when only a partial amount was purchased.
    pub partial:    bool,
}

impl FillResult {
    pub fn is_filled(&self) -> bool { self.filled }

    /// No-fill sentinel.
    pub fn no_fill() -> Self {
        Self { fill_price: 0.0, contracts: 0.0, cost: 0.0, filled: false, partial: false }
    }
}

/// Attempt to fill a market buy order.
///
/// # Arguments
/// * `ask`       — best ask on the relevant side.
/// * `budget`    — USD available for this order.
/// * `book`      — full orderbook (used to check spread).
/// * `side`      — YES or NO side.
///
/// # Returns
/// [`FillResult`] describing the outcome.
pub fn fill_order(ask: f64, budget: f64, book: &OrderBook, side: Side) -> FillResult {
    // Guard: ask must be valid and within normal Polymarket range.
    if ask <= 0.0 || ask > 1.0 || budget <= 0.0 {
        return FillResult::no_fill();
    }

    let spread = match side {
        Side::Yes => book.spread_yes(),
        Side::No  => book.spread_no(),
    };

    // Apply slippage when spread is wide (> 4 cents).
    let effective_ask = if spread > 0.04 {
        (ask + WIDE_SPREAD_SLIPPAGE).min(1.0)
    } else {
        ask
    };

    if effective_ask > 1.0 {
        return FillResult::no_fill();
    }

    let max_contracts = budget / effective_ask;

    if max_contracts < 0.01 {
        // Not enough budget to buy even a cent's worth.
        return FillResult::no_fill();
    }

    // Full fill: budget covers the full order.
    let contracts  = max_contracts;
    let cost       = contracts * effective_ask;
    let partial    = cost < budget - 1e-9; // leftover > $1 means partial intent

    FillResult {
        fill_price: effective_ask,
        contracts,
        cost,
        filled: true,
        partial,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::orderbook::OrderBook;

    fn tight_book() -> OrderBook { OrderBook::new(0.60, 0.62, 0.38, 0.40) }
    fn wide_book()  -> OrderBook { OrderBook::new(0.55, 0.65, 0.35, 0.45) }

    #[test]
    fn full_fill_tight_spread() {
        let fill = fill_order(0.62, 100.0, &tight_book(), Side::Yes);
        assert!(fill.is_filled());
        assert!(!fill.partial);
        assert!((fill.contracts - 100.0 / 0.62).abs() < 1e-6);
    }

    #[test]
    fn fill_price_equals_ask_tight() {
        let fill = fill_order(0.62, 100.0, &tight_book(), Side::Yes);
        assert!((fill.fill_price - 0.62).abs() < 1e-9);
    }

    #[test]
    fn wide_spread_adds_slippage() {
        let fill = fill_order(0.65, 100.0, &wide_book(), Side::Yes);
        assert!(fill.is_filled());
        // fill_price should be ask + slippage
        assert!((fill.fill_price - (0.65 + WIDE_SPREAD_SLIPPAGE)).abs() < 1e-9);
    }

    #[test]
    fn zero_ask_returns_no_fill() {
        let fill = fill_order(0.0, 100.0, &tight_book(), Side::Yes);
        assert!(!fill.is_filled());
    }

    #[test]
    fn ask_above_one_returns_no_fill() {
        let fill = fill_order(1.01, 100.0, &tight_book(), Side::Yes);
        assert!(!fill.is_filled());
    }

    #[test]
    fn zero_budget_returns_no_fill() {
        let fill = fill_order(0.62, 0.0, &tight_book(), Side::Yes);
        assert!(!fill.is_filled());
    }

    #[test]
    fn negative_budget_returns_no_fill() {
        let fill = fill_order(0.62, -50.0, &tight_book(), Side::Yes);
        assert!(!fill.is_filled());
    }

    #[test]
    fn cost_matches_contracts_times_price() {
        let fill = fill_order(0.62, 100.0, &tight_book(), Side::Yes);
        assert!((fill.cost - fill.contracts * fill.fill_price).abs() < 1e-9);
    }

    #[test]
    fn no_side_fill() {
        let fill = fill_order(0.40, 100.0, &tight_book(), Side::No);
        assert!(fill.is_filled());
        assert!((fill.fill_price - 0.40).abs() < 1e-9);
    }

    #[test]
    fn tiny_budget_no_fill() {
        let fill = fill_order(0.62, 0.001, &tight_book(), Side::Yes);
        assert!(!fill.is_filled());
    }
}
