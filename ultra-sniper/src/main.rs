use ultra_sniper::{
    data::{Candle, Aggregator, Timeframe},
    features::{Ema, Rsi, Atr, IndicatorState},
    strategy::{Signal, evaluate as strategy_eval},
    regime::{classify, RegimeInput},
    ml::{ModelOutput, ensemble},
    ev::compute as ev_compute,
    decision::{decide, BrainInput, Decision},
    allocator::{allocate, Opportunity},
    risk::{RiskMetrics, RiskLimits, is_blocked, max_drawdown},
    execution::Executor,
};

fn main() {
    // ── 1. Mock BTC 1m candle data ────────────────────────────────────────────
    let prices: &[f64] = &[
        29_500.0, 29_550.0, 29_600.0, 29_480.0, 29_700.0,
        29_750.0, 29_800.0, 29_650.0, 29_900.0, 30_000.0,
        29_950.0, 29_800.0, 29_700.0, 29_600.0, 29_500.0,
        29_400.0, 29_450.0, 29_600.0, 29_750.0, 30_000.0,
        30_050.0, 30_100.0, 30_200.0, 30_150.0, 30_300.0,
    ];

    // ── 2. Build candles ──────────────────────────────────────────────────────
    let candles: Vec<Candle> = prices.iter().enumerate().map(|(i, &p)| {
        Candle::new(p, p + 30.0, p - 30.0, p + 5.0, i as u64 * 60)
    }).collect();

    // ── 3. Compute features ───────────────────────────────────────────────────
    let mut ema9  = Ema::new(9);
    let mut ema21 = Ema::new(21);
    let mut rsi14 = Rsi::new(14);
    let mut atr14 = Atr::new(14);
    let mut agg5  = Aggregator::new(Timeframe::M5);
    let mut agg15 = Aggregator::new(Timeframe::M15);

    let mut indicators: Vec<IndicatorState> = Vec::new();
    let mut atr_baseline = 60.0_f64; // initial estimate
    let mut returns: Vec<f64> = Vec::new();
    let mut executor = Executor::new();

    let mut prev_candle: Option<Candle> = None;

    for candle in &candles {
        // Aggregate (side effect only — used for multi-TF context)
        let _ = agg5.push(*candle);
        let _ = agg15.push(*candle);

        // Update indicators
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

        // ── 4. Detect regime ─────────────────────────────────────────────────
        let regime_input = RegimeInput {
            ema9:            state.ema9,
            ema21:           state.ema21,
            atr:             state.atr14.unwrap_or(atr_baseline),
            atr_baseline,
            ema_cross_count: 0,
        };
        let regime = classify(&regime_input);

        // ── 5. Run strategy ───────────────────────────────────────────────────
        let signal = if let Some(prev) = prev_candle {
            strategy_eval(&prev, candle)
        } else {
            Signal::None
        };
        prev_candle = Some(*candle);

        // ── 6. ML: predict P_win ─────────────────────────────────────────────
        let rsi_score = state.rsi14.map_or(0.5, |r| {
            if signal == Signal::Down { (100.0 - r) / 100.0 }
            else                      { r / 100.0 }
        });
        let models = [
            ModelOutput::new(0.5,        1.0),  // base model (stub)
            ModelOutput::new(rsi_score,  0.5),  // RSI-based model
        ];
        let p_win = ensemble(&models);

        // ── 7. Compute EV ─────────────────────────────────────────────────────
        let market_price = 0.5_f64;  // mock binary price
        let ev_result    = ev_compute(p_win, market_price);

        // ── 8. Decision ───────────────────────────────────────────────────────
        let brain = BrainInput {
            signal,
            p_win,
            ev:           ev_result.value,
            regime,
            ev_threshold: 0.02,
            p_win_min:    0.52,
        };
        let decision = decide(&brain);

        // ── 9. Allocation ─────────────────────────────────────────────────────
        let opps = if decision == Decision::Trade {
            vec![Opportunity { id: 1, score: p_win }]
        } else {
            vec![]
        };
        let allocs = allocate(&opps, 1_000.0);

        // ── 10. Risk check ────────────────────────────────────────────────────
        let exposure: f64 = allocs.iter().map(|a| a.amount).sum();
        let metrics  = RiskMetrics {
            var:      0.05,
            cvar:     0.08,
            drawdown: max_drawdown(&returns),
            exposure,
        };
        let limits = RiskLimits {
            max_var:      0.15,
            max_drawdown: 0.30,
            max_exposure: 2_000.0,
        };
        let blocked = is_blocked(&metrics, &limits);

        // ── 11. Execution ─────────────────────────────────────────────────────
        if !blocked && decision == Decision::Trade && signal != Signal::None {
            if let Some(alloc) = allocs.first() {
                if let Some(id) = executor.open(signal, market_price, alloc.amount) {
                    // Immediately settle at mock resolution price
                    let resolution = if signal == Signal::Down { 0.3 } else { 0.7 };
                    if let Some(pnl) = executor.close(id, resolution) {
                        returns.push(pnl);
                    }
                }
            }
        }
    }

    // ── 12. Summary output ─────────────────────────────────────────────────
    let total_trades = executor.positions.len();
    let wins = executor.positions.iter().filter(|p| p.pnl > 0.0).count();
    let _ = (total_trades, wins); // suppress unused warnings in no-print binary
}

