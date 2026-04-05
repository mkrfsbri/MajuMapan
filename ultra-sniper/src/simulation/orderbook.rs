//! Phase 14 — Polymarket orderbook snapshot.

/// Best bid/ask for YES and NO tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrderBook {
    /// Highest price someone is willing to pay for YES
    pub best_bid_yes: f64,
    /// Lowest price someone is willing to sell YES
    pub best_ask_yes: f64,
    /// Highest price someone is willing to pay for NO
    pub best_bid_no:  f64,
    /// Lowest price someone is willing to sell NO
    pub best_ask_no:  f64,
}

impl OrderBook {
    pub fn new(bid_yes: f64, ask_yes: f64, bid_no: f64, ask_no: f64) -> Self {
        Self { best_bid_yes: bid_yes, best_ask_yes: ask_yes,
               best_bid_no: bid_no,  best_ask_no: ask_no }
    }

    /// Neutral starting book at 50 cents each side.
    pub fn neutral() -> Self {
        Self::new(0.49, 0.51, 0.49, 0.51)
    }

    /// Mid-price of YES token.
    pub fn mid_yes(&self) -> f64 { (self.best_bid_yes + self.best_ask_yes) / 2.0 }

    /// Mid-price of NO token.
    pub fn mid_no(&self)  -> f64 { (self.best_bid_no  + self.best_ask_no)  / 2.0 }

    /// YES bid-ask spread.
    pub fn spread_yes(&self) -> f64 { (self.best_ask_yes - self.best_bid_yes).max(0.0) }

    /// NO bid-ask spread.
    pub fn spread_no(&self)  -> f64 { (self.best_ask_no  - self.best_bid_no).max(0.0) }

    /// True when the book contains valid, non-negative prices.
    pub fn is_valid(&self) -> bool {
        let ok = |b: f64, a: f64| b >= 0.0 && a >= 0.0 && a >= b && a <= 1.0;
        ok(self.best_bid_yes, self.best_ask_yes) && ok(self.best_bid_no, self.best_ask_no)
    }

    /// Update YES side; NO side recalculated as complement.
    pub fn update_yes(&mut self, bid: f64, ask: f64) {
        self.best_bid_yes = bid;
        self.best_ask_yes = ask;
        self.best_bid_no  = (1.0 - ask).max(0.0);
        self.best_ask_no  = (1.0 - bid).min(1.0);
    }

    /// Update NO side; YES side recalculated as complement.
    pub fn update_no(&mut self, bid: f64, ask: f64) {
        self.best_bid_no  = bid;
        self.best_ask_no  = ask;
        self.best_bid_yes = (1.0 - ask).max(0.0);
        self.best_ask_yes = (1.0 - bid).min(1.0);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_book_is_valid() {
        assert!(OrderBook::neutral().is_valid());
    }

    #[test]
    fn mid_yes_is_average() {
        let ob = OrderBook::new(0.60, 0.62, 0.38, 0.40);
        assert!((ob.mid_yes() - 0.61).abs() < 1e-9);
    }

    #[test]
    fn mid_no_is_average() {
        let ob = OrderBook::new(0.60, 0.62, 0.38, 0.40);
        assert!((ob.mid_no() - 0.39).abs() < 1e-9);
    }

    #[test]
    fn spread_yes_correct() {
        let ob = OrderBook::new(0.60, 0.62, 0.38, 0.40);
        assert!((ob.spread_yes() - 0.02).abs() < 1e-9);
    }

    #[test]
    fn spread_no_correct() {
        let ob = OrderBook::new(0.60, 0.62, 0.38, 0.40);
        assert!((ob.spread_no() - 0.02).abs() < 1e-9);
    }

    #[test]
    fn update_yes_side() {
        let mut ob = OrderBook::neutral();
        ob.update_yes(0.65, 0.67);
        assert!((ob.best_bid_yes - 0.65).abs() < 1e-9);
        assert!((ob.best_ask_yes - 0.67).abs() < 1e-9);
    }

    #[test]
    fn update_yes_recalculates_no_complement() {
        let mut ob = OrderBook::neutral();
        ob.update_yes(0.60, 0.62);
        // NO bid = 1 - YES ask = 0.38
        assert!((ob.best_bid_no - 0.38).abs() < 1e-9);
        // NO ask = 1 - YES bid = 0.40
        assert!((ob.best_ask_no - 0.40).abs() < 1e-9);
    }

    #[test]
    fn update_no_recalculates_yes_complement() {
        let mut ob = OrderBook::neutral();
        ob.update_no(0.38, 0.40);
        assert!((ob.best_bid_yes - 0.60).abs() < 1e-9);
        assert!((ob.best_ask_yes - 0.62).abs() < 1e-9);
    }

    #[test]
    fn invalid_book_detected() {
        let ob = OrderBook::new(0.70, 0.60, 0.30, 0.40); // ask < bid on YES
        assert!(!ob.is_valid());
    }

    #[test]
    fn best_prices_stored_correctly() {
        let ob = OrderBook::new(0.58, 0.62, 0.37, 0.41);
        assert!((ob.best_bid_yes - 0.58).abs() < 1e-9);
        assert!((ob.best_ask_yes - 0.62).abs() < 1e-9);
        assert!((ob.best_bid_no  - 0.37).abs() < 1e-9);
        assert!((ob.best_ask_no  - 0.41).abs() < 1e-9);
    }
}
