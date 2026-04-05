/// Output from a single model.
#[derive(Debug, Clone, Copy)]
pub struct ModelOutput {
    pub p_win: f64,   // probability of winning trade [0, 1]
    pub weight: f64,  // relative confidence weight
}

impl ModelOutput {
    pub fn new(p_win: f64, weight: f64) -> Self {
        assert!((0.0..=1.0).contains(&p_win), "p_win out of range: {p_win}");
        assert!(weight >= 0.0, "weight must be non-negative");
        Self { p_win, weight }
    }
}

/// Weighted-average ensemble of model outputs.
/// Returns 0.5 when the slice is empty (neutral default).
pub fn ensemble(models: &[ModelOutput]) -> f64 {
    if models.is_empty() { return 0.5; }

    let total_weight: f64 = models.iter().map(|m| m.weight).sum();
    if total_weight == 0.0 { return 0.5; }

    let weighted_sum: f64 = models.iter().map(|m| m.p_win * m.weight).sum();
    (weighted_sum / total_weight).clamp(0.0, 1.0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_p_win_is_0_5_when_empty() {
        assert_eq!(ensemble(&[]), 0.5);
    }

    #[test]
    fn single_model_returns_its_p_win() {
        let models = [ModelOutput::new(0.7, 1.0)];
        assert!((ensemble(&models) - 0.7).abs() < 1e-9);
    }

    #[test]
    fn weighted_average_correct() {
        // (0.6×2 + 0.8×3) / (2+3) = (1.2 + 2.4) / 5 = 0.72
        let models = [ModelOutput::new(0.6, 2.0), ModelOutput::new(0.8, 3.0)];
        let v = ensemble(&models);
        assert!((v - 0.72).abs() < 1e-9, "ensemble={v}");
    }

    #[test]
    fn output_always_in_0_to_1() {
        let models = [
            ModelOutput::new(0.0, 1.0),
            ModelOutput::new(1.0, 1.0),
            ModelOutput::new(0.5, 2.0),
        ];
        let v = ensemble(&models);
        assert!(v >= 0.0 && v <= 1.0);
    }

    #[test]
    fn equal_weights_is_arithmetic_mean() {
        let models = [
            ModelOutput::new(0.4, 1.0),
            ModelOutput::new(0.6, 1.0),
            ModelOutput::new(0.8, 1.0),
        ];
        let v = ensemble(&models);
        assert!((v - 0.6).abs() < 1e-9);
    }
}
