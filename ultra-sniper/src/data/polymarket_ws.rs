//! Polymarket WebSocket — streams orderbook updates for a market.
//!
//! Polymarket CLOB WebSocket: wss://ws-subscriptions-clob.polymarket.com/ws/market
//!
//! Subscription message:
//!   {"type":"subscribe","channel":"market","markets":["<condition_id>"]}
//!
//! Events received include "price_change" with best bid/ask for YES/NO tokens.

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use crate::simulation::orderbook::OrderBook;

/// A parsed orderbook update from Polymarket WebSocket.
#[derive(Debug, Clone, Copy)]
pub struct BookEvent {
    pub book: OrderBook,
}

/// Parse a Polymarket WebSocket price_change message.
///
/// Expected shape (simplified):
/// ```json
/// {
///   "event_type": "price_change",
///   "market": "0xabc...",
///   "asset_id": "...",
///   "best_bid": "0.61",
///   "best_ask": "0.63"
///   "side": "YES"
/// }
/// ```
///
/// A full orderbook snapshot needs two messages (YES + NO sides).
/// For simplicity we build an `OrderBook` from a single message
/// and fill the opposite side with complement values.
pub fn parse_book_message(raw: &str) -> Option<BookEvent> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;

    // Accept both single object and array-of-one
    let msg = if v.is_array() {
        v.get(0)?.clone()
    } else {
        v
    };

    let event_type = msg["event_type"].as_str()?;
    if event_type != "price_change" && event_type != "book" { return None; }

    let best_bid = parse_price(&msg, "best_bid")?;
    let best_ask = parse_price(&msg, "best_ask")?;
    let side     = msg["side"].as_str().unwrap_or("YES");

    let book = if side.eq_ignore_ascii_case("YES") {
        OrderBook {
            best_bid_yes: best_bid,
            best_ask_yes: best_ask,
            best_bid_no:  (1.0 - best_ask).max(0.0),
            best_ask_no:  (1.0 - best_bid).max(0.0),
        }
    } else {
        OrderBook {
            best_bid_no:  best_bid,
            best_ask_no:  best_ask,
            best_bid_yes: (1.0 - best_ask).max(0.0),
            best_ask_yes: (1.0 - best_bid).max(0.0),
        }
    };

    Some(BookEvent { book })
}

fn parse_price(v: &serde_json::Value, field: &str) -> Option<f64> {
    v[field].as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .or_else(|| v[field].as_f64())
}

/// Connect to Polymarket CLOB WebSocket and stream orderbook updates.
///
/// Reconnects up to `max_retries` times with exponential back-off.
pub async fn stream_orderbook<F>(
    condition_id: &str,
    max_retries:  u32,
    mut on_event: F,
) where
    F: FnMut(Result<BookEvent, String>),
{
    let url         = "wss://ws-subscriptions-clob.polymarket.com/ws/market";
    let subscribe   = serde_json::json!({
        "type": "subscribe",
        "channel": "market",
        "markets": [condition_id]
    }).to_string();

    let mut delay_secs = 1u64;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            tokio::time::sleep(tokio::time::Duration::from_secs(delay_secs)).await;
            delay_secs = (delay_secs * 2).min(30);
        }

        let ws = match connect_async(url).await {
            Ok((ws, _)) => ws,
            Err(e) => {
                on_event(Err(format!("connect failed: {e}")));
                continue;
            }
        };

        let (mut write, mut read) = ws.split();
        delay_secs = 1;

        if let Err(e) = write.send(Message::Text(subscribe.clone().into())).await {
            on_event(Err(format!("subscribe failed: {e}")));
            continue;
        }

        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(txt)) => {
                    match parse_book_message(&txt) {
                        Some(ev) => on_event(Ok(ev)),
                        None     => {} // ignore non-price messages (heartbeat, etc.)
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
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn yes_msg(bid: &str, ask: &str) -> String {
        format!(r#"{{"event_type":"price_change","market":"0xabc","side":"YES","best_bid":"{bid}","best_ask":"{ask}"}}"#)
    }

    fn no_msg(bid: &str, ask: &str) -> String {
        format!(r#"{{"event_type":"price_change","market":"0xabc","side":"NO","best_bid":"{bid}","best_ask":"{ask}"}}"#)
    }

    #[test]
    fn parse_yes_bid_ask() {
        let ev = parse_book_message(&yes_msg("0.60", "0.62")).unwrap();
        assert!((ev.book.best_bid_yes - 0.60).abs() < 1e-9);
        assert!((ev.book.best_ask_yes - 0.62).abs() < 1e-9);
    }

    #[test]
    fn parse_yes_fills_no_side_as_complement() {
        let ev = parse_book_message(&yes_msg("0.60", "0.62")).unwrap();
        // NO bid = 1 - YES ask = 0.38
        assert!((ev.book.best_bid_no - 0.38).abs() < 1e-9);
        // NO ask = 1 - YES bid = 0.40
        assert!((ev.book.best_ask_no - 0.40).abs() < 1e-9);
    }

    #[test]
    fn parse_no_bid_ask() {
        let ev = parse_book_message(&no_msg("0.38", "0.40")).unwrap();
        assert!((ev.book.best_bid_no - 0.38).abs() < 1e-9);
        assert!((ev.book.best_ask_no - 0.40).abs() < 1e-9);
    }

    #[test]
    fn parse_no_fills_yes_side_as_complement() {
        let ev = parse_book_message(&no_msg("0.38", "0.40")).unwrap();
        assert!((ev.book.best_bid_yes - 0.60).abs() < 1e-9);
        assert!((ev.book.best_ask_yes - 0.62).abs() < 1e-9);
    }

    #[test]
    fn unknown_event_type_returns_none() {
        let msg = r#"{"event_type":"heartbeat"}"#;
        assert!(parse_book_message(msg).is_none());
    }

    #[test]
    fn invalid_json_returns_none() {
        assert!(parse_book_message("bad json").is_none());
    }

    #[test]
    fn missing_price_fields_returns_none() {
        let msg = r#"{"event_type":"price_change","side":"YES"}"#;
        assert!(parse_book_message(msg).is_none());
    }

    #[test]
    fn array_wrapped_message_parsed() {
        let arr = format!(r#"[{}]"#, yes_msg("0.55", "0.57").trim_start_matches('[').trim_end_matches(']'));
        // The array wrapper should be unwrapped automatically
        let raw = format!(r#"[{{"event_type":"price_change","side":"YES","best_bid":"0.55","best_ask":"0.57"}}]"#);
        let ev  = parse_book_message(&raw).unwrap();
        assert!((ev.book.best_bid_yes - 0.55).abs() < 1e-9);
        let _ = arr;
    }

    #[test]
    fn prices_in_valid_range() {
        let ev = parse_book_message(&yes_msg("0.60", "0.62")).unwrap();
        assert!(ev.book.best_bid_yes >= 0.0 && ev.book.best_bid_yes <= 1.0);
        assert!(ev.book.best_ask_yes >= 0.0 && ev.book.best_ask_yes <= 1.0);
        assert!(ev.book.best_bid_no  >= 0.0 && ev.book.best_bid_no  <= 1.0);
        assert!(ev.book.best_ask_no  >= 0.0 && ev.book.best_ask_no  <= 1.0);
    }
}
