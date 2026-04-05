//! Auto-fetch runner — polls Binance every N seconds and runs
//! 1m / 5m / 15m pipelines concurrently.
//!
//! Flow per tick:
//!   Binance 1m candle
//!     ├─▶ TfPipeline(1m)  → Signal1m
//!     ├─▶ Aggregator5m    → (every 5 ticks) TfPipeline(5m)  → Signal5m
//!     └─▶ Aggregator15m   → (every 15 ticks) TfPipeline(15m) → Signal15m
//!
//!   Then: Polymarket price (every poly_poll_bars ticks)
//!         Paper trader feed (1m candle)
//!         Print status row

use std::time::{Duration, Instant};

use crate::{
    data::{Candle, Aggregator, Timeframe},
    features::{Ema, Rsi, Atr},
    strategy::{Signal, evaluate as strategy_eval},
    regime::{classify, RegimeInput},
    ml::{ModelOutput, ensemble},
    ev::compute as ev_compute,
    decision::{decide, BrainInput},
    feed::{BinanceFeed, PolymarketFeed, CandleFeed, MarketFeed, FeedError},
    feed::binance::Interval,
    paper_trade::{PaperTrader, PaperConfig},
};

// ─────────────────────────────────────────────────────────────────────────────
// Per-timeframe pipeline (indicators + strategy)
// ─────────────────────────────────────────────────────────────────────────────

pub struct TfPipeline {
    pub label:        &'static str,
    ema9:             Ema,
    ema21:            Ema,
    rsi14:            Rsi,
    atr14:            Atr,
    atr_baseline:     f64,
    prev_candle:      Option<Candle>,
}

#[derive(Debug, Clone, Copy)]
pub struct TfOutput {
    pub signal:   Signal,
    pub ema9:     f64,
    pub ema21:    f64,
    pub rsi14:    Option<f64>,
    pub atr14:    Option<f64>,
    pub p_win:    f64,
    pub ev:       f64,
}

impl TfPipeline {
    pub fn new(label: &'static str) -> Self {
        Self {
            label,
            ema9:         Ema::new(9),
            ema21:        Ema::new(21),
            rsi14:        Rsi::new(14),
            atr14:        Atr::new(14),
            atr_baseline: 0.0,
            prev_candle:  None,
        }
    }

    /// Feed one candle; returns indicators + signal.
    pub fn feed(&mut self, candle: Candle, market_price: f64) -> TfOutput {
        let ema9  = self.ema9.update(candle.close);
        let ema21 = self.ema21.update(candle.close);
        let rsi14 = self.rsi14.update(candle.close);
        let atr14 = self.atr14.update(&candle);

        if let Some(atr) = atr14 {
            self.atr_baseline = if self.atr_baseline == 0.0 { atr }
                                else { self.atr_baseline * 0.95 + atr * 0.05 };
        }

        let signal = if let Some(prev) = self.prev_candle {
            strategy_eval(&prev, &candle)
        } else {
            Signal::None
        };
        self.prev_candle = Some(candle);

        // ML p_win
        let rsi_score = rsi14.map_or(0.5, |r| {
            if signal == Signal::Down { (100.0 - r) / 100.0 } else { r / 100.0 }
        });
        let models = [ModelOutput::new(0.5, 1.0), ModelOutput::new(rsi_score, 0.5)];
        let p_win = ensemble(&models);
        let ev    = ev_compute(p_win, market_price).value;

        TfOutput { signal, ema9, ema21, rsi14, atr14, p_win, ev }
    }

    pub fn atr_baseline(&self) -> f64 { self.atr_baseline }
}

// ─────────────────────────────────────────────────────────────────────────────
// Runner config
// ─────────────────────────────────────────────────────────────────────────────

