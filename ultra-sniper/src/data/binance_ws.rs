//! Binance WebSocket — streams 1m kline candles for a symbol.
//!
//! Endpoint: wss://stream.binance.com:9443/ws/{symbol}@kline_1m
//!
//! Reconnects automatically on disconnect with exponential back-off.

use futures_util::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use crate::data::Candle;

/// A parsed kline event from Binance WebSocket.
#[derive(Debug, Clone, Copy)]
pub struct KlineEvent {
    pub candle:   Candle,
    /// True when the 1m bar is closed (final tick for that minute).
    pub is_closed: bool,
}

/// Parse a raw Binance kline WebSocket JSON message.
///
/// Public so tests can verify parsing without a live connection.
pub fn parse_kline_message(raw: &str) -> Option<KlineEvent> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let k = v.get("k")?;

    let open  = k["o"].as_str()?.parse::<f64>().ok()?;
    let high  = k["h"].as_str()?.parse::<f64>().ok()?;
    let low   = k["l"].as_str()?.parse::<f64>().ok()?;
    let close = k["c"].as_str()?.parse::<f64>().ok()?;
    let ts_ms = k["t"].as_u64()?;
    let is_closed = k["x"].as_bool().unwrap_or(false);

    Some(KlineEvent {
        candle: Candle::new(open, high, low, close, ts_ms / 1000),
        is_closed,
    })
}

/// Connect to Binance kline stream and call `on_event` for every message.
///
/// Reconnects up to `max_retries` times with doubling delay (1s → 2s → 4s…).
/// Passes `Err(msg)` to `on_event` on parse failures so the caller can log.
pub async fn stream_klines<F>(
    symbol:      &str,
    max_retries: u32,
    mut on_event: F,
) where
    F: FnMut(Result<KlineEvent, String>),
{
    let symbol  = symbol.to_lowercase();
    let url     = format!("wss://stream.binance.com:9443/ws/{}@kline_1m", symbol);
    let mut delay_secs = 1u64;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            tokio::time::sleep(tokio::time::Duration::from_secs(delay_secs)).await;
            delay_secs = (delay_secs * 2).min(30);
        }

        let ws = match connect_async(&url).await {
            Ok((ws, _)) => ws,
            Err(e) => {
                on_event(Err(format!("connect failed: {e}")));
                continue;
            }
        };

        let (_, mut read) = ws.split();
        delay_secs = 1; // reset on successful connect

        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(txt)) => {
                    match parse_kline_message(&txt) {
                        Some(ev) => on_event(Ok(ev)),
                        None     => on_event(Err(format!("parse failed: {txt}"))),
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(e) => {
                    on_event(Err(format!("ws error: {e}")));
                    break;
                }
                _ => {}
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests — parse only (no network)
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_msg(is_closed: bool) -> String {
        format!(r#"{{
          "e":"kline","E":1700000060000,"s":"BTCUSDT",
          "k":{{
            "t":1700000000000,"T":1700000059999,
            "s":"BTCUSDT","i":"1m",
            "o":"29500.00","h":"29600.00","l":"29400.00","c":"29550.00",
            "v":"12.5","x":{is_closed},
            "n":320
          }}
        }}"#)
    }

    #[test]
    fn parse_open_price() {
        let ev = parse_kline_message(&sample_msg(true)).unwrap();
        assert!((ev.candle.open - 29_500.0).abs() < 1e-6);
    }

    #[test]
    fn parse_high_price() {
        let ev = parse_kline_message(&sample_msg(true)).unwrap();
        assert!((ev.candle.high - 29_600.0).abs() < 1e-6);
    }

    #[test]
    fn parse_low_price() {
        let ev = parse_kline_message(&sample_msg(true)).unwrap();
        assert!((ev.candle.low - 29_400.0).abs() < 1e-6);
    }

    #[test]
    fn parse_close_price() {
        let ev = parse_kline_message(&sample_msg(true)).unwrap();
        assert!((ev.candle.close - 29_550.0).abs() < 1e-6);
    }

    #[test]
    fn parse_timestamp_ms_to_seconds() {
        let ev = parse_kline_message(&sample_msg(true)).unwrap();
        assert_eq!(ev.candle.timestamp, 1_700_000_000);
    }

    #[test]
    fn parse_is_closed_true() {
        let ev = parse_kline_message(&sample_msg(true)).unwrap();
        assert!(ev.is_closed);
    }

    #[test]
    fn parse_is_closed_false() {
        let ev = parse_kline_message(&sample_msg(false)).unwrap();
        assert!(!ev.is_closed);
    }

    #[test]
    fn parse_invalid_json_returns_none() {
        assert!(parse_kline_message("not json").is_none());
    }

    #[test]
    fn parse_missing_k_field_returns_none() {
        assert!(parse_kline_message(r#"{"e":"trade"}"#).is_none());
    }

    #[test]
    fn parsed_candle_is_valid() {
        let ev = parse_kline_message(&sample_msg(true)).unwrap();
        assert!(ev.candle.is_valid());
    }
}
