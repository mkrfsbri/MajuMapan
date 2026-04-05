//! Polymarket Gamma API feed — fetches YES/NO prices and auto-discovers markets.
//!
//! Endpoints used:
//!   GET /markets?conditionIds={id}        — price for known market
//!   GET /markets?active=true&q={query}&limit={n}  — search / discovery

use super::{MarketFeed, FeedError};

// ─────────────────────────────────────────────────────────────────────────────
// MarketPrice — current snapshot for one market
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct MarketPrice {
    pub yes_price: f64,
    pub no_price:  f64,
    pub volume:    f64,
    pub active:    bool,
}

impl MarketPrice {
    pub fn mid(&self)    -> f64 { self.yes_price }
    pub fn spread(&self) -> f64 { (self.yes_price + self.no_price - 1.0).abs() }
}

// ─────────────────────────────────────────────────────────────────────────────
// DiscoveredMarket — result of a search query
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DiscoveredMarket {
    pub condition_id: String,
    pub question:     String,
    pub yes_price:    f64,
    pub no_price:     f64,
    pub volume:       f64,
    pub active:       bool,
    /// Unix seconds of market end date (0 if unknown)
    pub end_ts:       u64,
}

impl DiscoveredMarket {
    /// Seconds until market expires from `now_ts` (None if already expired or unknown).
    pub fn secs_remaining(&self, now_ts: u64) -> Option<u64> {
        if self.end_ts == 0 || self.end_ts <= now_ts { None }
        else { Some(self.end_ts - now_ts) }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Timeframe pairing result
// ─────────────────────────────────────────────────────────────────────────────

/// Condition IDs selected for each signal timeframe.
#[derive(Debug, Clone, Default)]
pub struct TimeframePair {
    /// Market to use for 5m signal (shorter horizon, higher volume)
    pub tf5m_id:  Option<String>,
    /// Market to use for 15m signal (medium horizon, next by volume)
    pub tf15m_id: Option<String>,
}

impl TimeframePair {
    pub fn is_complete(&self) -> bool {
        self.tf5m_id.is_some() && self.tf15m_id.is_some()
    }
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

    // ── parse helpers ─────────────────────────────────────────────────────────

    /// Parse Gamma API response → MarketPrice for a single market.
    pub fn parse(raw: &str) -> Result<MarketPrice, FeedError> {
        let arr: serde_json::Value = serde_json::from_str(raw)
            .map_err(|e| FeedError::Parse(e.to_string()))?;
        let market = arr.get(0).ok_or(FeedError::Empty)?;
        Self::parse_market_price(market)
    }

    fn parse_market_price(m: &serde_json::Value) -> Result<MarketPrice, FeedError> {
        let prices = m["outcomePrices"].as_array()
            .ok_or_else(|| FeedError::Parse("outcomePrices missing".into()))?;

        let pf = |idx: usize, name: &str| -> Result<f64, FeedError> {
            prices.get(idx)
                .and_then(|v| v.as_str())
                .ok_or_else(|| FeedError::Parse(format!("{name} missing")))?
                .parse::<f64>()
                .map_err(|e| FeedError::Parse(format!("{name}: {e}")))
        };

        Ok(MarketPrice {
            yes_price: pf(0, "yes_price")?,
            no_price:  pf(1, "no_price")?,
            volume:    m["volume"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
            active:    m["active"].as_bool().unwrap_or(false),
        })
    }

    /// Parse Gamma API search response → Vec<DiscoveredMarket>.
    pub fn parse_discovery(raw: &str) -> Result<Vec<DiscoveredMarket>, FeedError> {
        let arr: serde_json::Value = serde_json::from_str(raw)
            .map_err(|e| FeedError::Parse(e.to_string()))?;

        let items = arr.as_array().ok_or(FeedError::Empty)?;
        if items.is_empty() { return Err(FeedError::Empty); }

        let mut markets = Vec::new();
        for m in items {
            let condition_id = m["conditionId"].as_str()
                .unwrap_or("").to_string();
            if condition_id.is_empty() { continue; }

            let question = m["question"].as_str().unwrap_or("").to_string();
            let active   = m["active"].as_bool().unwrap_or(false);
            let volume   = m["volume"].as_str()
                .and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);

            // Parse outcomePrices if available
            let (yes_price, no_price) = if let Some(prices) = m["outcomePrices"].as_array() {
                let y = prices.get(0).and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.5);
                let n = prices.get(1).and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.5);
                (y, n)
            } else {
                (0.5, 0.5)
            };

            // endDate as unix seconds (Gamma returns ISO-8601 string)
            let end_ts = Self::parse_end_ts(m["endDate"].as_str().unwrap_or(""));

            markets.push(DiscoveredMarket {
                condition_id, question, yes_price, no_price, volume, active, end_ts,
            });
        }

        if markets.is_empty() { Err(FeedError::Empty) } else { Ok(markets) }
    }

