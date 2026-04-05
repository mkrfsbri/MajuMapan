/// Risk metrics computed from a return series.
#[derive(Debug, Clone, Copy)]
pub struct RiskMetrics {
    pub var:      f64,  // Value at Risk (e.g. 95th percentile loss)
    pub cvar:     f64,  // Conditional VaR (expected loss beyond VaR)
    pub drawdown: f64,  // Maximum drawdown (peak-to-trough)
    pub exposure: f64,  // Sum of absolute position sizes
}

/// Compute VaR at `confidence` level (e.g. 0.95).
/// Returns the loss threshold exceeded by (1-confidence) of observations.
pub fn var(returns: &[f64], confidence: f64) -> f64 {
    if returns.is_empty() { return 0.0; }
    let mut sorted = returns.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((1.0 - confidence) * sorted.len() as f64) as usize;
    let idx = idx.min(sorted.len() - 1);
    -sorted[idx]  // VaR is reported as a positive loss
}

/// Conditional VaR: mean of losses beyond VaR.
pub fn cvar(returns: &[f64], confidence: f64) -> f64 {
    if returns.is_empty() { return 0.0; }
    let threshold = -var(returns, confidence);  // negative return boundary
    let tail: Vec<f64> = returns.iter().copied().filter(|&r| r <= threshold).collect();
    if tail.is_empty() { return 0.0; }
    -tail.iter().sum::<f64>() / tail.len() as f64
}

/// Maximum peak-to-trough equity drawdown from cumulative returns.
pub fn max_drawdown(returns: &[f64]) -> f64 {
    let mut peak   = 0.0_f64;
    let mut equity = 0.0_f64;
    let mut dd     = 0.0_f64;

    for &r in returns {
        equity += r;
        if equity > peak { peak = equity; }
        let cur_dd = peak - equity;
        if cur_dd > dd { dd = cur_dd; }
    }
    dd
}

/// Risk limits.
#[derive(Debug, Clone, Copy)]
pub struct RiskLimits {
    pub max_var:      f64,
    pub max_drawdown: f64,
    pub max_exposure: f64,
}

/// Returns true if any limit is breached (trade should be blocked).
pub fn is_blocked(metrics: &RiskMetrics, limits: &RiskLimits) -> bool {
    metrics.var      > limits.max_var
        || metrics.drawdown > limits.max_drawdown
        || metrics.exposure > limits.max_exposure
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn var_non_negative() {
        let returns = vec![-0.05, 0.02, -0.03, 0.01, -0.07, 0.04];
        assert!(var(&returns, 0.95) >= 0.0);
    }

    #[test]
    fn cvar_gte_var() {
        let returns = vec![-0.05, 0.02, -0.03, 0.01, -0.07, 0.04, -0.10, 0.03];
        let v = var(&returns, 0.90);
        let c = cvar(&returns, 0.90);
        assert!(c >= v, "CVaR={c} < VaR={v}");
    }

    #[test]
    fn drawdown_non_negative() {
        let returns = vec![0.01, -0.05, 0.02, -0.03, 0.04];
        assert!(max_drawdown(&returns) >= 0.0);
    }

    #[test]
    fn drawdown_known_value() {
        // equity path: 0, 1, -4, -2, 2
        let returns = vec![1.0, -5.0, 2.0, 4.0];
        // equity: 1→-4 is a drop of 5 from peak
        let dd = max_drawdown(&returns);
        assert!((dd - 5.0).abs() < 1e-9, "dd={dd}");
    }

    #[test]
    fn risk_blocking_triggers_on_excess_var() {
        let metrics = RiskMetrics { var: 0.15, cvar: 0.20, drawdown: 0.05, exposure: 100.0 };
        let limits  = RiskLimits  { max_var: 0.10, max_drawdown: 0.20, max_exposure: 500.0 };
        assert!(is_blocked(&metrics, &limits));
    }

    #[test]
    fn risk_blocking_triggers_on_excess_drawdown() {
        let metrics = RiskMetrics { var: 0.05, cvar: 0.08, drawdown: 0.25, exposure: 100.0 };
        let limits  = RiskLimits  { max_var: 0.10, max_drawdown: 0.20, max_exposure: 500.0 };
        assert!(is_blocked(&metrics, &limits));
    }

    #[test]
    fn risk_not_blocked_when_within_limits() {
        let metrics = RiskMetrics { var: 0.05, cvar: 0.08, drawdown: 0.10, exposure: 200.0 };
        let limits  = RiskLimits  { max_var: 0.10, max_drawdown: 0.20, max_exposure: 500.0 };
        assert!(!is_blocked(&metrics, &limits));
    }
}