pub struct RunnerConfig {
    /// Binance symbol (e.g. "BTCUSDT")
    pub symbol:            String,
    /// How many historical candles to fetch on startup for indicator warmup
    pub warmup_bars:       u32,
    /// Seconds between each live poll (60 = every 1m candle)
    pub poll_secs:         u64,
    /// Polymarket condition ID
    pub poly_condition_id: String,
    /// Fetch Polymarket price every N bars
    pub poly_poll_bars:    u64,
    /// Paper trading config
    pub paper:             PaperConfig,
    /// EV gate threshold
    pub ev_threshold:      f64,
    /// Minimum p_win gate
    pub p_win_min:         f64,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            symbol:            "BTCUSDT".into(),
            warmup_bars:       50,
            poll_secs:         60,
            poly_condition_id: String::new(),
            poly_poll_bars:    5,
            paper:             PaperConfig::default(),
            ev_threshold:      0.02,
            p_win_min:         0.52,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tick result (returned per processed candle)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct TickResult {
    pub candle_ts: u64,
    pub tf1m:      TfOutput,
    pub tf5m:      Option<TfOutput>,
    pub tf15m:     Option<TfOutput>,
    pub market_price: f64,
}

impl TickResult {
    /// True if any timeframe has a non-None signal.
    pub fn has_signal(&self) -> bool {
        self.tf1m.signal != Signal::None
            || self.tf5m.map_or(false,  |o| o.signal != Signal::None)
            || self.tf15m.map_or(false, |o| o.signal != Signal::None)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Runner
// ─────────────────────────────────────────────────────────────────────────────

pub struct Runner {
    config:       RunnerConfig,
    binance:      BinanceFeed,
    poly:         PolymarketFeed,
    pipe1m:       TfPipeline,
    pipe5m:       TfPipeline,
    pipe15m:      TfPipeline,
    agg5:         Aggregator,
    agg15:        Aggregator,
    paper:        PaperTrader,
    last_ts:      u64,
    bar_count:    u64,
    market_price: f64,
}

impl Runner {
    pub fn new(config: RunnerConfig) -> Self {
        let paper = PaperTrader::new(config.paper);
        Self {
            binance:      BinanceFeed::new(&config.symbol, Interval::M1),
            poly:         PolymarketFeed::new(),
            pipe1m:       TfPipeline::new("1m"),
            pipe5m:       TfPipeline::new("5m"),
            pipe15m:      TfPipeline::new("15m"),
            agg5:         Aggregator::new(Timeframe::M5),
            agg15:        Aggregator::new(Timeframe::M15),
            paper,
            last_ts:      0,
            bar_count:    0,
            market_price: 0.5,
            config,
        }
    }

    /// Fetch historical candles and warm up all indicator state.
    pub fn warmup(&mut self) -> Result<usize, FeedError> {
        let candles = self.binance.fetch_candles(self.config.warmup_bars)?;
        let n = candles.len();
        for c in candles {
            self.process(c);
        }
        Ok(n)
    }

    /// Process one candle through all timeframe pipelines.
    /// Returns the tick result (also feeds paper trader).
    pub fn process(&mut self, candle: Candle) -> TickResult {
        self.bar_count += 1;
        self.last_ts = candle.timestamp;

        // Refresh Polymarket price on schedule
        if !self.config.poly_condition_id.is_empty()
            && self.bar_count % self.config.poly_poll_bars == 0
        {
            if let Ok(mp) = self.poly.fetch_price(&self.config.poly_condition_id) {
                if mp.active { self.market_price = mp.yes_price; }
            }
        }

        // 1m pipeline
        let tf1m = self.pipe1m.feed(candle, self.market_price);

        // 5m aggregator → pipeline
        let tf5m = self.agg5.push(candle).map(|c5| {
            self.pipe5m.feed(c5, self.market_price)
        });

        // 15m aggregator → pipeline
        let tf15m = self.agg15.push(candle).map(|c15| {
            self.pipe15m.feed(c15, self.market_price)
        });

        // Paper trader
        let closed = self.paper.feed(candle);
        for t in &closed {
            println!(
                "  [paper #{:>3}] {:?} entry={:.2} exit={:.2} pnl={:+.2} ({:+.1}%) {:?}",
                t.id, t.side, t.entry_price, t.exit_price,
                t.pnl, t.pnl_pct * 100.0, t.exit_reason,
            );
        }

        TickResult { candle_ts: candle.timestamp, tf1m, tf5m, tf15m, market_price: self.market_price }
    }

    /// Print a compact status line for this tick.
    pub fn print_tick(&self, r: &TickResult) {
        fn fmt_sig(s: Signal) -> &'static str {
            match s { Signal::Up => "UP  ", Signal::Down => "DOWN", Signal::None => "----" }
        }

        let tf5  = r.tf5m .map_or("----".into(), |o| fmt_sig(o.signal).to_string());
        let tf15 = r.tf15m.map_or("----".into(), |o| fmt_sig(o.signal).to_string());
        let s    = self.paper.stats();

        println!(
            "[ts={:<10}] 1m={} 5m={} 15m={}  p_win={:.2}  EV={:+.3}  bal=${:.0}  trades={}",
            r.candle_ts,
            fmt_sig(r.tf1m.signal),
            tf5,
            tf15,
            r.tf1m.p_win,
            r.tf1m.ev,
            s.balance,
            s.total_trades,
        );
    }

    /// Run the auto-fetch loop indefinitely (blocks).
    /// Fetches a fresh 1m candle every `poll_secs` seconds.
    pub fn run(&mut self) {
        println!("=== Ultra Sniper — Auto Fetch ({}) ===", self.config.symbol);

        // Warmup
        match self.warmup() {
            Ok(n)  => println!("[warmup] fed {n} historical candles"),
            Err(e) => println!("[warmup] failed: {e} — starting cold"),
        }

        println!("[runner] polling every {}s  |  Ctrl-C to stop\n", self.config.poll_secs);

        loop {
            let t0 = Instant::now();

            match self.binance.fetch_candles(2) {
                Ok(candles) => {
                    // The last candle is the forming one — use the second-to-last
                    // (confirmed closed) if available, otherwise take the latest.
                    let candle = if candles.len() >= 2 {
                        candles[candles.len() - 2]
                    } else {
                        candles[candles.len() - 1]
                    };

                    if candle.timestamp > self.last_ts {
                        let tick = self.process(candle);
                        self.print_tick(&tick);

                        if tick.has_signal() {
                            self.print_signal_alert(&tick);
                        }
                    }
                }
                Err(e) => println!("[fetch] error: {e}"),
            }

            // Sleep remainder of the poll interval
            let elapsed = t0.elapsed();
            let target  = Duration::from_secs(self.config.poll_secs);
            if elapsed < target {
                std::thread::sleep(target - elapsed);
            }
        }
    }

    fn print_signal_alert(&self, r: &TickResult) {
        let check = |sig: Signal, tf: &str| {
            if sig != Signal::None {
                println!(
                    "  *** SIGNAL {} on {} | market_price={:.3} ***",
                    match sig { Signal::Up => "UP", Signal::Down => "DOWN", _ => "" },
                    tf,
                    r.market_price,
                );
            }
        };
        check(r.tf1m.signal, "1m");
        if let Some(o) = r.tf5m  { check(o.signal, "5m");  }
        if let Some(o) = r.tf15m { check(o.signal, "15m"); }
    }

    pub fn stats(&self) -> crate::paper_trade::PaperStats { self.paper.stats() }
    pub fn bar_count(&self) -> u64 { self.bar_count }
    pub fn market_price(&self) -> f64 { self.market_price }
}

// ─────────────────────────────────────────────────────────────────────────────
// TDD Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn candle(price: f64, ts: u64) -> Candle {
        Candle::new(price, price + 10.0, price - 10.0, price, ts)
    }

