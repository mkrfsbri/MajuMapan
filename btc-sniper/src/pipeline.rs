use crate::types::{Candle, Signal};
use crate::ring_buffer::RingBuffer;
use crate::indicators::{Ema, Rsi};
use crate::signal_engine::evaluate;

/// Output of the pipeline for a single candle.
#[derive(Debug, Clone, Copy)]
pub struct PipelineOutput {
    pub ema_fast: f64,
    pub ema_slow: f64,
    pub rsi:      Option<f64>,
    pub signal:   Signal,
}

/// End-to-end pipeline:
///   Candle feed → Indicator update → Signal evaluation.
///
/// Generic parameter HIST: history window kept in the ring buffer.
pub struct Pipeline<const HIST: usize = 50> {
    history:  RingBuffer<Candle, HIST>,
    ema_fast: Ema,
    ema_slow: Ema,
    rsi:      Rsi,
}

impl<const HIST: usize> Pipeline<HIST> {
    /// Create pipeline with given EMA periods and RSI period.
    pub fn new(fast_period: usize, slow_period: usize, rsi_period: usize) -> Self {
        Self {
            history:  RingBuffer::new(),
            ema_fast: Ema::new(fast_period),
            ema_slow: Ema::new(slow_period),
            rsi:      Rsi::new(rsi_period),
        }
    }

    /// Feed one candle through the full pipeline.
    pub fn feed(&mut self, candle: Candle) -> PipelineOutput {
        // 1. Update indicators
        let ema_fast = self.ema_fast.update(candle.close);
        let ema_slow = self.ema_slow.update(candle.close);
        let rsi      = self.rsi.update(candle.close);

        // 2. Evaluate signal (requires at least one previous candle)
        let signal = if let Some(prev) = self.history.latest() {
            evaluate(&prev, &candle)
        } else {
            Signal::None
        };

        // 3. Store candle in history
        self.history.push(candle);

        PipelineOutput { ema_fast, ema_slow, rsi, signal }
    }

    pub fn history(&self) -> &RingBuffer<Candle, HIST> { &self.history }
}

// ─────────────────────────────────────────────────────────────────────────────
// TDD — Step 6 Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn make_candle(price: f64) -> Candle {
        Candle::new(price, price + 1.0, price - 1.0, price, 100.0)
    }

    #[test]
    fn pipeline_does_not_panic_on_single_candle() {
        let mut p: Pipeline<10> = Pipeline::new(3, 9, 14);
        let out = p.feed(make_candle(100.0));
        assert_eq!(out.signal, Signal::None); // no prev candle yet
        assert!((out.ema_fast - 100.0).abs() < 1e-9);
    }

    #[test]
    fn pipeline_stores_history() {
        let mut p: Pipeline<10> = Pipeline::new(3, 9, 14);
        for i in 0..5 {
            p.feed(make_candle(100.0 + i as f64));
        }
        assert_eq!(p.history().len(), 5);
    }

    #[test]
    fn pipeline_ema_fast_slower_than_slow_converges() {
        let mut p: Pipeline<50> = Pipeline::new(3, 9, 14);
        // Rising prices → fast EMA should be above slow EMA after warmup
        for i in 0..30 {
            p.feed(make_candle(100.0 + i as f64));
        }
        let out = p.feed(make_candle(130.0));
        assert!(out.ema_fast > out.ema_slow,
            "fast={} slow={}", out.ema_fast, out.ema_slow);
    }

    #[test]
    fn pipeline_rsi_returns_some_after_warmup() {
        let mut p: Pipeline<50> = Pipeline::new(3, 9, 14);
        let prices = [
            44.34, 44.09, 44.15, 43.61, 44.33,
            44.83, 45.10, 45.15, 43.61, 44.33,
            44.83, 45.10, 45.15, 43.61, 44.55,
        ];
        let mut last_out = None;
        for &price in &prices {
            let c = Candle::new(price, price+0.5, price-0.5, price, 0.0);
            last_out = Some(p.feed(c));
        }
        assert!(last_out.unwrap().rsi.is_some());
    }

    #[test]
    fn pipeline_end_to_end_no_crash_100_candles() {
        let mut p: Pipeline<50> = Pipeline::new(9, 21, 14);
        for i in 0..100 {
            let price = 30_000.0 + (i as f64) * 10.0;
            p.feed(Candle::new(price, price+50.0, price-50.0, price+20.0, 1.0));
        }
        // Just verify last len is capped at HIST
        assert_eq!(p.history().len(), 50);
    }
}
