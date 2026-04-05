use crate::data::Candle;

/// Trading signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Up,
    Down,
    None,
}

// ── Fake Breakout ─────────────────────────────────────────────────────────────

/// Bearish fake breakout: breaks above prev high then closes back inside.
/// Requires an upper wick larger than the body (trap candle).
pub fn fake_breakout_above(prev: &Candle, cur: &Candle) -> bool {
    cur.high > prev.high
        && cur.close < prev.high
        && cur.upper_wick() > cur.body()
}

/// Bullish fake breakout: breaks below prev low then closes back inside.
pub fn fake_breakout_below(prev: &Candle, cur: &Candle) -> bool {
    cur.low < prev.low
        && cur.close > prev.low
        && cur.lower_wick() > cur.body()
}

// ── Rejection candle ─────────────────────────────────────────────────────────

/// Bearish rejection: upper wick > body × factor.
pub fn bearish_rejection(candle: &Candle, factor: f64) -> bool {
    candle.upper_wick() > candle.body() * factor
}

/// Bullish rejection: lower wick > body × factor.
pub fn bullish_rejection(candle: &Candle, factor: f64) -> bool {
    candle.lower_wick() > candle.body() * factor
}

// ── Break of Structure ────────────────────────────────────────────────────────

/// BOS Down: lower high AND close breaks previous low (bearish micro structure).
pub fn bos_down(prev: &Candle, cur: &Candle) -> bool {
    cur.high < prev.high && cur.close < prev.low
}

/// BOS Up: higher low AND close breaks previous high.
pub fn bos_up(prev: &Candle, cur: &Candle) -> bool {
    cur.low > prev.low && cur.close > prev.high
}

// ── Signal generator ─────────────────────────────────────────────────────────

const REJECTION_FACTOR: f64 = 1.5;

/// Combine all strategy components into a single Signal.
pub fn evaluate(prev: &Candle, cur: &Candle) -> Signal {
    if fake_breakout_above(prev, cur)
        && bearish_rejection(cur, REJECTION_FACTOR)
        && bos_down(prev, cur)
    {
        return Signal::Down;
    }

    if fake_breakout_below(prev, cur)
        && bullish_rejection(cur, REJECTION_FACTOR)
        && bos_up(prev, cur)
    {
        return Signal::Up;
    }

    Signal::None
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn c(open: f64, high: f64, low: f64, close: f64) -> Candle {
        Candle::new(open, high, low, close, 0)
    }

    // ── Fake Breakout ─────────────────────────────────────────────────────────

    #[test]
    fn fake_breakout_above_valid() {
        let prev = c(95.0, 100.0, 92.0, 97.0);
        // high=105 > prev.high=100, close=98 < prev.high, wick=105-98=7 > body=1
        let cur  = c(97.0, 105.0, 96.0, 98.0);
        assert!(fake_breakout_above(&prev, &cur));
    }

    #[test]
    fn fake_breakout_above_invalid_close_above() {
        let prev = c(95.0, 100.0, 92.0, 97.0);
        // close=102 > prev.high → not a fake breakout
        let cur  = c(97.0, 105.0, 96.0, 102.0);
        assert!(!fake_breakout_above(&prev, &cur));
    }

    #[test]
    fn fake_breakout_above_invalid_small_wick() {
        let prev = c(95.0, 100.0, 92.0, 97.0);
        // wick=1.5, body=3 → wick < body
        let cur  = c(96.0, 100.5, 95.0, 99.0);
        assert!(!fake_breakout_above(&prev, &cur));
    }

    // ── Rejection ─────────────────────────────────────────────────────────────

    #[test]
    fn bearish_rejection_valid() {
        // open=100, close=101, high=110 → body=1, wick=9 → 9 > 1.5
        let c_ = c(100.0, 110.0, 99.0, 101.0);
        assert!(bearish_rejection(&c_, 1.5));
    }

    #[test]
    fn bearish_rejection_invalid() {
        let c_ = c(100.0, 110.0, 99.0, 108.0);
        assert!(!bearish_rejection(&c_, 1.5));
    }

    #[test]
    fn bullish_rejection_valid() {
        let c_ = c(100.0, 101.0, 90.0, 99.0);
        assert!(bullish_rejection(&c_, 1.5));
    }

    #[test]
    fn bullish_rejection_invalid() {
        let c_ = c(100.0, 101.0, 90.0, 92.0);
        assert!(!bullish_rejection(&c_, 1.5));
    }

    // ── BOS ───────────────────────────────────────────────────────────────────

    #[test]
    fn bos_down_valid() {
        let prev = c(98.0, 105.0, 95.0, 97.0);
        let cur  = c(97.0, 103.0, 92.0, 93.0);
        assert!(bos_down(&prev, &cur));
    }

    #[test]
    fn bos_down_invalid() {
        let prev = c(98.0, 105.0, 95.0, 97.0);
        let cur  = c(97.0, 103.0, 94.0, 96.0);
        assert!(!bos_down(&prev, &cur));
    }

    #[test]
    fn bos_up_valid() {
        let prev = c(98.0, 105.0, 95.0, 102.0);
        let cur  = c(100.0, 108.0, 97.0, 107.0);
        assert!(bos_up(&prev, &cur));
    }

    #[test]
    fn bos_up_invalid() {
        let prev = c(98.0, 105.0, 95.0, 102.0);
        let cur  = c(100.0, 108.0, 97.0, 103.0);
        assert!(!bos_up(&prev, &cur));
    }

    // ── Signal output ─────────────────────────────────────────────────────────

    #[test]
    fn signal_none_flat_pair() {
        let prev = c(100.0, 101.0, 99.0, 100.0);
        let cur  = c(100.0, 101.0, 99.0, 100.0);
        assert_eq!(evaluate(&prev, &cur), Signal::None);
    }
}
