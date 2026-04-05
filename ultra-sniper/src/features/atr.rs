use crate::data::Candle;

/// Wilder's Average True Range — O(1) per update.
pub struct Atr {
    period:    usize,
    avg_tr:    f64,
    prev_close: f64,
    count:     usize,
}

impl Atr {
    pub fn new(period: usize) -> Self {
        assert!(period >= 1);
        Self { period, avg_tr: 0.0, prev_close: 0.0, count: 0 }
    }

    /// Feed one candle. Returns Some(atr) once warmed up.
    pub fn update(&mut self, candle: &Candle) -> Option<f64> {
        let tr = if self.count == 0 {
            candle.range() // no previous close on first bar
        } else {
            let hl = candle.high - candle.low;
            let hc = (candle.high  - self.prev_close).abs();
            let lc = (candle.low   - self.prev_close).abs();
            hl.max(hc).max(lc)
        };

        self.prev_close = candle.close;
        self.count += 1;

        if self.count <= self.period {
            self.avg_tr += tr;
            if self.count == self.period {
                self.avg_tr /= self.period as f64;
                return Some(self.avg_tr);
            }
            return None;
        }

        let p = self.period as f64;
        self.avg_tr = (self.avg_tr * (p - 1.0) + tr) / p;
        Some(self.avg_tr)
    }

    pub fn value(&self) -> f64  { self.avg_tr }
    pub fn is_ready(&self) -> bool { self.count >= self.period }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn candle(high: f64, low: f64, close: f64) -> Candle {
        Candle::new(low, high, low, close, 0)
    }

    #[test]
    fn atr_none_before_period() {
        let mut atr = Atr::new(14);
        for _ in 0..13 {
            assert!(atr.update(&candle(105.0, 95.0, 100.0)).is_none());
        }
    }

    #[test]
    fn atr_some_after_period() {
        let mut atr = Atr::new(14);
        let mut v = None;
        for i in 0..14 {
            v = atr.update(&candle(100.0 + i as f64, 90.0 + i as f64, 95.0 + i as f64));
        }
        assert!(v.is_some());
        assert!(v.unwrap() > 0.0);
    }

    #[test]
    fn atr_constant_range_equals_range() {
        // When every candle has identical range and no overnight gaps, ATR == range.
        let mut atr = Atr::new(3);
        let c = candle(110.0, 90.0, 100.0); // range = 20
        atr.update(&c); atr.update(&c);
        let v = atr.update(&c).unwrap();
        // TR[0]=20, TR[1]=max(20,|110-100|,|90-100|)=20, TR[2]=20 → avg=20
        assert!((v - 20.0).abs() < 1e-6, "ATR={v}");
    }

    #[test]
    fn atr_non_negative() {
        let mut atr = Atr::new(5);
        let candles = [
            candle(110.0, 95.0, 100.0),
            candle(108.0, 92.0, 105.0),
            candle(115.0, 98.0, 110.0),
            candle(112.0, 100.0, 107.0),
            candle(118.0, 105.0, 115.0),
        ];
        let mut last = None;
        for c in &candles { last = atr.update(c); }
        assert!(last.unwrap() >= 0.0);
    }
}
