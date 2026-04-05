use crate::{
    data::Candle,
    features::{Ema, Rsi, Atr},
    strategy::{Signal, evaluate as strategy_eval},
    regime::{classify, RegimeInput},
    ml::{ModelOutput, ensemble},
    ev::compute as ev_compute,
    decision::{decide, BrainInput, Decision},
    execution::Side,
};

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Paper-trading configuration.
#[derive(Debug, Clone, Copy)]
pub struct PaperConfig {
    /// Starting virtual balance.
    pub initial_balance:  f64,
    /// Fixed dollar size per trade.
    pub position_size:    f64,
    /// Percentage gain from entry that triggers take-profit (e.g. 0.02 = 2 %).
    pub take_profit_pct:  f64,
    /// Percentage loss from entry that triggers stop-loss (e.g. 0.01 = 1 %).
    pub stop_loss_pct:    f64,
    /// Maximum number of bars to hold before forced exit.
    pub max_hold_bars:    usize,
    /// Minimum EV to accept a trade.
    pub ev_threshold:     f64,
    /// Minimum p_win to accept a trade.
    pub p_win_min:        f64,
}

impl Default for PaperConfig {
    fn default() -> Self {
        Self {
            initial_balance:  10_000.0,
            position_size:    100.0,
            take_profit_pct:  0.02,
            stop_loss_pct:    0.01,
            max_hold_bars:    10,
            ev_threshold:     0.02,
            p_win_min:        0.52,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Exit reason
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    TakeProfit,
    StopLoss,
    BarLimit,
}

// ─────────────────────────────────────────────────────────────────────────────
// Trade record (closed trade)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct TradeRecord {
    pub id:           u32,
    pub side:         Side,
    pub entry_price:  f64,
    pub exit_price:   f64,
    pub size:         f64,
    pub pnl:          f64,
    pub pnl_pct:      f64,
    pub is_win:       bool,
    pub exit_reason:  ExitReason,
    pub bars_held:    usize,
    pub entry_ts:     u64,
    pub exit_ts:      u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Open position (internal)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
struct OpenPosition {
    id:          u32,
    side:        Side,
    entry_price: f64,
    size:        f64,
    bars_held:   usize,
    entry_ts:    u64,
}

impl OpenPosition {
    fn unrealized_pnl(&self, current_price: f64) -> f64 {
        match self.side {
            Side::Yes => (current_price - self.entry_price) / self.entry_price * self.size,
            Side::No  => (self.entry_price - current_price) / self.entry_price * self.size,
        }
    }

    fn pnl_pct(&self, current_price: f64) -> f64 {
        match self.side {
            Side::Yes => (current_price - self.entry_price) / self.entry_price,
            Side::No  => (self.entry_price - current_price) / self.entry_price,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Aggregate statistics
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PaperStats {
    pub balance:       f64,
    pub total_trades:  usize,
    pub wins:          usize,
    pub losses:        usize,
    pub winrate:       f64,
    pub net_pnl:       f64,
    pub profit_factor: f64,
    pub max_drawdown:  f64,
    pub open_trades:   usize,
}

impl PaperStats {
    /// Returns a formatted multi-line summary string.
    pub fn display(&self) -> String {
        let pf = if self.profit_factor == f64::INFINITY {
            "∞".to_string()
        } else {
            format!("{:.2}", self.profit_factor)
        };
        format!(
            "┌─ Paper Trade Summary ───────────────────\n\
             │  Balance      : ${:.2}\n\
             │  Net PnL      : ${:+.2}\n\
             │  Trades       : {}  (W:{} / L:{})\n\
             │  Win Rate     : {:.1}%\n\
             │  Profit Factor: {}\n\
             │  Max Drawdown : ${:.2}\n\
             │  Open         : {}\n\
             └─────────────────────────────────────────",
            self.balance,
            self.net_pnl,
            self.total_trades,
            self.wins,
            self.losses,
            self.winrate * 100.0,
            pf,
            self.max_drawdown,
            self.open_trades,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Paper Trader
// ─────────────────────────────────────────────────────────────────────────────

pub struct PaperTrader {
    config:         PaperConfig,
    balance:        f64,
    open_positions: Vec<OpenPosition>,
    pub trade_log:  Vec<TradeRecord>,
    equity_curve:   Vec<f64>,
    peak_equity:    f64,
    max_drawdown:   f64,

    // indicator state
    ema9:         Ema,
    ema21:        Ema,
    rsi14:        Rsi,
    atr14:        Atr,
    atr_baseline: f64,
    prev_candle:  Option<Candle>,
    bar_count:    u64,
    next_id:      u32,
}

impl PaperTrader {
    pub fn new(config: PaperConfig) -> Self {
        let balance = config.initial_balance;
        Self {
            config,
            balance,
            open_positions: Vec::new(),
            trade_log: Vec::new(),
            equity_curve: vec![balance],
            peak_equity: balance,
            max_drawdown: 0.0,
            ema9:         Ema::new(9),
            ema21:        Ema::new(21),
            rsi14:        Rsi::new(14),
            atr14:        Atr::new(14),
            atr_baseline: 0.0,
            prev_candle:  None,
            bar_count:    0,
            next_id:      1,
        }
    }

    /// Feed one candle through the full pipeline.
    /// Returns a list of trades closed on this bar.
    pub fn feed(&mut self, candle: Candle) -> Vec<TradeRecord> {
        self.bar_count += 1;
        let mut closed_this_bar: Vec<TradeRecord> = Vec::new();

        // ── 1. Update indicators ──────────────────────────────────────────────
        let ema9  = self.ema9.update(candle.close);
        let ema21 = self.ema21.update(candle.close);
        let rsi14 = self.rsi14.update(candle.close);
        let atr14 = self.atr14.update(&candle);

        if let Some(atr) = atr14 {
            self.atr_baseline = if self.atr_baseline == 0.0 {
                atr
            } else {
                self.atr_baseline * 0.95 + atr * 0.05
            };
        }

        // ── 2. Mark open positions to market, check TP/SL/bar-limit ──────────
        let mut remaining = Vec::new();
        for mut pos in self.open_positions.drain(..) {
            pos.bars_held += 1;
            let pnl_pct    = pos.pnl_pct(candle.close);
            let pnl        = pos.unrealized_pnl(candle.close);

            let reason = if pnl_pct >= self.config.take_profit_pct {
                Some(ExitReason::TakeProfit)
            } else if pnl_pct <= -self.config.stop_loss_pct {
                Some(ExitReason::StopLoss)
            } else if pos.bars_held >= self.config.max_hold_bars {
                Some(ExitReason::BarLimit)
            } else {
                None
            };

            if let Some(exit_reason) = reason {
                self.balance += self.config.position_size + pnl;
                let record = TradeRecord {
                    id:          pos.id,
                    side:        pos.side,
                    entry_price: pos.entry_price,
                    exit_price:  candle.close,
                    size:        pos.size,
                    pnl,
                    pnl_pct,
                    is_win:      pnl > 0.0,
                    exit_reason,
                    bars_held:   pos.bars_held,
                    entry_ts:    pos.entry_ts,
                    exit_ts:     candle.timestamp,
                };
                self.trade_log.push(record);
                closed_this_bar.push(record);
            } else {
                remaining.push(pos);
            }
        }
        self.open_positions = remaining;

        // ── 3. Detect regime ──────────────────────────────────────────────────
        let baseline = if self.atr_baseline > 0.0 { self.atr_baseline } else { 1.0 };
        let regime = classify(&RegimeInput {
            ema9,
            ema21,
            atr:             atr14.unwrap_or(baseline),
            atr_baseline:    baseline,
            ema_cross_count: 0,
        });

        // ── 4. Strategy signal ────────────────────────────────────────────────
        let signal = if let Some(prev) = self.prev_candle {
            strategy_eval(&prev, &candle)
        } else {
            Signal::None
        };
        self.prev_candle = Some(candle);

        // ── 5. ML p_win ───────────────────────────────────────────────────────
        let rsi_score = rsi14.map_or(0.5, |r| {
            if signal == Signal::Down { (100.0 - r) / 100.0 } else { r / 100.0 }
        });
        let models = [
            ModelOutput::new(0.5,       1.0),
            ModelOutput::new(rsi_score, 0.5),
        ];
        let p_win = ensemble(&models);

        // ── 6. EV ─────────────────────────────────────────────────────────────
        let ev = ev_compute(p_win, 0.5).value;

        // ── 7. Decision ───────────────────────────────────────────────────────
        let decision = decide(&BrainInput {
            signal,
            p_win,
            ev,
            regime,
            ev_threshold: self.config.ev_threshold,
            p_win_min:    self.config.p_win_min,
        });

        // ── 8. Open new position (if budget allows) ───────────────────────────
        if decision == Decision::Trade
            && signal != Signal::None
            && self.balance >= self.config.position_size
        {
            let side = match signal {
                Signal::Up   => Side::Yes,
                Signal::Down => Side::No,
                Signal::None => unreachable!(),
            };
            self.balance -= self.config.position_size;
            self.open_positions.push(OpenPosition {
                id:          self.next_id,
                side,
                entry_price: candle.close,
                size:        self.config.position_size,
                bars_held:   0,
                entry_ts:    candle.timestamp,
            });
            self.next_id += 1;
        }

        // ── 9. Update equity curve + drawdown ─────────────────────────────────
        let unrealized: f64 = self.open_positions.iter()
            .map(|p| p.unrealized_pnl(candle.close))
            .sum();
        let equity = self.balance + unrealized
            + self.open_positions.iter().map(|p| p.size).sum::<f64>();

        self.equity_curve.push(equity);
        if equity > self.peak_equity { self.peak_equity = equity; }
        let dd = self.peak_equity - equity;
        if dd > self.max_drawdown { self.max_drawdown = dd; }

        closed_this_bar
    }

    /// Snapshot of current performance.
    pub fn stats(&self) -> PaperStats {
        let total  = self.trade_log.len();
        let wins   = self.trade_log.iter().filter(|t| t.is_win).count();
        let losses = total - wins;
        let gross_profit: f64 = self.trade_log.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum();
        let gross_loss:   f64 = self.trade_log.iter().filter(|t| t.pnl < 0.0).map(|t| t.pnl.abs()).sum();

        PaperStats {
            balance:       self.balance,
            total_trades:  total,
            wins,
            losses,
            winrate:       if total == 0 { 0.0 } else { wins as f64 / total as f64 },
            net_pnl:       self.trade_log.iter().map(|t| t.pnl).sum(),
            profit_factor: if gross_loss == 0.0 {
                if gross_profit > 0.0 { f64::INFINITY } else { 0.0 }
            } else {
                gross_profit / gross_loss
            },
            max_drawdown:  self.max_drawdown,
            open_trades:   self.open_positions.len(),
        }
    }

    pub fn balance(&self)       -> f64       { self.balance }
    pub fn equity_curve(&self)  -> &[f64]    { &self.equity_curve }
    pub fn open_count(&self)    -> usize      { self.open_positions.len() }
}

// ─────────────────────────────────────────────────────────────────────────────
// TDD — Paper Trade Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn default_trader() -> PaperTrader {
        PaperTrader::new(PaperConfig::default())
    }

    fn candle(price: f64, ts: u64) -> Candle {
        Candle::new(price, price + 10.0, price - 10.0, price, ts)
    }

    // ── balance ───────────────────────────────────────────────────────────────

    #[test]
    fn initial_balance_matches_config() {
        let pt = default_trader();
        assert_eq!(pt.balance(), 10_000.0);
    }

    #[test]
    fn balance_never_negative_under_normal_feed() {
        let mut pt = PaperTrader::new(PaperConfig {
            initial_balance: 500.0,
            position_size:   100.0,
            ..PaperConfig::default()
        });
        let prices = [100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0, 107.0];
        for (i, &p) in prices.iter().enumerate() {
            pt.feed(candle(p, i as u64 * 60));
            assert!(pt.balance() >= 0.0, "negative balance at bar {i}");
        }
    }

    // ── equity curve ─────────────────────────────────────────────────────────

    #[test]
    fn equity_curve_starts_at_initial_balance() {
        let pt = default_trader();
        assert_eq!(pt.equity_curve()[0], 10_000.0);
    }

    #[test]
    fn equity_curve_grows_by_one_per_bar() {
        let mut pt = default_trader();
        for i in 0..5 {
            pt.feed(candle(100.0 + i as f64, i as u64 * 60));
        }
        assert_eq!(pt.equity_curve().len(), 6); // initial + 5 bars
    }

    // ── take-profit exit ──────────────────────────────────────────────────────

    #[test]
    fn take_profit_closes_yes_position() {
        // Manually plant a YES position and move price up 3 % (above tp=2 %)
        let mut pt = PaperTrader::new(PaperConfig {
            take_profit_pct: 0.02,
            stop_loss_pct:   0.01,
            max_hold_bars:   20,
            ..PaperConfig::default()
        });

        // Plant position directly (bypass pipeline for determinism)
        pt.balance -= pt.config.position_size;
        pt.open_positions.push(OpenPosition {
            id:          1,
            side:        Side::Yes,
            entry_price: 100.0,
            size:        pt.config.position_size,
            bars_held:   0,
            entry_ts:    0,
        });

        // Feed a candle with price +3 % → should trigger TP
        let closed = pt.feed(candle(103.0, 60));
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0].exit_reason, ExitReason::TakeProfit);
        assert!(closed[0].is_win);
    }

    // ── stop-loss exit ────────────────────────────────────────────────────────

    #[test]
    fn stop_loss_closes_yes_position() {
        let mut pt = PaperTrader::new(PaperConfig {
            take_profit_pct: 0.02,
            stop_loss_pct:   0.01,
            max_hold_bars:   20,
            ..PaperConfig::default()
        });

        pt.balance -= pt.config.position_size;
        pt.open_positions.push(OpenPosition {
            id:          1,
            side:        Side::Yes,
            entry_price: 100.0,
            size:        pt.config.position_size,
            bars_held:   0,
            entry_ts:    0,
        });

        // Price falls 2 % → exceeds sl=1 %
        let closed = pt.feed(candle(98.0, 60));
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0].exit_reason, ExitReason::StopLoss);
        assert!(!closed[0].is_win);
    }

    // ── bar-limit exit ────────────────────────────────────────────────────────

    #[test]
    fn bar_limit_exits_position_after_max_hold() {
        let max_bars = 3_usize;
        let mut pt = PaperTrader::new(PaperConfig {
            take_profit_pct: 0.50, // very wide — won't trigger
            stop_loss_pct:   0.50,
            max_hold_bars:   max_bars,
            ..PaperConfig::default()
        });

        pt.balance -= pt.config.position_size;
        pt.open_positions.push(OpenPosition {
            id:          1,
            side:        Side::Yes,
            entry_price: 100.0,
            size:        pt.config.position_size,
            bars_held:   0,
            entry_ts:    0,
        });

        let mut closed = Vec::new();
        for i in 1..=(max_bars + 1) {
            let c = pt.feed(candle(100.0, i as u64 * 60)); // flat price — no TP/SL
            closed.extend(c);
        }
        // Should have been force-closed at bar_limit
        let bar_limit_exits: Vec<_> = closed.iter()
            .filter(|t| t.exit_reason == ExitReason::BarLimit)
            .collect();
        assert_eq!(bar_limit_exits.len(), 1);
    }

    // ── stats ─────────────────────────────────────────────────────────────────

    #[test]
    fn stats_zero_before_any_trades() {
        let pt = default_trader();
        let s  = pt.stats();
        assert_eq!(s.total_trades, 0);
        assert_eq!(s.wins,         0);
        assert_eq!(s.winrate,      0.0);
        assert_eq!(s.net_pnl,      0.0);
    }

    #[test]
    fn stats_winrate_in_range() {
        let mut pt = default_trader();
        let prices: Vec<f64> = (0..50).map(|i| 100.0 + i as f64 * 0.5).collect();
        for (i, &p) in prices.iter().enumerate() {
            pt.feed(candle(p, i as u64 * 60));
        }
        let s = pt.stats();
        assert!(s.winrate >= 0.0 && s.winrate <= 1.0);
        assert_eq!(s.wins + s.losses, s.total_trades);
    }

    #[test]
    fn stats_max_drawdown_non_negative() {
        let mut pt = default_trader();
        for i in 0..30 {
            pt.feed(candle(100.0 - i as f64 * 0.5, i as u64 * 60));
        }
        assert!(pt.stats().max_drawdown >= 0.0);
    }

    // ── profit_factor ─────────────────────────────────────────────────────────

    #[test]
    fn profit_factor_non_negative() {
        let mut pt = default_trader();
        for i in 0..40 {
            pt.feed(candle(100.0 + (i as f64 % 3.0), i as u64 * 60));
        }
        assert!(pt.stats().profit_factor >= 0.0);
    }

    // ── no position opened when insufficient balance ──────────────────────────

    #[test]
    fn no_open_when_balance_too_low() {
        let mut pt = PaperTrader::new(PaperConfig {
            initial_balance: 50.0,
            position_size:   100.0,  // more than balance
            ..PaperConfig::default()
        });
        for i in 0..20 {
            pt.feed(candle(100.0 + i as f64, i as u64 * 60));
        }
        // Should never have gone negative
        assert!(pt.balance() >= 0.0);
    }

    // ── NO side ───────────────────────────────────────────────────────────────

    #[test]
    fn stop_loss_closes_no_position_on_price_rise() {
        let mut pt = PaperTrader::new(PaperConfig {
            take_profit_pct: 0.50,
            stop_loss_pct:   0.01,
            max_hold_bars:   20,
            ..PaperConfig::default()
        });

        pt.balance -= pt.config.position_size;
        pt.open_positions.push(OpenPosition {
            id:          1,
            side:        Side::No,
            entry_price: 100.0,
            size:        pt.config.position_size,
            bars_held:   0,
            entry_ts:    0,
        });

        // Price rises 2 % → NO position loses → SL hit
        let closed = pt.feed(candle(102.0, 60));
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0].exit_reason, ExitReason::StopLoss);
        assert!(!closed[0].is_win);
    }
}