    /// Parse ISO-8601 date string like "2024-01-15T00:00:00Z" → unix seconds.
    /// Returns 0 on any parse failure (treated as unknown).
    pub fn parse_end_ts(s: &str) -> u64 {
        if s.is_empty() { return 0; }
        // Manual parse: "YYYY-MM-DDTHH:MM:SSZ" or "YYYY-MM-DD"
        let digits: String = s.chars()
            .filter(|c| c.is_ascii_digit() || *c == '-' || *c == 'T' || *c == ':')
            .collect();

        // Split on common delimiters
        let parts: Vec<u64> = digits
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<u64>().ok())
            .collect();

        if parts.len() < 3 { return 0; }
        let (y, mo, d) = (parts[0], parts[1], parts[2]);
        let (h, mi, sc) = (
            parts.get(3).copied().unwrap_or(0),
            parts.get(4).copied().unwrap_or(0),
            parts.get(5).copied().unwrap_or(0),
        );

        // Rough conversion (no timezone adjustments — good enough for ordering)
        let days_from_epoch = Self::days_since_epoch(y, mo, d);
        days_from_epoch * 86_400 + h * 3_600 + mi * 60 + sc
    }

    fn days_since_epoch(y: u64, m: u64, d: u64) -> u64 {
        // Simplified Gregorian → days since 1970-01-01
        let y = y as i64; let m = m as i64; let d = d as i64;
        let a = (14 - m) / 12;
        let yr = y + 4800 - a;
        let mo = m + 12 * a - 3;
        let jdn = d + (153 * mo + 2) / 5 + 365 * yr + yr / 4 - yr / 100 + yr / 400 - 32045;
        (jdn - 2_440_588).max(0) as u64  // 2440588 = JDN of 1970-01-01
    }

    // ── selection logic ───────────────────────────────────────────────────────

    /// Select the best two active BTC markets for 5m and 15m signals.
    ///
    /// Strategy:
    ///   1. Filter: active=true, question contains BTC/bitcoin keyword
    ///   2. Sort by volume descending (most liquid = best price signal)
    ///   3. tf5m  → highest volume market
    ///   4. tf15m → second highest volume market (or same if only one)
    pub fn select_for_timeframes(markets: &[DiscoveredMarket]) -> TimeframePair {
        let mut active: Vec<&DiscoveredMarket> = markets.iter()
            .filter(|m| m.active && Self::is_btc_market(m))
            .collect();

        // Sort by volume descending
        active.sort_by(|a, b| b.volume.partial_cmp(&a.volume).unwrap_or(std::cmp::Ordering::Equal));

        TimeframePair {
            tf5m_id:  active.get(0).map(|m| m.condition_id.clone()),
            tf15m_id: active.get(1)
                .or_else(|| active.get(0))   // fallback to same if only one found
                .map(|m| m.condition_id.clone()),
        }
    }

    fn is_btc_market(m: &DiscoveredMarket) -> bool {
        let q = m.question.to_lowercase();
        q.contains("btc") || q.contains("bitcoin")
    }

    // ── URL builders ──────────────────────────────────────────────────────────

    fn url(&self, condition_id: &str) -> String {
        format!("{}/markets?conditionIds={}", self.base_url, condition_id)
    }

    fn discovery_url(&self, query: &str, limit: u32) -> String {
        format!("{}/markets?active=true&q={}&limit={}", self.base_url, query, limit)
    }

    // ── public fetch methods ──────────────────────────────────────────────────

    /// Search Polymarket for active markets matching `query`.
    pub fn discover_markets(&self, query: &str, limit: u32) -> Result<Vec<DiscoveredMarket>, FeedError> {
        let url  = self.discovery_url(query, limit);
        let body = reqwest::blocking::get(&url)
            .map_err(|e| FeedError::Http(e.to_string()))?
            .text()
            .map_err(|e| FeedError::Http(e.to_string()))?;
        Self::parse_discovery(&body)
    }

    /// Discover BTC markets and auto-select condition IDs for 5m/15m.
    pub fn auto_select_btc(&self, limit: u32) -> Result<TimeframePair, FeedError> {
        let markets = self.discover_markets("bitcoin", limit)?;
        let pair = Self::select_for_timeframes(&markets);
        if pair.tf5m_id.is_none() {
            return Err(FeedError::Parse("no active BTC markets found".into()));
        }
        Ok(pair)
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
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

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

    const DISCOVERY_SAMPLE: &str = r#"[
      {
        "conditionId": "0xaaa",
        "question": "Will Bitcoin price exceed $40k this week?",
        "outcomePrices": ["0.70", "0.30"],
        "volume": "500000.00",
        "endDate": "2024-01-20T00:00:00Z",
        "active": true
      },
      {
        "conditionId": "0xbbb",
        "question": "BTC above $38k by end of month?",
        "outcomePrices": ["0.55", "0.45"],
        "volume": "250000.00",
        "endDate": "2024-01-31T00:00:00Z",
        "active": true
      },
      {
        "conditionId": "0xccc",
        "question": "Will ETH flip BTC in 2024?",
        "outcomePrices": ["0.10", "0.90"],
        "volume": "80000.00",
        "endDate": "2024-12-31T00:00:00Z",
        "active": true
      },
      {
        "conditionId": "0xddd",
        "question": "Will Bitcoin reach $50k?",
        "outcomePrices": ["0.40", "0.60"],
        "volume": "100000.00",
        "endDate": "2024-03-01T00:00:00Z",
        "active": false
      }
    ]"#;

    // ── MarketPrice parse ─────────────────────────────────────────────────────

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
        assert!(PolymarketFeed::parse(SAMPLE).unwrap().active);
    }

    #[test]
    fn parse_active_flag_false() {
        assert!(!PolymarketFeed::parse(SAMPLE_INACTIVE).unwrap().active);
    }

    #[test]
    fn prices_sum_to_one_approx() {
        let mp = PolymarketFeed::parse(SAMPLE).unwrap();
        assert!((mp.yes_price + mp.no_price - 1.0).abs() < 1e-9);
    }

    #[test]
    fn spread_near_zero() {
        let mp = PolymarketFeed::parse(SAMPLE).unwrap();
        assert!(mp.spread() < 0.01);
    }

    #[test]
    fn mid_equals_yes_price() {
        let mp = PolymarketFeed::parse(SAMPLE).unwrap();
        assert!((mp.mid() - mp.yes_price).abs() < 1e-9);
    }

    #[test]
    fn parse_empty_returns_error() {
        assert!(matches!(PolymarketFeed::parse("[]"), Err(FeedError::Empty)));
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        assert!(matches!(PolymarketFeed::parse("bad{"), Err(FeedError::Parse(_))));
    }

    #[test]
    fn url_contains_condition_id() {
        let feed = PolymarketFeed::new();
        let url  = feed.url("0xabc123");
        assert!(url.contains("0xabc123") && url.contains("conditionIds"));
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

    // ── Discovery parse ───────────────────────────────────────────────────────

    #[test]
    fn parse_discovery_returns_all_items() {
        let markets = PolymarketFeed::parse_discovery(DISCOVERY_SAMPLE).unwrap();
        assert_eq!(markets.len(), 4);
    }

    #[test]
    fn parse_discovery_condition_id_correct() {
        let markets = PolymarketFeed::parse_discovery(DISCOVERY_SAMPLE).unwrap();
        assert_eq!(markets[0].condition_id, "0xaaa");
        assert_eq!(markets[1].condition_id, "0xbbb");
    }

    #[test]
    fn parse_discovery_question_correct() {
        let markets = PolymarketFeed::parse_discovery(DISCOVERY_SAMPLE).unwrap();
        assert!(markets[0].question.contains("Bitcoin"));
    }

    #[test]
    fn parse_discovery_volume_correct() {
        let markets = PolymarketFeed::parse_discovery(DISCOVERY_SAMPLE).unwrap();
        assert!((markets[0].volume - 500_000.0).abs() < 1.0);
    }

    #[test]
    fn parse_discovery_active_flag_preserved() {
        let markets = PolymarketFeed::parse_discovery(DISCOVERY_SAMPLE).unwrap();
        assert!(markets[0].active);
        assert!(!markets[3].active);
    }

    #[test]
    fn parse_discovery_prices_parsed() {
        let markets = PolymarketFeed::parse_discovery(DISCOVERY_SAMPLE).unwrap();
        assert!((markets[0].yes_price - 0.70).abs() < 1e-9);
        assert!((markets[0].no_price  - 0.30).abs() < 1e-9);
    }

    #[test]
    fn parse_discovery_empty_returns_error() {
        assert!(matches!(PolymarketFeed::parse_discovery("[]"), Err(FeedError::Empty)));
    }

    // ── select_for_timeframes ─────────────────────────────────────────────────

    #[test]
    fn select_picks_two_active_btc_markets() {
        let markets = PolymarketFeed::parse_discovery(DISCOVERY_SAMPLE).unwrap();
        let pair = PolymarketFeed::select_for_timeframes(&markets);
        // 0xaaa (vol=500k) and 0xbbb (vol=250k) are active BTC markets
        assert_eq!(pair.tf5m_id.as_deref(),  Some("0xaaa"));
        assert_eq!(pair.tf15m_id.as_deref(), Some("0xbbb"));
    }

    #[test]
    fn select_excludes_inactive_markets() {
        let markets = PolymarketFeed::parse_discovery(DISCOVERY_SAMPLE).unwrap();
        let pair = PolymarketFeed::select_for_timeframes(&markets);
        // 0xddd is inactive — must not appear
        assert_ne!(pair.tf5m_id.as_deref(),  Some("0xddd"));
        assert_ne!(pair.tf15m_id.as_deref(), Some("0xddd"));
    }

    #[test]
    fn select_excludes_non_btc_markets() {
        let markets = PolymarketFeed::parse_discovery(DISCOVERY_SAMPLE).unwrap();
        let pair = PolymarketFeed::select_for_timeframes(&markets);
        // 0xccc is ETH flip — must not appear
        assert_ne!(pair.tf5m_id.as_deref(),  Some("0xccc"));
        assert_ne!(pair.tf15m_id.as_deref(), Some("0xccc"));
    }

    #[test]
    fn select_returns_none_when_no_btc_markets() {
        let markets = vec![DiscoveredMarket {
            condition_id: "0x1".into(),
            question:     "Will Doge moon?".into(),
            yes_price: 0.5, no_price: 0.5, volume: 1000.0,
            active: true, end_ts: 9_999_999_999,
        }];
        let pair = PolymarketFeed::select_for_timeframes(&markets);
        assert!(pair.tf5m_id.is_none());
    }

    #[test]
    fn select_fallback_same_id_when_only_one_btc_market() {
        let markets = vec![DiscoveredMarket {
            condition_id: "0xonly".into(),
            question:     "Will Bitcoin reach $100k?".into(),
            yes_price: 0.3, no_price: 0.7, volume: 50_000.0,
            active: true, end_ts: 9_999_999_999,
        }];
        let pair = PolymarketFeed::select_for_timeframes(&markets);
        assert_eq!(pair.tf5m_id,  pair.tf15m_id);
    }

    #[test]
    fn pair_is_complete_with_two_markets() {
        let markets = PolymarketFeed::parse_discovery(DISCOVERY_SAMPLE).unwrap();
        let pair = PolymarketFeed::select_for_timeframes(&markets);
        assert!(pair.is_complete());
    }

    // ── parse_end_ts ──────────────────────────────────────────────────────────

    #[test]
    fn parse_end_ts_iso_string() {
        let ts = PolymarketFeed::parse_end_ts("2024-01-20T00:00:00Z");
        assert!(ts > 0, "ts={ts}");
    }

    #[test]
    fn parse_end_ts_empty_returns_zero() {
        assert_eq!(PolymarketFeed::parse_end_ts(""), 0);
    }

    #[test]
    fn parse_end_ts_ordering_preserved() {
        let t1 = PolymarketFeed::parse_end_ts("2024-01-20T00:00:00Z");
        let t2 = PolymarketFeed::parse_end_ts("2024-01-31T00:00:00Z");
        assert!(t1 < t2, "t1={t1} t2={t2}");
    }

    // ── discovery URL ─────────────────────────────────────────────────────────

    #[test]
    fn discovery_url_contains_query_and_limit() {
        let feed = PolymarketFeed::new();
        let url  = feed.discovery_url("bitcoin", 10);
        assert!(url.contains("bitcoin") && url.contains("limit=10") && url.contains("active=true"));
    }
}
