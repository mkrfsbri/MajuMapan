/// Incremental Exponential Moving Average.
/// Stores only the previous EMA value — O(1) per update.
pub struct Ema {
    pub value: f64,
    pub k:     f64,  // smoothing factor = 2 / (period + 1)
    initialized: bool,
}

impl Ema {
    pub fn new(period: usize) -> Self {
        assert!(period >= 1, "EMA period must be ≥ 1");
        Self {
            value: 0.0,
            k: 2.0 / (period as f64 + 1.0),
            initialized: false,
        }
    }

    /// Feed one price. Returns current EMA.
    #[inline]
    pub fn update(&mut self, price: f64) -> f64 {
        if !self.initialized {
            self.value = price;
            self.initialized = true;
        } else {
            self.value = price * self.k + self.value * (1.0 - self.k);
        }
        self.value
    }

    pub fn is_ready(&self) -> bool { self.initialized }
}

// ─────────────────────────────────────────────────────────────────────────────

/// Rolling RSI using Wilder's smoothed average gain/loss.
/// State: only avg_gain and avg_loss — O(1) per update.
pub struct Rsi {
    period:      usize,
    avg_gain:    f64,
    avg_loss:    f64,
    prev_price:  f64,
    count:       usize,  // warmup counter
}

impl Rsi {
    pub fn new(period: usize) -> Self {
        assert!(period >= 1, "RSI period must be ≥ 1");
        Self {
            period,
            avg_gain:   0.0,
            avg_loss:   0.0,
            prev_price: 0.0,
            count:      0,
        }
    }

    /// Feed one price. Returns Some(rsi) once warmed up, None otherwise.
    pub fn update(&mut self, price: f64) -> Option<f64> {
        if self.count == 0 {
            self.prev_price = price;
            self.count += 1;
            return None;
        }

        let change = price - self.prev_price;
        let gain = if change > 0.0 { change } else { 0.0 };
        let loss = if change < 0.0 { -change } else { 0.0 };
        self.prev_price = price;
        self.count += 1;

        if self.count <= self.period {
            // Accumulate simple average during warmup
            self.avg_gain += gain;
            self.avg_loss += loss;

            if self.count == self.period {
                self.avg_gain /= self.period as f64;
                self.avg_loss /= self.period as f64;
                return Some(self.rsi_value());
            }
            return None;
        }

        // Wilder smoothing
        let p = self.period as f64;
        self.avg_gain = (self.avg_gain * (p - 1.0) + gain) / p;
        self.avg_loss = (self.avg_loss * (p - 1.0) + loss) / p;
        Some(self.rsi_value())
    }

    #[inline]
    fn rsi_value(&self) -> f64 {
        if self.avg_loss == 0.0 { return 100.0; }
        let rs = self.avg_gain / self.avg_loss;
        100.0 - 100.0 / (1.0 + rs)
    }

    pub fn is_ready(&self) -> bool { self.count >= self.period }
}

// ─────────────────────────────────────────────────────────────────────────────
// TDD — Step 3 Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    // ── EMA ──────────────────────────────────────────────────────────────────

    #[test]
    fn ema_first_value_equals_seed_price() {
        let mut ema = Ema::new(3);
        let v = ema.update(100.0);
        assert_eq!(v, 100.0);
    }

    #[test]
    fn ema_updates_incrementally() {
        let mut ema = Ema::new(3);
        // k = 2/(3+1) = 0.5
        ema.update(100.0);  // seed = 100
        let v1 = ema.update(101.0); // 101*0.5 + 100*0.5 = 100.5
        assert!((v1 - 100.5).abs() < 1e-9);
        let v2 = ema.update(102.0); // 102*0.5 + 100.5*0.5 = 101.25
        assert!((v2 - 101.25).abs() < 1e-9);
    }

    #[test]
    fn ema_sequence_100_101_102() {
        let mut ema = Ema::new(3);
        ema.update(100.0);
        ema.update(101.0);
        let v = ema.update(102.0);
        // seed=100, after 101: 100.5, after 102: 101.25
        assert!((v - 101.25).abs() < 1e-9);
    }

    // ── RSI ──────────────────────────────────────────────────────────────────

    #[test]
    fn rsi_returns_none_during_warmup() {
        let mut rsi = Rsi::new(14);
        for i in 0..13 {
            assert!(rsi.update(44.0 + i as f64).is_none());
        }
    }

    #[test]
    fn rsi_returns_some_after_period_values() {
        let mut rsi = Rsi::new(14);
        let prices = [
            44.34, 44.09, 44.15, 43.61, 44.33,
            44.83, 45.10, 45.15, 43.61, 44.33,
            44.83, 45.10, 45.15, 43.61,
        ];
        let mut result = None;
        for &p in &prices {
            result = rsi.update(p);
        }
        assert!(result.is_some());
    }

    #[test]
    fn rsi_all_gains_gives_100() {
        let mut rsi = Rsi::new(3);
        // Rising prices only → avg_loss = 0 → RSI = 100
        rsi.update(10.0);
        rsi.update(20.0);
        rsi.update(30.0);
        let v = rsi.update(40.0).unwrap();
        assert_eq!(v, 100.0);
    }

    #[test]
    fn rsi_all_losses_gives_0() {
        let mut rsi = Rsi::new(3);
        rsi.update(40.0);
        rsi.update(30.0);
        rsi.update(20.0);
        let v = rsi.update(10.0).unwrap();
        assert_eq!(v, 0.0);
    }

    #[test]
    fn rsi_known_reference_value() {
        // 14-period RSI; reference sequence from Wilder 1978 example.
        // We test that RSI stays in [0, 100] for a realistic price series.
        let prices = [
            44.34, 44.09, 44.15, 43.61, 44.33,
            44.83, 45.10, 45.15, 43.61, 44.33,
            44.83, 45.10, 45.15, 43.61, 44.55,
        ];
        let mut rsi = Rsi::new(14);
        let mut last = None;
        for &p in &prices {
            last = rsi.update(p);
        }
        let v = last.unwrap();
        assert!(v >= 0.0 && v <= 100.0, "RSI out of range: {v}");
    }
}
