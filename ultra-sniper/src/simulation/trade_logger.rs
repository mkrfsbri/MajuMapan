//! Phase 17 — Trade logger.
//!
//! Records every closed trade with timestamp, signal context, prices, PnL,
//! and EV at entry.  Provides simple query helpers.

use crate::execution::Side;
use crate::simulation::paper_engine::CloseReason;

/// A single closed-trade record.
#[derive(Debug, Clone, Copy)]
pub struct TradeLog {
    pub side:        Side,
    pub entry_price: f64,
    pub exit_price:  f64,
    pub contracts:   f64,
    pub pnl:         f64,
    pub is_win:      bool,
    pub reason:      CloseReason,
    pub bars_held:   u32,
    pub entry_ts:    u64,
    pub exit_ts:     u64,
    /// Expected value computed at entry time.
    pub entry_ev:    f64,
}

impl TradeLog {
    /// Return/PnL as a fraction of cost (entry_price × contracts).
    pub fn return_pct(&self) -> f64 {
        let cost = self.entry_price * self.contracts;
        if cost == 0.0 { return 0.0; }
        self.pnl / cost
    }
}

/// Append-only store of trade logs with summary helpers.
#[derive(Debug, Default)]
pub struct TradeLogger {
    records: Vec<TradeLog>,
}

impl TradeLogger {
    pub fn new() -> Self { Self::default() }

    pub fn record(&mut self, log: TradeLog) { self.records.push(log); }

    pub fn logs(&self) -> &[TradeLog] { &self.records }

    pub fn total(&self)  -> usize { self.records.len() }
    pub fn wins(&self)   -> usize { self.records.iter().filter(|l| l.is_win).count() }
    pub fn losses(&self) -> usize { self.records.iter().filter(|l| !l.is_win).count() }

    pub fn winrate(&self) -> f64 {
        let t = self.total();
        if t == 0 { return 0.0; }
        self.wins() as f64 / t as f64
    }

    pub fn net_pnl(&self) -> f64 {
        self.records.iter().map(|l| l.pnl).sum()
    }

    pub fn gross_profit(&self) -> f64 {
        self.records.iter().filter(|l| l.is_win).map(|l| l.pnl).sum()
    }

    pub fn gross_loss(&self) -> f64 {
        self.records.iter().filter(|l| !l.is_win).map(|l| l.pnl).sum::<f64>().abs()
    }

    pub fn profit_factor(&self) -> f64 {
        let gl = self.gross_loss();
        if gl == 0.0 { return f64::INFINITY; }
        self.gross_profit() / gl
    }

    pub fn avg_bars_held(&self) -> f64 {
        let t = self.total();
        if t == 0 { return 0.0; }
        self.records.iter().map(|l| l.bars_held as f64).sum::<f64>() / t as f64
    }

    /// Format a summary box for display.
    pub fn display(&self) -> String {
        format!(
            "┌── Trade Log ───────────────────────────┐\n\
             │  Trades      : {:<6}                  │\n\
             │  Wins        : {:<6}                  │\n\
             │  Losses      : {:<6}                  │\n\
             │  Win-rate    : {:.1}%                  │\n\
             │  Net PnL     : {:<+10.4}              │\n\
             │  Profit Factor: {:<8.3}              │\n\
             │  Avg bars    : {:<6.1}                │\n\
             └────────────────────────────────────────┘",
            self.total(),
            self.wins(),
            self.losses(),
            self.winrate() * 100.0,
            self.net_pnl(),
            self.profit_factor(),
            self.avg_bars_held(),
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::paper_engine::CloseReason;

    fn win_log() -> TradeLog {
        TradeLog {
            side:        Side::Yes,
            entry_price: 0.50,
            exit_price:  0.62,
            contracts:   100.0,
            pnl:         12.0,
            is_win:      true,
            reason:      CloseReason::TakeProfit,
            bars_held:   3,
            entry_ts:    1_000,
            exit_ts:     1_003,
            entry_ev:    0.06,
        }
    }

    fn loss_log() -> TradeLog {
        TradeLog {
            side:        Side::Yes,
            entry_price: 0.50,
            exit_price:  0.44,
            contracts:   100.0,
            pnl:         -6.0,
            is_win:      false,
            reason:      CloseReason::StopLoss,
            bars_held:   2,
            entry_ts:    2_000,
            exit_ts:     2_002,
            entry_ev:    0.06,
        }
    }

    #[test]
    fn empty_logger_zeros() {
        let l = TradeLogger::new();
        assert_eq!(l.total(), 0);
        assert_eq!(l.winrate(), 0.0);
        assert_eq!(l.net_pnl(), 0.0);
    }

    #[test]
    fn record_and_count() {
        let mut l = TradeLogger::new();
        l.record(win_log());
        l.record(loss_log());
        assert_eq!(l.total(), 2);
        assert_eq!(l.wins(), 1);
        assert_eq!(l.losses(), 1);
    }

    #[test]
    fn winrate_correct() {
        let mut l = TradeLogger::new();
        l.record(win_log());
        l.record(loss_log());
        assert!((l.winrate() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn net_pnl_correct() {
        let mut l = TradeLogger::new();
        l.record(win_log());
        l.record(loss_log());
        assert!((l.net_pnl() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn gross_profit_sums_wins() {
        let mut l = TradeLogger::new();
        l.record(win_log());
        l.record(loss_log());
        assert!((l.gross_profit() - 12.0).abs() < 1e-9);
    }

    #[test]
    fn gross_loss_sums_losses() {
        let mut l = TradeLogger::new();
        l.record(win_log());
        l.record(loss_log());
        assert!((l.gross_loss() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn profit_factor_correct() {
        let mut l = TradeLogger::new();
        l.record(win_log());
        l.record(loss_log());
        // 12 / 6 = 2.0
        assert!((l.profit_factor() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn profit_factor_infinity_no_losses() {
        let mut l = TradeLogger::new();
        l.record(win_log());
        assert_eq!(l.profit_factor(), f64::INFINITY);
    }

    #[test]
    fn avg_bars_held() {
        let mut l = TradeLogger::new();
        l.record(win_log());  // 3 bars
        l.record(loss_log()); // 2 bars
        assert!((l.avg_bars_held() - 2.5).abs() < 1e-9);
    }

    #[test]
    fn return_pct_win() {
        let log = win_log(); // pnl=12, entry=0.50*100=50
        assert!((log.return_pct() - 12.0 / 50.0).abs() < 1e-9);
    }

    #[test]
    fn display_contains_trades() {
        let mut l = TradeLogger::new();
        l.record(win_log());
        assert!(l.display().contains("Trade Log"));
    }
}
