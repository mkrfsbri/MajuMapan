/// OHLCV candle representing one time period.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candle {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl Candle {
    pub fn new(open: f64, high: f64, low: f64, close: f64, volume: f64) -> Self {
        Self { open, high, low, close, volume }
    }

    /// Body size (absolute).
    #[inline]
    pub fn body(&self) -> f64 {
        (self.close - self.open).abs()
    }

    /// Upper wick size.
    #[inline]
    pub fn upper_wick(&self) -> f64 {
        self.high - self.open.max(self.close)
    }

    /// Lower wick size.
    #[inline]
    pub fn lower_wick(&self) -> f64 {
        self.open.min(self.close) - self.low
    }
}

/// Trading signal produced by the signal engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Up,
    Down,
    None,
}

// ─────────────────────────────────────────────────────────────────────────────
// TDD — Step 1 Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candle_stores_values_correctly() {
        let c = Candle::new(100.0, 110.0, 90.0, 105.0, 1_000.0);
        assert_eq!(c.open,   100.0);
        assert_eq!(c.high,   110.0);
        assert_eq!(c.low,    90.0);
        assert_eq!(c.close,  105.0);
        assert_eq!(c.volume, 1_000.0);
    }

    #[test]
    fn candle_body_is_absolute_difference() {
        let c = Candle::new(105.0, 110.0, 90.0, 100.0, 0.0);
        assert_eq!(c.body(), 5.0);
    }

    #[test]
    fn candle_upper_wick_bullish() {
        // open=100 close=105 high=115  → wick = 115 - 105 = 10
        let c = Candle::new(100.0, 115.0, 98.0, 105.0, 0.0);
        assert_eq!(c.upper_wick(), 10.0);
    }

    #[test]
    fn candle_lower_wick_bearish() {
        // open=105 close=100 low=90  → wick = 100 - 90 = 10
        let c = Candle::new(105.0, 108.0, 90.0, 100.0, 0.0);
        assert_eq!(c.lower_wick(), 10.0);
    }

    #[test]
    fn signal_variants_are_usable() {
        let up   = Signal::Up;
        let down = Signal::Down;
        let none = Signal::None;
        assert_ne!(up, down);
        assert_ne!(up, none);
        assert_ne!(down, none);
    }
}
