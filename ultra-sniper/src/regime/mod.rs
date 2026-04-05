/// Market regime classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Regime {
    Trending,
    Sideways,
    HighVolatility,
    LowVolatility,
}

/// Input snapshot needed to classify regime.
#[derive(Debug, Clone, Copy)]
pub struct RegimeInput {
    pub ema9:         f64,
    pub ema21:        f64,
    pub atr:          f64,
    pub atr_baseline: f64,  // long-term average ATR for normalisation
    pub ema_cross_count: usize, // # of EMA9/EMA21 crosses in recent window
}

/// Rule-based classifier.
///
/// Priority (first match wins):
///  1. HighVolatility  — ATR > 2× baseline
///  2. Trending        — |EMA9 - EMA21| / EMA21 > 0.5% AND cross_count ≤ 1
///  3. Sideways        — cross_count ≥ 3 OR EMA spread < 0.1%
///  4. LowVolatility   — ATR < 0.5× baseline
///  5. default         — Trending
pub fn classify(input: &RegimeInput) -> Regime {
    let spread_pct = if input.ema21 != 0.0 {
        (input.ema9 - input.ema21).abs() / input.ema21
    } else {
        0.0
    };

    if input.atr_baseline > 0.0 {
        let atr_ratio = input.atr / input.atr_baseline;

        if atr_ratio > 2.0 {
            return Regime::HighVolatility;
        }

        if spread_pct > 0.005 && input.ema_cross_count <= 1 {
            return Regime::Trending;
        }

        if input.ema_cross_count >= 3 || spread_pct < 0.001 {
            return Regime::Sideways;
        }

        if atr_ratio < 0.5 {
            return Regime::LowVolatility;
        }
    }

    // fallback: trend on EMA spread alone
    if spread_pct > 0.005 { Regime::Trending } else { Regime::Sideways }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn input(ema9: f64, ema21: f64, atr: f64, atr_base: f64, crosses: usize) -> RegimeInput {
        RegimeInput { ema9, ema21, atr, atr_baseline: atr_base, ema_cross_count: crosses }
    }

    #[test]
    fn classify_high_volatility() {
        // ATR = 3× baseline → HighVolatility
        let r = classify(&input(100.0, 99.0, 300.0, 100.0, 0));
        assert_eq!(r, Regime::HighVolatility);
    }

    #[test]
    fn classify_trending() {
        // spread > 0.5%, ATR normal, few crosses
        let r = classify(&input(101.0, 100.0, 80.0, 100.0, 0));
        assert_eq!(r, Regime::Trending);
    }

    #[test]
    fn classify_sideways_many_crosses() {
        // spread > 0.5% but 4 crosses → Sideways
        let r = classify(&input(101.0, 100.0, 80.0, 100.0, 4));
        assert_eq!(r, Regime::Sideways);
    }

    #[test]
    fn classify_sideways_tiny_spread() {
        // EMA spread < 0.1% → Sideways
        let r = classify(&input(100.05, 100.0, 80.0, 100.0, 0));
        assert_eq!(r, Regime::Sideways);
    }

    #[test]
    fn classify_low_volatility() {
        // spread between 0.1% and 0.5%, ATR < 0.5× baseline
        let r = classify(&input(100.3, 100.0, 30.0, 100.0, 2));
        assert_eq!(r, Regime::LowVolatility);
    }
}
