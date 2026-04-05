//! Binance REST feed — fetches OHLCV klines for any symbol/interval.
//!
//! Endpoint: GET https://api.binance.com/api/v3/klines
//!
//! Kline array format (index):
//!   0  open_time (ms)
//!   1  open
//!   2  high
//!   3  low
//!   4  close
//!   5  volume
//!   6  close_time (ms)
//!   ...

use crate::data::Candle;
use super::{CandleFeed, FeedError};

// ─────────────────────────────────────────────────────────────────────────────
// Config
// ─────────────────────────────────────────────────────────────────────────────

/// Common Binance kline intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interval {
    M1,
    M5,
    M15,
    H1,
}

impl Interval {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::M1  => "1m",
            Self::M5  => "5m",
            Self::M15 => "15m",
            Self::H1  => "1h",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BinanceFeed
// ─────────────────────────────────────────────────────────────────────────────

pub struct BinanceFeed {
    pub symbol:   String,
    pub interval: Interval,
    base_url:     String,
}

impl BinanceFeed {
    pub fn new(symbol: &str, interval: Interval) -> Self {
        Self {
            symbol:   symbol.to_uppercase(),
            interval,
            base_url: "https://api.binance.com".to_string(),
        }
    }

    /// Override base URL (for tests or alternative endpoints).
    #[cfg(test)]
    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = url.to_string();
        self
    }

    /// Parse raw JSON string into candles.
    /// Public so tests can verify parsing without network.
    pub fn parse(raw: &str) -> Result<Vec<Candle>, FeedError> {
        let value: serde_json::Value = serde_json::from_str(raw)
            .map_err(|e| FeedError::Parse(e.to_string()))?;

        let arr = value.as_array()
            .ok_or_else(|| FeedError::Parse("expected JSON array".into()))?;

        if arr.is_empty() { return Err(FeedError::Empty); }

        arr.iter().map(|row| {
            let row = row.as_array()
                .ok_or_else(|| FeedError::Parse("kline row not array".into()))?;

            let open_time_ms = row.get(0)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| FeedError::Parse("open_time missing".into()))?;

            let parse_f64 = |idx: usize, name: &str| -> Result<f64, FeedError> {
                row.get(idx)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| FeedError::Parse(format!("{name} missing")))?
                    .parse::<f64>()
                    .map_err(|e| FeedError::Parse(format!("{name}: {e}")))
            };

            Ok(Candle::new(
                parse_f64(1, "open")?,
                parse_f64(2, "high")?,
                parse_f64(3, "low")?,
                parse_f64(4, "close")?,
                open_time_ms / 1000,   // convert ms → seconds
            ))
        }).collect()
    }

    fn url(&self, limit: u32) -> String {
        format!(
            "{}/api/v3/klines?symbol={}&interval={}&limit={}",
            self.base_url,
            self.symbol,
            self.interval.as_str(),
            limit,
        )
    }
}

impl CandleFeed for BinanceFeed {
    fn fetch_candles(&self, limit: u32) -> Result<Vec<Candle>, FeedError> {
        let url = self.url(limit);
        let body = reqwest::blocking::get(&url)
            .map_err(|e| FeedError::Http(e.to_string()))?
            .text()
            .map_err(|e| FeedError::Http(e.to_string()))?;
        Self::parse(&body)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests — parse only (no network)
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal Binance klines JSON with 2 bars.
    const SAMPLE: &str = r#"[
      [1700000000000,"29500.00","29600.00","29400.00","29550.00","10.5",
       1700000059999,"309000","120","5.0","147500","0"],
      [1700000060000,"29550.00","29700.00","29450.00","29650.00","12.3",
       1700000119999,"364500","135","6.0","177600","0"]
    ]"#;

    #[test]
    fn parse_two_candles() {
        let candles = BinanceFeed::parse(SAMPLE).unwrap();
        assert_eq!(candles.len(), 2);
    }

    #[test]
    fn parse_open_price_correct() {
        let candles = BinanceFeed::parse(SAMPLE).unwrap();
        assert!((candles[0].open - 29_500.0).abs() < 1e-6);
    }

    #[test]
    fn parse_high_correct() {
        let candles = BinanceFeed::parse(SAMPLE).unwrap();
        assert!((candles[0].high - 29_600.0).abs() < 1e-6);
    }

    #[test]
    fn parse_low_correct() {
        let candles = BinanceFeed::parse(SAMPLE).unwrap();
        assert!((candles[0].low - 29_400.0).abs() < 1e-6);
    }

    #[test]
    fn parse_close_correct() {
        let candles = BinanceFeed::parse(SAMPLE).unwrap();
        assert!((candles[0].close - 29_550.0).abs() < 1e-6);
    }

    #[test]
    fn parse_timestamp_converted_to_seconds() {
        let candles = BinanceFeed::parse(SAMPLE).unwrap();
        // 1700000000000 ms → 1700000000 s
        assert_eq!(candles[0].timestamp, 1_700_000_000);
    }

    #[test]
    fn parse_second_candle_values() {
        let candles = BinanceFeed::parse(SAMPLE).unwrap();
        assert!((candles[1].open  - 29_550.0).abs() < 1e-6);
        assert!((candles[1].close - 29_650.0).abs() < 1e-6);
    }

    #[test]
    fn parse_empty_array_returns_error() {
        let result = BinanceFeed::parse("[]");
        assert!(matches!(result, Err(FeedError::Empty)));
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        let result = BinanceFeed::parse("not json");
        assert!(matches!(result, Err(FeedError::Parse(_))));
    }

    #[test]
    fn url_contains_symbol_and_interval() {
        let feed = BinanceFeed::new("btcusdt", Interval::M5);
        let url  = feed.url(100);
        assert!(url.contains("BTCUSDT"));
        assert!(url.contains("5m"));
        assert!(url.contains("limit=100"));
    }

    #[test]
    fn candle_passes_validation() {
        let candles = BinanceFeed::parse(SAMPLE).unwrap();
        for c in candles {
            assert!(c.is_valid(), "candle invalid: {c:?}");
        }
    }
}
