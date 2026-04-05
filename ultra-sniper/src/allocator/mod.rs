/// One opportunity in the allocation pool.
#[derive(Debug, Clone, Copy)]
pub struct Opportunity {
    pub id:    u32,
    pub score: f64,  // composite score: higher → more capital
}

/// Allocation result for one opportunity.
#[derive(Debug, Clone, Copy)]
pub struct Allocation {
    pub id:     u32,
    pub weight: f64,  // fraction of capital [0, 1]
    pub amount: f64,  // absolute capital
}

/// Distribute `total_capital` proportionally by score.
/// Scores must be ≥ 0; all-zero scores produce equal weights.
pub fn allocate(opportunities: &[Opportunity], total_capital: f64) -> Vec<Allocation> {
    if opportunities.is_empty() { return Vec::new(); }

    let total_score: f64 = opportunities.iter().map(|o| o.score).sum();

    opportunities.iter().map(|o| {
        let weight = if total_score > 0.0 {
            o.score / total_score
        } else {
            1.0 / opportunities.len() as f64
        };
        Allocation {
            id:     o.id,
            weight,
            amount: weight * total_capital,
        }
    }).collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_sum_to_one() {
        let opps = vec![
            Opportunity { id: 1, score: 3.0 },
            Opportunity { id: 2, score: 1.0 },
            Opportunity { id: 3, score: 2.0 },
        ];
        let allocs = allocate(&opps, 1000.0);
        let sum: f64 = allocs.iter().map(|a| a.weight).sum();
        assert!((sum - 1.0).abs() < 1e-9, "sum={sum}");
    }

    #[test]
    fn amounts_sum_to_capital() {
        let opps = vec![
            Opportunity { id: 1, score: 2.0 },
            Opportunity { id: 2, score: 3.0 },
        ];
        let allocs = allocate(&opps, 500.0);
        let total: f64 = allocs.iter().map(|a| a.amount).sum();
        assert!((total - 500.0).abs() < 1e-9);
    }

    #[test]
    fn higher_score_gets_more_capital() {
        let opps = vec![
            Opportunity { id: 1, score: 1.0 },
            Opportunity { id: 2, score: 4.0 },
        ];
        let allocs = allocate(&opps, 1000.0);
        let a1 = allocs.iter().find(|a| a.id == 1).unwrap();
        let a2 = allocs.iter().find(|a| a.id == 2).unwrap();
        assert!(a2.amount > a1.amount, "a2={} a1={}", a2.amount, a1.amount);
    }

    #[test]
    fn equal_scores_equal_weights() {
        let opps = vec![
            Opportunity { id: 1, score: 1.0 },
            Opportunity { id: 2, score: 1.0 },
        ];
        let allocs = allocate(&opps, 100.0);
        assert!((allocs[0].weight - 0.5).abs() < 1e-9);
        assert!((allocs[1].weight - 0.5).abs() < 1e-9);
    }

    #[test]
    fn empty_opportunities_returns_empty() {
        let allocs = allocate(&[], 1000.0);
        assert!(allocs.is_empty());
    }
}
