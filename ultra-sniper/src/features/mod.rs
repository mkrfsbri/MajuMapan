pub mod ema;
pub mod rsi;
pub mod atr;

pub use ema::Ema;
pub use rsi::Rsi;
pub use atr::Atr;

/// Snapshot of computed indicator values for one bar.
#[derive(Debug, Clone, Copy)]
pub struct IndicatorState {
    pub ema9:  f64,
    pub ema21: f64,
    pub rsi14: Option<f64>,
    pub atr14: Option<f64>,
}
