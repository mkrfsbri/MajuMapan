/// Incremental EMA — O(1) per update, stores only previous value.
pub struct Ema {
    pub value:    f64,
    pub k:        f64,       // 2/(period+1)
    initialized:  bool,
}

impl Ema {
    pub fn new(period: usize) -> Self {
        assert!(period >= 1);
        Self { value: 0.0, k: 2.0 / (period as f64 + 1.0), initialized: false }
    }

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
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ema_seed_equals_first_price() {
        let mut e = Ema::new(9);
        assert_eq!(e.update(100.0), 100.0);
    }

    #[test]
    fn ema_period3_incremental() {
        let mut e = Ema::new(3); // k = 0.5
        e.update(100.0);
        let v1 = e.update(101.0); // 101*0.5 + 100*0.5 = 100.5
        assert!((v1 - 100.5).abs() < 1e-9);
        let v2 = e.update(102.0); // 102*0.5 + 100.5*0.5 = 101.25
        assert!((v2 - 101.25).abs() < 1e-9);
    }

    #[test]
    fn ema9_stays_below_ema21_in_downtrend() {
        let mut e9  = Ema::new(9);
        let mut e21 = Ema::new(21);
        let prices: Vec<f64> = (0..50).map(|i| 1000.0 - i as f64 * 5.0).collect();
        let mut v9 = 0.0; let mut v21 = 0.0;
        for &p in &prices { v9 = e9.update(p); v21 = e21.update(p); }
        assert!(v9 < v21, "EMA9={v9} EMA21={v21}");
    }
}
