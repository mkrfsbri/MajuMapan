use crate::strategy::Signal;
use crate::regime::Regime;

/// Decision output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Trade,
    Skip,
}

/// All inputs the brain needs to decide.
#[derive(Debug, Clone, Copy)]
pub struct BrainInput {
    pub signal:    Signal,
    pub p_win:     f64,
    pub ev:        f64,
    pub regime:    Regime,
    pub ev_threshold: f64,
    pub p_win_min:    f64,
}

/// Rule:
///  - signal must not be None
///  - EV > threshold
///  - p_win ≥ minimum
///  - regime is not HighVolatility (skip during chaotic markets)
pub fn decide(input: &BrainInput) -> Decision {
    if input.signal == Signal::None {
        return Decision::Skip;
    }
    if input.ev <= input.ev_threshold {
        return Decision::Skip;
    }
    if input.p_win < input.p_win_min {
        return Decision::Skip;
    }
    if input.regime == Regime::HighVolatility {
        return Decision::Skip;
    }
    Decision::Trade
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> BrainInput {
        BrainInput {
            signal:       Signal::Down,
            p_win:        0.65,
            ev:           0.15,
            regime:       Regime::Trending,
            ev_threshold: 0.05,
            p_win_min:    0.55,
        }
    }

    #[test]
    fn valid_trade_all_conditions_met() {
        assert_eq!(decide(&good()), Decision::Trade);
    }

    #[test]
    fn skip_when_signal_none() {
        let mut b = good();
        b.signal = Signal::None;
        assert_eq!(decide(&b), Decision::Skip);
    }

    #[test]
    fn skip_when_ev_below_threshold() {
        let mut b = good();
        b.ev = 0.02;
        assert_eq!(decide(&b), Decision::Skip);
    }

    #[test]
    fn skip_when_p_win_too_low() {
        let mut b = good();
        b.p_win = 0.40;
        assert_eq!(decide(&b), Decision::Skip);
    }

    #[test]
    fn skip_during_high_volatility() {
        let mut b = good();
        b.regime = Regime::HighVolatility;
        assert_eq!(decide(&b), Decision::Skip);
    }
}
