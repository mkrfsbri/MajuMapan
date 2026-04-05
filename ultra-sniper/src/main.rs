//! Ultra Sniper — Phase 19: Real-time async simulation loop.
//!
//! Connects two WebSocket streams in parallel:
//!   1. Binance kline (1m BTC/USDT) → feeds candles into indicator pipeline
//!   2. Polymarket CLOB              → keeps orderbook snapshot up to date
//!
//! On every closed 1m bar:
//!   - Run TA indicators + strategy signal
//!   - Compute EV against live orderbook
//!   - Route to PaperEngine (Mode::Paper) or print Live alert (Mode::Live)
//!   - Print running performance summary every N closed bars

use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use ultra_sniper::{
    data::{
        binance_ws::{stream_klines, KlineEvent},
        polymarket_ws::{stream_orderbook, BookEvent},
    },
    decision::{BrainInput, decide, Decision},
    ev::compute as ev_compute,
    execution::Side,
    features::{Ema, Rsi, Atr},
    ml::{ensemble, ModelOutput},
    regime::{classify, RegimeInput},
    simulation::{
        mode::Mode,
        paper_engine::{EngineConfig, PaperEngine},
        OrderBook,
    },
    strategy::{evaluate, Signal},
};

// ── Configuration ─────────────────────────────────────────────────────────────

const SYMBOL:       &str = "BTCUSDT";
const CONDITION_ID: &str = "";   // Set real Polymarket conditionId here
const MAX_RETRIES:  u32  = 10;
const STATS_EVERY:  u32  = 10;   // Print stats every N closed bars
const MODE:         Mode = Mode::Paper;

// ─────────────────────────────────────────────────────────────────────────────

/// Shared mutable orderbook updated by the Polymarket WS task.
type SharedBook = Arc<Mutex<OrderBook>>;

#[tokio::main]
async fn main() {
    eprintln!("[ultra-sniper] mode={} symbol={}", MODE, SYMBOL);

    let book: SharedBook = Arc::new(Mutex::new(OrderBook::neutral()));

    // ── Channel: Binance closed-bar events ───────────────────────────────────
    let (kline_tx, mut kline_rx) = mpsc::unbounded_channel::<KlineEvent>();

    // ── Task 1: Binance WebSocket ─────────────────────────────────────────────
    let kline_tx2 = kline_tx.clone();
    tokio::spawn(async move {
        stream_klines(SYMBOL, MAX_RETRIES, move |res| {
            match res {
                Ok(ev) if ev.is_closed => { let _ = kline_tx2.send(ev); }
                Err(e) => eprintln!("[binance-ws] {}", e),
                _ => {}
            }
        }).await;
    });

    // ── Task 2: Polymarket WebSocket ──────────────────────────────────────────
    let book_ws = Arc::clone(&book);
    if !CONDITION_ID.is_empty() {
        tokio::spawn(async move {
            stream_orderbook(CONDITION_ID, MAX_RETRIES, move |res| {
                match res {
                    Ok(BookEvent { book: b }) => {
                        if let Ok(mut guard) = book_ws.lock() { *guard = b; }
                    }
                    Err(e) => eprintln!("[polymarket-ws] {}", e),
                }
            }).await;
        });
    } else {
        eprintln!("[ultra-sniper] CONDITION_ID not set — orderbook will stay neutral");
    }

    // ── Main loop: process closed bars ───────────────────────────────────────
    let engine_cfg = EngineConfig::default();
    let mut engine = PaperEngine::new(engine_cfg);

    // Inline indicators (stateful).
    let mut ema9  = Ema::new(9);
    let mut ema21 = Ema::new(21);
    let mut rsi14 = Rsi::new(14);
    let mut atr14 = Atr::new(14);

    let mut prev_candle = None;
    let mut bars = 0u32;

    while let Some(ev) = kline_rx.recv().await {
        let candle = ev.candle;

        // Update indicators.
        let e9  = ema9.update(candle.close);
        let e21 = ema21.update(candle.close);
        let rsi = rsi14.update(candle.close);
        let atr = atr14.update(&candle);

        if let Some(prev) = prev_candle {
            let signal = evaluate(&prev, &candle);

            // Snapshot orderbook.
            let ob    = book.lock().map(|g| *g).unwrap_or_else(|_| OrderBook::neutral());
            let price = ob.mid_yes();
            let p_win = stub_p_win(e9, e21, rsi);
            let ev_res = ev_compute(p_win, price);
            let ev_val = ev_res.value;

            let regime = classify(&RegimeInput {
                ema9:            e9,
                ema21:           e21,
                atr:             atr.unwrap_or(0.0),
                atr_baseline:    0.0,
                ema_cross_count: 0,
            });

            let decision = decide(&BrainInput {
                signal,
                ev: ev_val,
                p_win,
                regime,
                ev_threshold: 0.02,
                p_win_min:    0.52,
            });

            // Tick existing positions first.
            let closed = engine.tick(&ob, candle.timestamp);
            for log in &closed {
                eprintln!(
                    "[trade closed] side={:?} pnl={:+.4} reason={:?} bars={}",
                    log.side, log.pnl, log.reason, log.bars_held
                );
            }

            // Open new position if decision says trade.
            if matches!(decision, Decision::Trade) {
                let side = match signal {
                    Signal::Up   => Some(Side::Yes),
                    Signal::Down => Some(Side::No),
                    Signal::None => None,
                };
                if let Some(s) = side {
                    if let Some(fill) = engine.open(s, &ob, ev_val, candle.timestamp) {
                        eprintln!(
                            "[trade open]   side={:?} price={:.4} contracts={:.2} ev={:.4}",
                            s, fill.fill_price, fill.contracts, ev_val
                        );
                    }
                }
            }

            bars += 1;
            if bars % STATS_EVERY == 0 {
                let s = engine.stats();
                eprintln!(
                    "[stats] bars={} trades={} wins={} winrate={:.1}% pnl={:+.2} dd={:.2}% bal={:.2}",
                    bars, s.total_trades, s.wins,
                    s.winrate * 100.0, s.net_pnl,
                    s.max_drawdown * 100.0, s.balance,
                );
            }
        }

        prev_candle = Some(candle);
    }

    // Final stats on shutdown.
    let s = engine.stats();
    eprintln!("\n[final stats]");
    eprintln!("  Balance      : {:.2}", s.balance);
    eprintln!("  Total trades : {}", s.total_trades);
    eprintln!("  Wins/Losses  : {}/{}", s.wins, s.losses);
    eprintln!("  Win-rate     : {:.1}%", s.winrate * 100.0);
    eprintln!("  Net PnL      : {:+.4}", s.net_pnl);
    eprintln!("  Max Drawdown : {:.2}%", s.max_drawdown * 100.0);
}

/// Stub p_win from RSI + EMA cross — replace with real ML ensemble.
fn stub_p_win(ema9: f64, ema21: f64, rsi: Option<f64>) -> f64 {
    let rsi_val  = rsi.unwrap_or(50.0);
    let ema_bull = ema9 > ema21;
    let models = &[
        ModelOutput { p_win: if rsi_val > 55.0 { 0.60 } else if rsi_val < 45.0 { 0.40 } else { 0.50 }, weight: 1.0 },
        ModelOutput { p_win: if ema_bull { 0.58 } else { 0.44 }, weight: 0.8 },
    ];
    ensemble(models)
}
