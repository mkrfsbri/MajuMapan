use crate::types::Candle;

// ─────────────────────────────────────────────────────────────────────────────
// Fake Breakout detector
// ─────────────────────────────────────────────────────────────────────────────

/// Returns true when `current` shows a fake breakout above `prev`:
///   - current.high > prev.high   (price breached prior high)
///   - current.close < prev.high  (closed back below — trap)
///   - upper_wick > body (significant rejection wick)
pub fn is_fake_breakout_above(prev: &Candle, current: &Candle) -> bool {
    current.high > prev.high
        && current.close < prev.high
        && current.upper_wick() > current.body()
}

/// Returns true when `current` shows a fake breakout below `prev` (mirror).
pub fn is_fake_breakout_below(prev: &Candle, current: &Candle) -> bool {
    current.low < prev.low
        && current.close > prev.low
        && current.lower_wick() > current.body()
}

// ─────────────────────────────────────────────────────────────────────────────
// Rejection candle detector
// ─────────────────────────────────────────────────────────────────────────────

/// Bearish rejection: upper wick > body * factor (default 1.5).
pub fn is_bearish_rejection(candle: &Candle, factor: f64) -> bool {
    let body = candle.body();
    candle.upper_wick() > body * factor
}

/// Bullish rejection: lower wick > body * factor.
pub fn is_bullish_rejection(candle: &Candle, factor: f64) -> bool {
    let body = candle.body();
    candle.lower_wick() > body * factor
}

// ─────────────────────────────────────────────────────────────────────────────
// Break of Structure (BOS)
// ─────────────────────────────────────────────────────────────────────────────

/// BOS Down: lower high AND close breaks previous low.
pub fn is_bos_down(prev: &Candle, current: &Candle) -> bool {
    current.high < prev.high && current.close < prev.low
}

/// BOS Up: higher low AND close breaks previous high.
pub fn is_bos_up(prev: &Candle, current: &Candle) -> bool {
    current.low > prev.low && current.close > prev.high
}

// ─────────────────────────────────────────────────────────────────────────────
// TDD — Step 4 Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn candle(open: f64, high: f64, low: f64, close: f64) -> Candle {
        Candle::new(open, high, low, close, 0.0)
    }

    // ── Fake Breakout ─────────────────────────────────────────────────────────

    #[test]
    fn fake_breakout_above_valid() {
        // prev high = 100
        // current: high=105 (breaches), close=98 (back below), big upper wick
        let prev    = candle(95.0, 100.0, 92.0, 97.0);
        let current = candle(97.0, 105.0, 96.0, 98.0);
        // upper_wick = 105 - max(97,98) = 7, body = 1 → wick > body ✓
        assert!(is_fake_breakout_above(&prev, &current));
    }

    #[test]
    fn fake_breakout_above_invalid_close_above_prev_high() {
        let prev    = candle(95.0, 100.0, 92.0, 97.0);
        // close = 102 > prev.high=100 → not a fake breakout
        let current = candle(97.0, 105.0, 96.0, 102.0);
        assert!(!is_fake_breakout_above(&prev, &current));
    }

    #[test]
    fn fake_breakout_above_invalid_no_wick() {
        let prev    = candle(95.0, 100.0, 92.0, 97.0);
        // close = 99 (below prev.high), but wick = 105-99 = 6 and body = |97-99| = 2 → wick > body ✓
        // Make body bigger than wick to force failure
        // open=96 close=99 high=100.5 → wick=1.5, body=3 → wick < body
        let current = candle(96.0, 100.5, 95.0, 99.0);
        assert!(!is_fake_breakout_above(&prev, &current));
    }

    // ── Rejection ─────────────────────────────────────────────────────────────

    #[test]
    fn bearish_rejection_valid() {
        // open=100 close=101 high=110 → body=1, upper_wick=9 → 9 > 1.5*1 ✓
        let c = candle(100.0, 110.0, 99.0, 101.0);
        assert!(is_bearish_rejection(&c, 1.5));
    }

    #[test]
    fn bearish_rejection_invalid() {
        // open=100 close=108 high=110 → body=8, upper_wick=2 → 2 < 1.5*8
        let c = candle(100.0, 110.0, 99.0, 108.0);
        assert!(!is_bearish_rejection(&c, 1.5));
    }

    #[test]
    fn bullish_rejection_valid() {
        // open=100 close=99 low=90 → body=1, lower_wick=9 → 9 > 1.5 ✓
        let c = candle(100.0, 101.0, 90.0, 99.0);
        assert!(is_bullish_rejection(&c, 1.5));
    }

    #[test]
    fn bullish_rejection_invalid() {
        // open=100 close=92 low=90 → body=8, lower_wick=2 → 2 < 12
        let c = candle(100.0, 101.0, 90.0, 92.0);
        assert!(!is_bullish_rejection(&c, 1.5));
    }

    // ── BOS ───────────────────────────────────────────────────────────────────

    #[test]
    fn bos_down_valid() {
        // prev: high=105, low=95
        // current: high=103 (lower), close=93 (< prev.low=95)
        let prev    = candle(98.0, 105.0, 95.0, 97.0);
        let current = candle(97.0, 103.0, 92.0, 93.0);
        assert!(is_bos_down(&prev, &current));
    }

    #[test]
    fn bos_down_invalid_close_above_prev_low() {
        let prev    = candle(98.0, 105.0, 95.0, 97.0);
        let current = candle(97.0, 103.0, 94.0, 96.0); // close 96 > prev.low 95
        assert!(!is_bos_down(&prev, &current));
    }

    #[test]
    fn bos_up_valid() {
        // prev: high=105, low=95
        // current: low=97 (higher), close=107 (> prev.high=105)
        let prev    = candle(98.0, 105.0, 95.0, 102.0);
        let current = candle(100.0, 108.0, 97.0, 107.0);
        assert!(is_bos_up(&prev, &current));
    }

    #[test]
    fn bos_up_invalid_close_below_prev_high() {
        let prev    = candle(98.0, 105.0, 95.0, 102.0);
        let current = candle(100.0, 108.0, 97.0, 103.0); // close 103 < prev.high 105
        assert!(!is_bos_up(&prev, &current));
    }
}
