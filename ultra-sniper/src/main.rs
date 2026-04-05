use ultra_sniper::{
    paper_trade::PaperConfig,
    runner::{Runner, RunnerConfig},
};

// ── Polymarket condition ID — replace with a real market ID ──────────────────
// Find at polymarket.com → open a market → copy the conditionId from the URL
// or via: GET https://gamma-api.polymarket.com/markets?q=bitcoin
const POLY_CONDITION_ID: &str = "";

fn main() {
    let mut runner = Runner::new(RunnerConfig {
        symbol:            "BTCUSDT".into(),
        warmup_bars:       50,       // bars fed to seed indicators before live loop
        poll_secs:         60,       // fetch a new 1m candle every 60 s
        poly_condition_id: POLY_CONDITION_ID.into(),
        poly_poll_bars:    5,        // refresh Polymarket price every 5 bars
        ev_threshold:      0.02,
        p_win_min:         0.52,
        paper: PaperConfig {
            initial_balance:  10_000.0,
            position_size:    200.0,
            take_profit_pct:  0.02,   // 2 % TP
            stop_loss_pct:    0.01,   // 1 % SL
            max_hold_bars:    8,
            ev_threshold:     0.02,
            p_win_min:        0.52,
        },
    });

    // Blocks forever — Ctrl-C to stop
    runner.run();

    // Unreachable in normal operation; shown if run() returns
    println!("{}", runner.stats().display());
}

