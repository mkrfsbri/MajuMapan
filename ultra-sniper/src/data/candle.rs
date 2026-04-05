/// OHLC candle with Unix-second timestamp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candle {
    pub open:      f64,
    pub high:      f64,
    pub low:       f64,
    pub close:     f64,
    pub timestamp: u64,  // unix seconds
}

impl Candle {
    pub fn new(open: f64, high: f64, low: f64, close: f64, timestamp: u64) -> Self {
        Self { open, high, low, close, timestamp }
    }

    #[inline] pub fn body(&self)       -> f64 { (self.close - self.open).abs() }
    #[inline] pub fn range(&self)      -> f64 { self.high - self.low }
    #[inline] pub fn upper_wick(&self) -> f64 { self.high - self.open.max(self.close) }
    #[inline] pub fn lower_wick(&self) -> f64 { self.open.min(self.close) - self.low }

    /// True when OHLC values form a valid candle.
    pub fn is_valid(&self) -> bool {
        self.high >= self.open
            && self.high >= self.close
            && self.low  <= self.open
            && self.low  <= self.close
            && self.high >= self.low
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candle_stores_fields() {
        let c = Candle::new(100.0, 110.0, 90.0, 105.0, 1_000);
        assert_eq!(c.open,      100.0);
        assert_eq!(c.high,      110.0);
        assert_eq!(c.low,       90.0);
        assert_eq!(c.close,     105.0);
        assert_eq!(c.timestamp, 1_000);
    }

    #[test]
    fn candle_valid() {
        let c = Candle::new(100.0, 110.0, 90.0, 105.0, 0);
        assert!(c.is_valid());
    }

    #[test]
    fn candle_invalid_high_below_open() {
        let c = Candle::new(100.0, 90.0, 80.0, 85.0, 0);
        assert!(!c.is_valid());
    }

    #[test]
    fn candle_body_absolute() {
        let c = Candle::new(105.0, 110.0, 90.0, 100.0, 0);
        assert_eq!(c.body(), 5.0);
    }

    #[test]
    fn candle_upper_wick() {
        // open=100, close=105, high=115 → wick = 115-105 = 10
        let c = Candle::new(100.0, 115.0, 98.0, 105.0, 0);
        assert_eq!(c.upper_wick(), 10.0);
    }

    #[test]
    fn candle_lower_wick() {
        // open=105, close=100, low=90 → wick = 100-90 = 10
        let c = Candle::new(105.0, 108.0, 90.0, 100.0, 0);
        assert_eq!(c.lower_wick(), 10.0);
    }

    #[test]
    fn candle_range() {
        let c = Candle::new(100.0, 110.0, 90.0, 105.0, 0);
        assert_eq!(c.range(), 20.0);
    }
}
