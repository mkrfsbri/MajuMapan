//! Polymarket Gamma API feed — fetches YES/NO prices for a binary market.
//!
//! Endpoint: GET https://gamma-api.polymarket.com/markets?conditionIds={id}
//!
//! Relevant response fields per market token:
//!   outcomePrices  — JSON array ["yes_price", "no_price"]  (strings)
//!   volume         — total volume
//!   active         — bool

use super::{MarketFeed, FeedError};

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// Current binary market price snapshot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketPrice {
    /// YES token price [0.0, 1.0]
    pub yes_price: f64,
    /// NO  token price [0.0, 1.0]  (≈ 1 - yes_price)
    pub no_price:  f64,
    /// Reported 24 h volume in USD
    pub volume:    f64,
    /// Whether the market is still active
    pub active:    bool,
}

impl MarketPrice {
    /// Mid-point price of the YES token.
    pub fn mid(&self) -> f64 { self.yes_price }

    /// Spread between YES and NO (should ≈ 0 in efficient markets).
    pub fn spread(&self) -> f64 { (self.yes_price + self.no_price - 1.0).abs() }
}

// ─────────────────────────────────────────────────────────────────────────────
// PolymarketFeed
// ─────────────────────────────────────────────────────────────────────────────

pub struct PolymarketFeed {
    base_url: String,
}

impl PolymarketFeed {
    pub fn new() -> Self {
        Self { base_url: "https://gamma-api.polymarket.com".to_string() }
    }

    #[cfg(test)]
    pub fn with_base_url(base_url: &str) -> Self {
        Self { base_url: base_url.to_string() }
    }

    /// Parse Gamma API JSON response for a single market.
    /// Public for unit-testing parse logic without network.
    pub fn parse(raw: &str) -> Result<MarketPrice, FeedError> {
        let arr: serde_json::Value = serde_json::from_str(raw)
            .map_err(|e| FeedError::Parse(e.to_string()))?;

        // Gamma returns an array; take the first element
        let market = arr.get(0)
            .ok_or(FeedError::Empty)?;

        // outcomePrices is ["<yes>", "<no>"]
        let prices = market["outcomePrices"]
            .as_array()
            .ok_or_else(|| FeedError::Parse("outcomePrices missing".into()))?;

        let parse_price = |idx: usize, name: &str| -> Result<f64, FeedError> {
            prices.get(idx)
                .and_then(|v| v.as_str())
                .ok_or_else(|| FeedError::Parse(format!("{name} missing")))?
                .parse::<f64>()
                .map_err(|e| FeedError::Parse(format!("{name}: {e}")))
        };

        let yes_price = parse_price(0, "yes_price")?;
        let no_price  = parse_price(1, "no_price")?;

        let volume = market["volume"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        let active = market["active"].as_bool().unwrap_or(false);

        Ok(MarketPrice { yes_price, no_price, volume, active })
    }

    fn url(&self, condition_id: &str) -> String {
        format!("{}/markets?conditionIds={}", self.base_url, condition_id)
    }
}

impl Default for PolymarketFeed {
    fn default() -> Self { Self::new() }
}

impl MarketFeed for PolymarketFeed {
    fn fetch_price(&self, condition_id: &str) -> Result<MarketPrice, FeedError> {
        let url  = self.url(condition_id);
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

    /// Minimal Gamma API response for one market.
    const SAMPLE: &str = r#"[
      {
        "conditionId": "0xabc123",
        "question": "Will BTC exceed $35k by end of month?",
        "outcomePrices": ["0.62", "0.38"],
        "volume": "125000.50",
        "active": true
      }
    ]"#;

    const SAMPLE_INACTIVE: &str = r#"[
      {
        "conditionId": "0xdef456",
        "question": "Closed market",
        "outcomePrices": ["1.00", "0.00"],
        "volume": "5000.00",
        "active": false
      }
    ]"#;

    #[test]
    fn parse_yes_price() {
        let mp = PolymarketFeed::parse(SAMPLE).unwrap();
        assert!((mp.yes_price - 0.62).abs() < 1e-9);
    }

    #[test]
    fn parse_no_price() {
        let mp = PolymarketFeed::parse(SAMPLE).unwrap();
        assert!((mp.no_price - 0.38).abs() < 1e-9);
    }

    #[test]
    fn parse_volume() {
        let mp = PolymarketFeed::parse(SAMPLE).unwrap();
        assert!((mp.volume - 125_000.50).abs() < 1e-3);
    }

    #[test]
    fn parse_active_flag_true() {
        let mp = PolymarketFeed::parse(SAMPLE).unwrap();
        assert!(mp.active);
    }

    #[test]
    fn parse_active_flag_false() {
        let mp = PolymarketFeed::parse(SAMPLE_INACTIVE).unwrap();
        assert!(!mp.active);
    }

    #[test]
    fn prices_sum_to_one_approx() {
        let mp = PolymarketFeed::parse(SAMPLE).unwrap();
        assert!((mp.yes_price + mp.no_price - 1.0).abs() < 1e-9);
    }

    #[test]
    fn spread_near_zero() {
        let mp = PolymarketFeed::parse(SAMPLE).unwrap();
        assert!(mp.spread() < 0.01, "spread={}", mp.spread());
    }

    #[test]
    fn mid_equals_yes_price() {
        let mp = PolymarketFeed::parse(SAMPLE).unwrap();
        assert!((mp.mid() - mp.yes_price).abs() < 1e-9);
    }

    #[test]
    fn parse_empty_array_returns_empty_error() {
        let result = PolymarketFeed::parse("[]");
        assert!(matches!(result, Err(FeedError::Empty)));
    }

    #[test]
    fn parse_invalid_json_returns_parse_error() {
        let result = PolymarketFeed::parse("not json {");
        assert!(matches!(result, Err(FeedError::Parse(_))));
    }

    #[test]
    fn url_contains_condition_id() {
        let feed = PolymarketFeed::new();
        let url  = feed.url("0xabc123");
        assert!(url.contains("0xabc123"));
        assert!(url.contains("conditionIds"));
    }

    #[test]
    fn yes_price_in_range() {
        let mp = PolymarketFeed::parse(SAMPLE).unwrap();
        assert!(mp.yes_price >= 0.0 && mp.yes_price <= 1.0);
    }

    #[test]
    fn no_price_in_range() {
        let mp = PolymarketFeed::parse(SAMPLE).unwrap();
        assert!(mp.no_price >= 0.0 && mp.no_price <= 1.0);
    }
}
