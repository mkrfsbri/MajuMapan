/// Expected Value for a binary Polymarket contract.
///
/// price  = current YES price  (0.0 – 1.0)
/// p_win  = estimated win probability
///
/// For a YES position:
///   reward = 1.0 - price   (profit per unit if win)
///   risk   = price          (loss per unit if lose)
///   EV     = p_win × reward - (1 - p_win) × risk
///
/// For a NO position (price = 1 - yes_price):
///   reward = 1.0 - no_price
///   risk   = no_price
pub fn ev_yes(p_win: f64, price: f64) -> f64 {
    let reward = 1.0 - price;
    let risk   = price;
    p_win * reward - (1.0 - p_win) * risk
}

pub fn ev_no(p_win: f64, yes_price: f64) -> f64 {
    // buying NO at (1 - yes_price)
    let no_price = 1.0 - yes_price;
    ev_yes(p_win, no_price)
}

/// Compute EV and whether it is positive.
#[derive(Debug, Clone, Copy)]
pub struct EvResult {
    pub value:       f64,
    pub is_positive: bool,
}

pub fn compute(p_win: f64, price: f64) -> EvResult {
    let value = ev_yes(p_win, price);
    EvResult { value, is_positive: value > 0.0 }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_ev_high_p_win_low_price() {
        // p_win=0.7, price=0.4 → EV = 0.7×0.6 - 0.3×0.4 = 0.42 - 0.12 = 0.30
        let r = compute(0.7, 0.4);
        assert!(r.is_positive);
        assert!((r.value - 0.30).abs() < 1e-9, "ev={}", r.value);
    }

    #[test]
    fn negative_ev_low_p_win_high_price() {
        // p_win=0.3, price=0.7 → EV = 0.3×0.3 - 0.7×0.7 = 0.09 - 0.49 = -0.40
        let r = compute(0.3, 0.7);
        assert!(!r.is_positive);
        assert!((r.value - (-0.40)).abs() < 1e-9, "ev={}", r.value);
    }

    #[test]
    fn zero_ev_at_fair_price() {
        // p_win=0.6, price=0.6 → EV = 0.6×0.4 - 0.4×0.6 = 0
        let r = compute(0.6, 0.6);
        assert!((r.value).abs() < 1e-9);
    }

    #[test]
    fn ev_no_positive_when_p_win_high() {
        // Buying NO at (1 - 0.4) = 0.6 with 70% chance NO wins → positive EV
        // EV = 0.7×(1-0.6) - 0.3×0.6 = 0.28 - 0.18 = 0.10
        let v = ev_no(0.7, 0.4);
        assert!(v > 0.0, "ev_no={v}");
        assert!((v - 0.10).abs() < 1e-9, "ev_no={v}");
    }
}
