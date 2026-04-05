use ultra_sniper::{
    data::{Candle, Aggregator, Timeframe},
    feed::{BinanceFeed, PolymarketFeed, CandleFeed, MarketFeed},
    feed::binance::Interval,
    features::{Ema, Rsi, Atr, IndicatorState},
    strategy::{Signal, evaluate as strategy_eval},
    regime::{classify, RegimeInput},
    ml::{ModelOutput, ensemble},
    ev::compute as ev_compute,
    decision::{decide, BrainInput, Decision},
    allocator::{allocate, Opportunity},
    risk::{RiskMetrics, RiskLimits, is_blocked, max_drawdown},
    execution::Executor,
    paper_trade::{PaperTrader, PaperConfig},
};

// Polymarket condition ID for "BTC above X by date" — replace with real ID
const POLY_CONDITION_ID: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";

fn main() {
    // ── 1. Fetch candles from Binance (fallback to mock if offline) ───────────
    let binance = BinanceFeed::new("BTCUSDT", Interval::M1);
    let candles: Vec<Candle> = match binance.fetch_candles(100) {
        Ok(c) => {
            println!("[binance] fetched {} candles (BTCUSDT 1m)", c.len());
            c
        }
        Err(e) => {
            println!("[binance] offline — using mock data ({e})");
            let prices: &[f64] = &[
                29_500.0, 29_550.0, 29_600.0, 29_480.0, 29_700.0,
                29_750.0, 29_800.0, 29_650.0, 29_900.0, 30_000.0,
                29_950.0, 29_800.0, 29_700.0, 29_600.0, 29_500.0,
                29_400.0, 29_450.0, 29_600.0, 29_750.0, 30_000.0,
                30_050.0, 30_100.0, 30_200.0, 30_150.0, 30_300.0,
            ];
            prices.iter().enumerate()
                .map(|(i, &p)| Candle::new(p, p + 30.0, p - 30.0, p + 5.0, i as u64 * 60))
                .collect()
        }
    };

    // ── 2. Fetch Polymarket YES price (fallback to 0.5 if offline) ───────────
    let poly = PolymarketFeed::new();
    let market_price: f64 = match poly.fetch_price(POLY_CONDITION_ID) {
        Ok(mp) if mp.active => {
            println!("[polymarket] YES={:.3}  NO={:.3}  vol=${:.0}", mp.yes_price, mp.no_price, mp.volume);
            mp.yes_price
        }
        Ok(_) => {
            println!("[polymarket] market inactive — using 0.50");
            0.5
        }
        Err(e) => {
            println!("[polymarket] offline — using 0.50 ({e})");
            0.5
        }
    };

    // ── 3. Compute features ───────────────────────────────────────────────────
    let mut ema9  = Ema::new(9);
    let mut ema21 = Ema::new(21);
    let mut rsi14 = Rsi::new(14);
    let mut atr14 = Atr::new(14);
    let mut agg5  = Aggregator::new(Timeframe::M5);
    let mut agg15 = Aggregator::new(Timeframe::M15);

    let mut indicators: Vec<IndicatorState> = Vec::new();
    let mut atr_baseline = 60.0_f64;
    let mut returns: Vec<f64> = Vec::new();
    let mut executor = Executor::new();

    // ── Paper trader (runs in parallel with live pipeline) ───────────────────
    let mut paper = PaperTrader::new(PaperConfig {
        initial_balance: 10_000.0,
        position_size:   200.0,
        take_profit_pct: 0.02,
        stop_loss_pct:   0.01,
        max_hold_bars:   8,
        ev_threshold:    0.02,
        p_win_min:       0.52,
    });

    let mut prev_candle: Option<Candle> = None;

    for candle in &candles {
        let _ = agg5.push(*candle);
        let _ = agg15.push(*candle);

        let state = IndicatorState {
            ema9:  ema9.update(candle.close),
            ema21: ema21.update(candle.close),
            rsi14: rsi14.update(candle.close),
            atr14: atr14.update(candle),
        };
        indicators.push(state);

        if let Some(atr_val) = state.atr14 {
            atr_baseline = atr_baseline * 0.95 + atr_val * 0.05;
        }

        let regime = classify(&RegimeInput {
            ema9:            state.ema9,
            ema21:           state.ema21,
            atr:             state.atr14.unwrap_or(atr_baseline),
            atr_baseline,
            ema_cross_count: 0,
        });

        let signal = if let Some(prev) = prev_candle {
            strategy_eval(&prev, candle)
        } else {
            Signal::None
        };
        prev_candle = Some(*candle);

        let rsi_score = state.rsi14.map_or(0.5, |r| {
            if signal == Signal::Down { (100.0 - r) / 100.0 } else { r / 100.0 }
        });
        let models = [
            ModelOutput::new(0.5,       1.0),
            ModelOutput::new(rsi_score, 0.5),
        ];
        let p_win     = ensemble(&models);
        let ev_result = ev_compute(p_win, market_price);

        let decision = decide(&BrainInput {
            signal,
            p_win,
            ev:           ev_result.value,
            regime,
            ev_threshold: 0.02,
            p_win_min:    0.52,
        });

        let opps   = if decision == Decision::Trade {
            vec![Opportunity { id: 1, score: p_win }]
        } else { vec![] };
        let allocs = allocate(&opps, 1_000.0);

        let exposure = allocs.iter().map(|a| a.amount).sum();
        let blocked  = is_blocked(
            &RiskMetrics { var: 0.05, cvar: 0.08, drawdown: max_drawdown(&returns), exposure },
            &RiskLimits  { max_var: 0.15, max_drawdown: 0.30, max_exposure: 2_000.0 },
        );

        if !blocked && decision == Decision::Trade && signal != Signal::None {
            if let Some(alloc) = allocs.first() {
                if let Some(id) = executor.open(signal, market_price, alloc.amount) {
                    let resolution = if signal == Signal::Down { 0.3 } else { 0.7 };
                    if let Some(pnl) = executor.close(id, resolution) {
                        returns.push(pnl);
                    }
                }
            }
        }

        // ── Paper trade feed ──────────────────────────────────────────────────
        let closed = paper.feed(*candle);

        // Print each closed trade as it happens
        for t in &closed {
            println!(
                "[trade #{:>3}] {:?} | entry={:.2} exit={:.2} | pnl={:+.2} ({:+.1}%) | {:?} after {} bars",
                t.id,
                t.side,
                t.entry_price,
                t.exit_price,
                t.pnl,
                t.pnl_pct * 100.0,
                t.exit_reason,
                t.bars_held,
            );
        }
    }

    // ── Final summary (paper trader) ─────────────────────────────────────────
    println!("{}", paper.stats().display());
}

