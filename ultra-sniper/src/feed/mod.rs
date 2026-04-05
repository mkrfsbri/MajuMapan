pub mod binance;
pub mod polymarket;

pub use binance::BinanceFeed;
pub use polymarket::{PolymarketFeed, MarketPrice};

use crate::data::Candle;

// ─────────────────────────────────────────────────────────────────────────────
// Error
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum FeedError {
    Http(String),
    Parse(String),
    Empty,
}

impl std::fmt::Display for FeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(e)   => write!(f, "HTTP error: {e}"),
            Self::Parse(e)  => write!(f, "Parse error: {e}"),
            Self::Empty     => write!(f, "Empty response"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Traits — testable via mock implementations
// ─────────────────────────────────────────────────────────────────────────────

/// Anything that can supply OHLC candles.
pub trait CandleFeed {
    fn fetch_candles(&self, limit: u32) -> Result<Vec<Candle>, FeedError>;
    fn fetch_latest(&self)              -> Result<Candle,      FeedError> {
        self.fetch_candles(1)?
            .into_iter()
            .next()
            .ok_or(FeedError::Empty)
    }
}

/// Anything that can supply a binary market price.
pub trait MarketFeed {
    fn fetch_price(&self, market_id: &str) -> Result<MarketPrice, FeedError>;
}