    fn runner() -> Runner {
        Runner::new(RunnerConfig {
            symbol:            "BTCUSDT".into(),
            warmup_bars:       5,
            poll_secs:         60,
            poly_condition_id: String::new(), // no network in tests
            poly_poll_bars:    5,
            paper:             PaperConfig { initial_balance: 1_000.0, ..PaperConfig::default() },
            ev_threshold:      0.02,
            p_win_min:         0.52,
        })
    }

    // ── bar_count ─────────────────────────────────────────────────────────────

    #[test]
    fn bar_count_increments_per_process() {
        let mut r = runner();
        r.process(candle(100.0, 0));
        r.process(candle(101.0, 60));
        assert_eq!(r.bar_count(), 2);
    }

    // ── last_ts ───────────────────────────────────────────────────────────────

    #[test]
    fn last_ts_updated_after_process() {
        let mut r = runner();
        r.process(candle(100.0, 1_000));
        assert_eq!(r.last_ts, 1_000);
    }

    // ── 1m signal ─────────────────────────────────────────────────────────────

    #[test]
    fn tf1m_output_always_present() {
        let mut r  = runner();
        let tick   = r.process(candle(100.0, 0));
        // Signal may be None on first candle (no prev), but field exists
        let _ = tick.tf1m.signal;
    }

