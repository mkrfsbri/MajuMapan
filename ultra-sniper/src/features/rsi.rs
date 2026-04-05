/// Wilder's RSI — rolling avg_gain/avg_loss, O(1) per update.
pub struct Rsi {
    period:     usize,
    avg_gain:   f64,
    avg_loss:   f64,
    prev_price: f64,
    count:      usize,
}

impl Rsi {
    pub fn new(period: usize) -> Self {
        assert!(period >= 1);
        Self { period, avg_gain: 0.0, avg_loss: 0.0, prev_price: 0.0, count: 0 }
    }

    pub fn update(&mut self, price: f64) -> Option<f64> {
        if self.count == 0 {
            self.prev_price = price;
            self.count += 1;
            return None;
        }

        let change = price - self.prev_price;
        let gain = if change > 0.0 {  change } else { 0.0 };
        let loss = if change < 0.0 { -change } else { 0.0 };
        self.prev_price = price;
        self.count += 1;

        if self.count <= self.period {
            self.avg_gain += gain;
            self.avg_loss += loss;
            if self.count == self.period {
                self.avg_gain /= self.period as f64;
                self.avg_loss /= self.period as f64;
                return Some(self.value());
            }
            return None;
        }

        let p = self.period as f64;
        self.avg_gain = (self.avg_gain * (p - 1.0) + gain) / p;
        self.avg_loss = (self.avg_loss * (p - 1.0) + loss) / p;
        Some(self.value())
    }

    #[inline]
    fn value(&self) -> f64 {
        if self.avg_loss == 0.0 { return 100.0; }
        100.0 - 100.0 / (1.0 + self.avg_gain / self.avg_loss)
    }

    pub fn is_ready(&self) -> bool { self.count >= self.period }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsi_none_during_warmup() {
        let mut rsi = Rsi::new(14);
        for i in 0..13 { assert!(rsi.update(44.0 + i as f64).is_none()); }
    }

    #[test]
    fn rsi_some_after_period() {
        let mut rsi = Rsi::new(14);
        let prices = [44.34,44.09,44.15,43.61,44.33,44.83,45.10,45.15,43.61,44.33,44.83,45.10,45.15,43.61];
        let mut v = None;
        for &p in &prices { v = rsi.update(p); }
        assert!(v.is_some());
    }

    #[test]
    fn rsi_all_gains_is_100() {
        let mut rsi = Rsi::new(3);
        rsi.update(10.0); rsi.update(20.0); rsi.update(30.0);
        assert_eq!(rsi.update(40.0).unwrap(), 100.0);
    }

    #[test]
    fn rsi_all_losses_is_0() {
        let mut rsi = Rsi::new(3);
        rsi.update(40.0); rsi.update(30.0); rsi.update(20.0);
        assert_eq!(rsi.update(10.0).unwrap(), 0.0);
    }

    #[test]
    fn rsi_in_range_0_to_100() {
        let mut rsi = Rsi::new(14);
        let prices = [44.34,44.09,44.15,43.61,44.33,44.83,45.10,45.15,43.61,44.33,44.83,45.10,45.15,43.61,44.55];
        let mut last = None;
        for &p in &prices { last = rsi.update(p); }
        let v = last.unwrap();
        assert!(v >= 0.0 && v <= 100.0, "RSI={v}");
    }
}
