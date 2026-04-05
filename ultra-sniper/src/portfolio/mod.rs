/// Compute Sharpe ratio: mean(returns) / std(returns) × sqrt(periods_per_year).
/// Returns 0.0 when std is zero or slice is empty.
pub fn sharpe(returns: &[f64], periods_per_year: f64) -> f64 {
    if returns.len() < 2 { return 0.0; }

    let n    = returns.len() as f64;
    let mean = returns.iter().sum::<f64>() / n;
    let var  = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let std  = var.sqrt();

    if std == 0.0 { return 0.0; }
    mean / std * periods_per_year.sqrt()
}

/// Simple mean-variance weight allocation.
///
/// Weight_i = Sharpe_i / Σ Sharpe_j   (negative Sharpe → weight = 0)
pub fn mean_variance_weights(sharpe_ratios: &[f64]) -> Vec<f64> {
    if sharpe_ratios.is_empty() { return Vec::new(); }

    let clamped: Vec<f64> = sharpe_ratios.iter().map(|&s| s.max(0.0)).collect();
    let total: f64 = clamped.iter().sum();

    if total == 0.0 {
        // All non-positive → equal weight
        let eq = 1.0 / sharpe_ratios.len() as f64;
        return vec![eq; sharpe_ratios.len()];
    }

    clamped.iter().map(|&s| s / total).collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sharpe_positive_consistent_returns() {
        let returns = vec![0.01, 0.02, 0.01, 0.03, 0.02];
        let s = sharpe(&returns, 252.0);
        assert!(s > 0.0, "sharpe={s}");
    }

    #[test]
    fn sharpe_zero_for_empty() {
        assert_eq!(sharpe(&[], 252.0), 0.0);
    }

    #[test]
    fn sharpe_zero_for_constant_returns() {
        let returns = vec![0.01, 0.01, 0.01];
        assert_eq!(sharpe(&returns, 252.0), 0.0);
    }

    #[test]
    fn better_sharpe_gets_higher_weight() {
        // Strategy A: Sharpe 2.0  Strategy B: Sharpe 1.0
        let weights = mean_variance_weights(&[2.0, 1.0]);
        assert!(weights[0] > weights[1], "w0={} w1={}", weights[0], weights[1]);
    }

    #[test]
    fn weights_sum_to_one() {
        let weights = mean_variance_weights(&[1.5, 2.5, 1.0]);
        let sum: f64 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-9, "sum={sum}");
    }

    #[test]
    fn all_negative_sharpe_equal_weights() {
        let weights = mean_variance_weights(&[-1.0, -2.0]);
        assert!((weights[0] - 0.5).abs() < 1e-9);
        assert!((weights[1] - 0.5).abs() < 1e-9);
    }
}