    // ── 5m aggregation ────────────────────────────────────────────────────────

    #[test]
    fn tf5m_none_before_5_bars() {
        let mut r = runner();
        for i in 0..4 {
            let tick = r.process(candle(100.0 + i as f64, i as u64 * 60));
            assert!(tick.tf5m.is_none(), "expected None at bar {i}");
        }
    }

    #[test]
    fn tf5m_some_on_5th_bar() {
        let mut r = runner();
        let mut last = None;
        for i in 0..5 {
            last = Some(r.process(candle(100.0 + i as f64, i as u64 * 60)));
        }
        assert!(last.unwrap().tf5m.is_some());
    }

    #[test]
    fn tf5m_fires_again_at_10th_bar() {
        let mut r = runner();
        let mut count = 0;
        for i in 0..10 {
            let tick = r.process(candle(100.0 + i as f64, i as u64 * 60));
            if tick.tf5m.is_some() { count += 1; }
        }
        assert_eq!(count, 2, "expected 2 completed 5m candles in 10 bars");
    }

    // ── 15m aggregation ───────────────────────────────────────────────────────

    #[test]
    fn tf15m_none_before_15_bars() {
        let mut r = runner();
        for i in 0..14 {
            let tick = r.process(candle(100.0 + i as f64, i as u64 * 60));
            assert!(tick.tf15m.is_none(), "expected None at bar {i}");
        }
    }

    #[test]
    fn tf15m_some_on_15th_bar() {
        let mut r = runner();
        let mut last = None;
        for i in 0..15 {
            last = Some(r.process(candle(100.0 + i as f64, i as u64 * 60)));
        }
        assert!(last.unwrap().tf15m.is_some());
    }

    // ── has_signal ────────────────────────────────────────────────────────────

    #[test]
    fn has_signal_false_when_all_none() {
        let mut r    = runner();
        let tick     = r.process(candle(100.0, 0));
        // Fresh runner, first bar — all signals are None
        assert!(!tick.has_signal());
    }

    // ── market_price default ─────────────────────────────────────────────────

    #[test]
    fn market_price_defaults_to_0_5() {
        let r = runner();
        assert_eq!(r.market_price(), 0.5);
    }

    // ── TfPipeline ────────────────────────────────────────────────────────────

    #[test]
    fn tf_pipeline_first_output_has_valid_ema() {
        let mut p   = TfPipeline::new("test");
        let out     = p.feed(candle(100.0, 0), 0.5);
        assert!((out.ema9 - 100.0).abs() < 1e-9);
    }

    #[test]
    fn tf_pipeline_rsi_none_before_warmup() {
        let mut p = TfPipeline::new("test");
        let out   = p.feed(candle(100.0, 0), 0.5);
        assert!(out.rsi14.is_none());
    }

    #[test]
    fn tf_pipeline_rsi_some_after_14_bars() {
        let mut p   = TfPipeline::new("test");
        let mut out = None;
        for i in 0..14 {
            out = Some(p.feed(candle(100.0 + i as f64, i as u64 * 60), 0.5));
        }
        assert!(out.unwrap().rsi14.is_some());
    }

    #[test]
    fn tf_pipeline_p_win_in_range() {
        let mut p = TfPipeline::new("test");
        for i in 0..5 {
            let out = p.feed(candle(100.0 + i as f64, i as u64 * 60), 0.5);
            assert!(out.p_win >= 0.0 && out.p_win <= 1.0);
        }
    }

    // ── TickResult market_price propagates ────────────────────────────────────

    #[test]
    fn tick_result_carries_market_price() {
        let mut r  = runner();
        let tick   = r.process(candle(100.0, 0));
        assert!((tick.market_price - 0.5).abs() < 1e-9);
    }
}
