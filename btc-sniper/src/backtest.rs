use crate::types::{Candle, Signal};
use crate::pipeline::Pipeline;

/// Result of a single simulated trade.
#[derive(Debug, Clone, Copy)]
pub struct Trade {
    pub entry:  f64,
    pub exit:   f64,
    pub pnl:    f64,   // profit / loss in price units
    pub is_win: bool,
}

/// Aggregate backtest statistics.
#[derive(Debug, Clone, Copy)]
pub struct BacktestResult {
    pub total_trades:  usize,
    pub wins:          usize,
    pub losses:        usize,
    pub winrate:       f64,    // 0.0 – 1.0
    pub profit_factor: f64,    // gross_profit / gross_loss  (∞ if no losses)
    pub max_drawdown:  f64,    // maximum peak-to-trough equity drop
    pub net_pnl:       f64,
}

/// Simple fixed-risk backtest engine.
///
/// Entry: on Signal (next candle open).
/// Exit:  after `hold_bars` candles.
/// Risk:  fixed `risk_per_trade` price units per trade.
pub struct Backtester {
    pub hold_bars:       usize,
    pub risk_per_trade:  f64,
}

impl Backtester {
    pub fn new(hold_bars: usize, risk_per_trade: f64) -> Self {
        Self { hold_bars, risk_per_trade }
    }

    /// Run on a slice of historical candles.  Returns aggregated stats + trade log.
    pub fn run(&self, candles: &[Candle]) -> (BacktestResult, Vec<Trade>) {
        let mut pipeline: Pipeline<50> = Pipeline::new(9, 21, 14);
        let mut trades: Vec<Trade>     = Vec::new();
        let mut equity                 = 0.0_f64;
        let mut peak_equity            = 0.0_f64;
        let mut max_drawdown           = 0.0_f64;

        let n = candles.len();

        for i in 0..n {
            let signal = pipeline.feed(candles[i]).signal;

            // If signal fired and there is room for exit candle
            if i + self.hold_bars < n {
                let entry_price = candles[i + 1].open; // enter on next open
                let exit_price  = candles[i + self.hold_bars].close;

                let raw_pnl = match signal {
                    Signal::Down => entry_price - exit_price, // short
                    Signal::Up   => exit_price - entry_price, // long
                    Signal::None => continue,
                };

                // Scale to fixed risk
                let price_range = candles[i].high - candles[i].low;
                let scale = if price_range > 0.0 {
                    self.risk_per_trade / price_range
                } else {
                    1.0
                };
                let pnl = raw_pnl * scale;
                equity += pnl;

                if equity > peak_equity { peak_equity = equity; }
                let drawdown = peak_equity - equity;
                if drawdown > max_drawdown { max_drawdown = drawdown; }

                trades.push(Trade {
                    entry: entry_price,
                    exit:  exit_price,
                    pnl,
                    is_win: pnl > 0.0,
                });
            }
        }

        let result = Self::compute_stats(&trades, max_drawdown, equity);
        (result, trades)
    }

    fn compute_stats(trades: &[Trade], max_drawdown: f64, net_pnl: f64) -> BacktestResult {
        let total = trades.len();
        let wins  = trades.iter().filter(|t| t.is_win).count();
        let losses = total - wins;

        let gross_profit: f64 = trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum();
        let gross_loss:   f64 = trades.iter().filter(|t| t.pnl < 0.0).map(|t| t.pnl.abs()).sum();

        let winrate = if total == 0 { 0.0 } else { wins as f64 / total as f64 };
        let profit_factor = if gross_loss == 0.0 {
            if gross_profit > 0.0 { f64::INFINITY } else { 0.0 }
        } else {
            gross_profit / gross_loss
        };

        BacktestResult {
            total_trades: total,
            wins,
            losses,
            winrate,
            profit_factor,
            max_drawdown,
            net_pnl,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TDD — Step 7 Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    /// Build a simple rising price series.
    fn rising_series(n: usize) -> Vec<Candle> {
        (0..n).map(|i| {
            let p = 100.0 + i as f64;
            Candle::new(p, p + 2.0, p - 2.0, p + 0.5, 1.0)
        }).collect()
    }

    /// Build a chopped (alternating) price series.
    fn choppy_series(n: usize) -> Vec<Candle> {
        (0..n).map(|i| {
            let p = 100.0 + if i % 2 == 0 { 0.0 } else { 1.0 };
            Candle::new(p, p + 1.0, p - 1.0, p, 0.5)
        }).collect()
    }

    #[test]
    fn backtest_returns_deterministic_results() {
        let candles = rising_series(100);
        let bt = Backtester::new(3, 100.0);
        let (r1, _) = bt.run(&candles);
        let (r2, _) = bt.run(&candles);
        assert_eq!(r1.total_trades, r2.total_trades);
        assert!((r1.net_pnl - r2.net_pnl).abs() < 1e-9);
    }

    #[test]
    fn backtest_winrate_in_range() {
        let candles = rising_series(200);
        let bt = Backtester::new(5, 100.0);
        let (result, _) = bt.run(&candles);
        assert!(result.winrate >= 0.0 && result.winrate <= 1.0,
            "winrate={}", result.winrate);
    }

    #[test]
    fn backtest_profit_factor_non_negative() {
        let candles = choppy_series(150);
        let bt = Backtester::new(3, 50.0);
        let (result, _) = bt.run(&candles);
        assert!(result.profit_factor >= 0.0);
    }

    #[test]
    fn backtest_drawdown_non_negative() {
        let candles = rising_series(100);
        let bt = Backtester::new(3, 100.0);
        let (result, _) = bt.run(&candles);
        assert!(result.max_drawdown >= 0.0);
    }

    #[test]
    fn backtest_wins_plus_losses_equals_total() {
        let candles = rising_series(200);
        let bt = Backtester::new(5, 100.0);
        let (result, _) = bt.run(&candles);
        assert_eq!(result.wins + result.losses, result.total_trades);
    }

    #[test]
    fn backtest_trade_log_consistent_with_stats() {
        let candles = rising_series(200);
        let bt = Backtester::new(5, 100.0);
        let (result, trades) = bt.run(&candles);
        assert_eq!(trades.len(), result.total_trades);

        let wins_in_log = trades.iter().filter(|t| t.is_win).count();
        assert_eq!(wins_in_log, result.wins);
    }
}
