use crate::strategy::Signal;

/// Side of the Polymarket trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Yes,
    No,
}

impl Side {
    pub fn from_signal(signal: Signal) -> Option<Self> {
        match signal {
            Signal::Up   => Some(Side::Yes),
            Signal::Down => Some(Side::No),
            Signal::None => None,
        }
    }
}

/// A live position.
#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub id:        u32,
    pub side:      Side,
    pub entry_price: f64,
    pub size:      f64,
    pub pnl:       f64,
}

impl Position {
    pub fn new(id: u32, side: Side, entry_price: f64, size: f64) -> Self {
        Self { id, side, entry_price, size, pnl: 0.0 }
    }

    /// Update PnL against current market price.
    pub fn update_pnl(&mut self, current_price: f64) {
        self.pnl = match self.side {
            Side::Yes => (current_price - self.entry_price) * self.size,
            Side::No  => (self.entry_price - current_price) * self.size,
        };
    }

    /// Realise PnL at settlement (price resolves to 0 or 1).
    pub fn settle(&mut self, resolution: f64) -> f64 {
        self.update_pnl(resolution);
        self.pnl
    }
}

/// Simple execution tracker.
pub struct Executor {
    pub positions: Vec<Position>,
    pub total_pnl: f64,
    next_id:       u32,
}

impl Executor {
    pub fn new() -> Self {
        Self { positions: Vec::new(), total_pnl: 0.0, next_id: 1 }
    }

    /// Open a new position. Returns the position id.
    pub fn open(&mut self, signal: Signal, entry_price: f64, size: f64) -> Option<u32> {
        let side = Side::from_signal(signal)?;
        let id   = self.next_id;
        self.next_id += 1;
        self.positions.push(Position::new(id, side, entry_price, size));
        Some(id)
    }

    /// Close a position by id at resolution price.
    pub fn close(&mut self, id: u32, resolution: f64) -> Option<f64> {
        if let Some(pos) = self.positions.iter_mut().find(|p| p.id == id) {
            let pnl = pos.settle(resolution);
            self.total_pnl += pnl;
            return Some(pnl);
        }
        None
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_up_maps_to_yes() {
        assert_eq!(Side::from_signal(Signal::Up),   Some(Side::Yes));
    }

    #[test]
    fn signal_down_maps_to_no() {
        assert_eq!(Side::from_signal(Signal::Down), Some(Side::No));
    }

    #[test]
    fn signal_none_maps_to_none() {
        assert_eq!(Side::from_signal(Signal::None), None);
    }

    #[test]
    fn yes_position_pnl_correct() {
        let mut pos = Position::new(1, Side::Yes, 0.4, 100.0);
        pos.update_pnl(0.7);
        // (0.7 - 0.4) × 100 = 30
        assert!((pos.pnl - 30.0).abs() < 1e-9);
    }

    #[test]
    fn no_position_pnl_correct() {
        let mut pos = Position::new(1, Side::No, 0.6, 100.0);
        pos.update_pnl(0.3);
        // (0.6 - 0.3) × 100 = 30
        assert!((pos.pnl - 30.0).abs() < 1e-9);
    }

    #[test]
    fn yes_position_settles_to_one() {
        let mut pos = Position::new(1, Side::Yes, 0.4, 100.0);
        let pnl = pos.settle(1.0);
        assert!((pnl - 60.0).abs() < 1e-9);
    }

    #[test]
    fn no_position_settles_to_zero() {
        // Bought NO at (1-0.6)=0.4. Market resolves to 0 (YES loses → NO wins).
        let mut pos = Position::new(1, Side::No, 0.4, 100.0);
        let pnl = pos.settle(0.0);
        // (0.4 - 0.0) × 100 = 40
        assert!((pnl - 40.0).abs() < 1e-9);
    }

    #[test]
    fn executor_open_and_close_tracks_pnl() {
        let mut exec = Executor::new();
        let id = exec.open(Signal::Up, 0.4, 100.0).unwrap();
        let pnl = exec.close(id, 1.0).unwrap();
        assert!((pnl - 60.0).abs() < 1e-9);
        assert!((exec.total_pnl - 60.0).abs() < 1e-9);
    }

    #[test]
    fn executor_open_signal_none_returns_none() {
        let mut exec = Executor::new();
        assert!(exec.open(Signal::None, 0.5, 100.0).is_none());
    }
}
