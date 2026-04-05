use crate::data::candle::Candle;

/// Timeframe multiplier (in 1-minute bars).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timeframe {
    M5  = 5,
    M15 = 15,
}

/// Aggregates 1-minute candles into a higher timeframe candle.
/// Internally accumulates bars until a full period is complete.
pub struct Aggregator {
    tf:           Timeframe,
    period:       usize,
    buf:          [Option<Candle>; 15],  // max period = 15
    count:        usize,
}

impl Aggregator {
    pub fn new(tf: Timeframe) -> Self {
        let period = tf as usize;
        assert!(period <= 15, "period exceeds internal buffer");
        Self { tf, period, buf: [None; 15], count: 0 }
    }

    /// Feed a 1-minute candle. Returns Some(aggregated) when a full period completes.
    pub fn push(&mut self, candle: Candle) -> Option<Candle> {
        self.buf[self.count] = Some(candle);
        self.count += 1;

        if self.count == self.period {
            let result = self.merge();
            self.count = 0;
            self.buf = [None; 15];
            Some(result)
        } else {
            None
        }
    }

    fn merge(&self) -> Candle {
        let first = self.buf[0].unwrap();
        let mut high  = first.high;
        let mut low   = first.low;
        let mut close = first.close;

        for i in 1..self.period {
            if let Some(c) = self.buf[i] {
                if c.high  > high  { high  = c.high; }
                if c.low   < low   { low   = c.low;  }
                close = c.close;
            }
        }

        Candle::new(first.open, high, low, close, first.timestamp)
    }

    pub fn timeframe(&self) -> Timeframe { self.tf }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn m1(open: f64, high: f64, low: f64, close: f64, ts: u64) -> Candle {
        Candle::new(open, high, low, close, ts)
    }

    // ── M5 ────────────────────────────────────────────────────────────────────

    #[test]
    fn aggregator_m5_returns_none_before_5_bars() {
        let mut agg = Aggregator::new(Timeframe::M5);
        for i in 0..4 {
            let result = agg.push(m1(100.0, 101.0, 99.0, 100.5, i as u64 * 60));
            assert!(result.is_none(), "expected None on bar {i}");
        }
    }

    #[test]
    fn aggregator_m5_returns_candle_on_5th_bar() {
        let mut agg = Aggregator::new(Timeframe::M5);
        let bars = [
            m1(100.0, 102.0, 99.0, 101.0, 0),
            m1(101.0, 103.0, 100.0, 102.0, 60),
            m1(102.0, 104.0, 101.0, 103.0, 120),
            m1(103.0, 105.0, 102.0, 104.0, 180),
            m1(104.0, 106.0, 103.0, 105.0, 240),
        ];
        let mut result = None;
        for b in bars { result = agg.push(b); }

        let c = result.expect("expected Some on 5th bar");
        assert_eq!(c.open,      100.0); // open of first bar
        assert_eq!(c.high,      106.0); // max high
        assert_eq!(c.low,       99.0);  // min low
        assert_eq!(c.close,     105.0); // close of last bar
        assert_eq!(c.timestamp, 0);     // timestamp of first bar
    }

    #[test]
    fn aggregator_m5_resets_after_period() {
        let mut agg = Aggregator::new(Timeframe::M5);
        let bar = m1(100.0, 101.0, 99.0, 100.0, 0);
        // Fill one complete period
        for _ in 0..5 { agg.push(bar); }
        // Next bar should start fresh → None
        assert!(agg.push(bar).is_none());
    }

    // ── M15 ───────────────────────────────────────────────────────────────────

    #[test]
    fn aggregator_m15_returns_candle_on_15th_bar() {
        let mut agg = Aggregator::new(Timeframe::M15);
        let mut result = None;
        for i in 0..15 {
            let p = 200.0 + i as f64;
            result = agg.push(m1(p, p + 1.0, p - 1.0, p + 0.5, i as u64 * 60));
        }
        assert!(result.is_some());
        let c = result.unwrap();
        assert_eq!(c.open,  200.0);        // first bar open
        assert_eq!(c.close, 214.5);        // last bar close
        assert!((c.high - 215.0).abs() < 1e-9);
        assert!((c.low  - 199.0).abs() < 1e-9);
    }
}
