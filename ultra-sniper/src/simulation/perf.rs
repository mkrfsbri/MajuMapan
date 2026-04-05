//! Phase 20 — Performance metrics.
//!
//! Computes Sharpe ratio, Sortino ratio, max drawdown, CAGR (stub),
//! and a formatted summary from a slice of per-trade PnL values.

/// Full performance snapshot.
#[derive(Debug, Clone)]
pub struct PerfMetrics {
    pub total_trades:  usize,
    pub wins:          usize,
    pub losses:        usize,
    pub winrate:       f64,
    pub net_pnl:       f64,
    pub avg_win:       f64,
    pub avg_loss:      f64,
    pub profit_factor: f64,
    pub sharpe:        f64,
    pub sortino:       f64,
    pub max_drawdown:  f64,   // fraction, e.g. 0.10 = 10 %
    pub expectancy:    f64,   // avg PnL per trade
}

impl PerfMetrics {
    /// Compute metrics from a slice of per-trade PnL values.
    ///
    /// `initial_balance` is used to build the equity curve for drawdown.
    pub fn compute(pnls: &[f64], initial_balance: f64) -> Self {
        let total = pnls.len();
        if total == 0 {
            return Self::zero();
        }

        let wins:   Vec<f64> = pnls.iter().copied().filter(|&p| p > 0.0).collect();
        let losses: Vec<f64> = pnls.iter().copied().filter(|&p| p <= 0.0).collect();

        let net_pnl:      f64 = pnls.iter().sum();
        let avg_win:      f64 = mean(&wins);
        let avg_loss:     f64 = mean(&losses).abs();
        let profit_factor: f64 = {
            let gp: f64 = wins.iter().sum();
            let gl: f64 = losses.iter().map(|x| x.abs()).sum();
            if gl == 0.0 { f64::INFINITY } else { gp / gl }
        };

        let n = total as f64;
        let mean_pnl = net_pnl / n;
        let sharpe   = sharpe_ratio(pnls);
        let sortino  = sortino_ratio(pnls);

        let winrate    = wins.len() as f64 / n;
        let expectancy = mean_pnl;
        let max_dd     = max_drawdown_from_pnls(pnls, initial_balance);

        Self {
            total_trades:  total,
            wins:          wins.len(),
            losses:        losses.len(),
            winrate,
            net_pnl,
            avg_win,
            avg_loss,
            profit_factor,
            sharpe,
            sortino,
            max_drawdown:  max_dd,
            expectancy,
        }
    }

    fn zero() -> Self {
        Self {
            total_trades:  0,
            wins:          0,
            losses:        0,
            winrate:       0.0,
            net_pnl:       0.0,
            avg_win:       0.0,
            avg_loss:      0.0,
            profit_factor: 0.0,
            sharpe:        0.0,
            sortino:       0.0,
            max_drawdown:  0.0,
            expectancy:    0.0,
        }
    }

    pub fn display(&self) -> String {
        format!(
            "┌── Performance ─────────────────────────────────┐\n\
             │  Trades        : {:<6}                        │\n\
             │  Wins / Losses : {} / {}                      │\n\
             │  Win-rate      : {:.1}%                        │\n\
             │  Net PnL       : {:+.4}                       │\n\
             │  Avg Win       : {:.4}                        │\n\
             │  Avg Loss      : {:.4}                        │\n\
             │  Profit Factor : {:.3}                        │\n\
             │  Sharpe        : {:.3}                        │\n\
             │  Sortino       : {:.3}                        │\n\
             │  Max Drawdown  : {:.2}%                        │\n\
             │  Expectancy    : {:+.4}                       │\n\
             └────────────────────────────────────────────────┘",
            self.total_trades,
            self.wins, self.losses,
            self.winrate * 100.0,
            self.net_pnl,
            self.avg_win,
            self.avg_loss,
            self.profit_factor,
            self.sharpe,
            self.sortino,
            self.max_drawdown * 100.0,
            self.expectancy,
        )
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() { return 0.0; }
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn std_dev(xs: &[f64]) -> f64 {
    if xs.len() < 2 { return 0.0; }
    let m = mean(xs);
    let var = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() - 1) as f64;
    var.sqrt()
}

fn sharpe_ratio(pnls: &[f64]) -> f64 {
    let m = mean(pnls);
    let s = std_dev(pnls);
    if s == 0.0 { return 0.0; }
    m / s
}

fn sortino_ratio(pnls: &[f64]) -> f64 {
    let m = mean(pnls);
    let downside: Vec<f64> = pnls.iter().copied().filter(|&p| p < 0.0).collect();
    if downside.is_empty() { return f64::INFINITY; }
    let ds = std_dev(&downside);
    if ds == 0.0 { return 0.0; }
    m / ds
}

/// Compute max drawdown from a PnL series using a running equity curve.
pub fn max_drawdown_from_pnls(pnls: &[f64], initial_balance: f64) -> f64 {
    let mut peak   = initial_balance;
    let mut equity = initial_balance;
    let mut max_dd = 0.0f64;
    for &p in pnls {
        equity += p;
        if equity > peak { peak = equity; }
        let dd = if peak > 0.0 { (peak - equity) / peak } else { 0.0 };
        if dd > max_dd { max_dd = dd; }
    }
    max_dd
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pnls() -> Vec<f64> {
        vec![10.0, -5.0, 8.0, -3.0, 12.0, -6.0, 7.0]
    }

    #[test]
    fn empty_pnls_returns_zero_metrics() {
        let m = PerfMetrics::compute(&[], 1000.0);
        assert_eq!(m.total_trades, 0);
        assert_eq!(m.net_pnl, 0.0);
    }

    #[test]
    fn total_trades_correct() {
        let m = PerfMetrics::compute(&sample_pnls(), 1000.0);
        assert_eq!(m.total_trades, 7);
    }

    #[test]
    fn wins_losses_correct() {
        let m = PerfMetrics::compute(&sample_pnls(), 1000.0);
        assert_eq!(m.wins, 4);
        assert_eq!(m.losses, 3);
    }

    #[test]
    fn winrate_correct() {
        let m = PerfMetrics::compute(&sample_pnls(), 1000.0);
        assert!((m.winrate - 4.0 / 7.0).abs() < 1e-9);
    }

    #[test]
    fn net_pnl_correct() {
        let m = PerfMetrics::compute(&sample_pnls(), 1000.0);
        // 10-5+8-3+12-6+7 = 23
        assert!((m.net_pnl - 23.0).abs() < 1e-9);
    }

    #[test]
    fn profit_factor_positive() {
        let m = PerfMetrics::compute(&sample_pnls(), 1000.0);
        assert!(m.profit_factor > 1.0);
    }

    #[test]
    fn sharpe_positive_on_profitable_series() {
        let m = PerfMetrics::compute(&sample_pnls(), 1000.0);
        assert!(m.sharpe > 0.0);
    }

    #[test]
    fn sortino_positive_on_profitable_series() {
        let m = PerfMetrics::compute(&sample_pnls(), 1000.0);
        assert!(m.sortino > 0.0);
    }

    #[test]
    fn max_drawdown_non_negative() {
        let m = PerfMetrics::compute(&sample_pnls(), 1000.0);
        assert!(m.max_drawdown >= 0.0);
    }

    #[test]
    fn max_drawdown_from_pnls_simple() {
        // equity: 100 → 110 → 105 → 113 → 110 → 122 → 116 → 123
        // peak after 110: dd=(110-105)/110 ~ 0.0455
        let pnls = vec![10.0, -5.0, 8.0, -3.0, 12.0, -6.0, 7.0];
        let dd = max_drawdown_from_pnls(&pnls, 100.0);
        assert!(dd > 0.0 && dd < 0.1);
    }

    #[test]
    fn display_contains_trades() {
        let m = PerfMetrics::compute(&sample_pnls(), 1000.0);
        assert!(m.display().contains("Performance"));
    }

    #[test]
    fn all_wins_sortino_infinity() {
        let pnls = vec![5.0, 3.0, 7.0];
        let m = PerfMetrics::compute(&pnls, 1000.0);
        assert_eq!(m.sortino, f64::INFINITY);
    }
}
