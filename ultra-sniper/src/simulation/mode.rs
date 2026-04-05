//! Phase 18 — Mode enum: paper vs live routing.

/// Operating mode of the sniper bot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Paper trading: all trades are simulated, no real money.
    #[default]
    Paper,
    /// Live trading: orders are sent to the real Polymarket CLOB.
    Live,
}

impl Mode {
    pub fn is_paper(self) -> bool { self == Mode::Paper }
    pub fn is_live(self)  -> bool { self == Mode::Live  }

    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Paper => "PAPER",
            Mode::Live  => "LIVE",
        }
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_paper() {
        assert_eq!(Mode::default(), Mode::Paper);
    }

    #[test]
    fn paper_is_paper() {
        assert!(Mode::Paper.is_paper());
        assert!(!Mode::Paper.is_live());
    }

    #[test]
    fn live_is_live() {
        assert!(Mode::Live.is_live());
        assert!(!Mode::Live.is_paper());
    }

    #[test]
    fn paper_label() {
        assert_eq!(Mode::Paper.label(), "PAPER");
    }

    #[test]
    fn live_label() {
        assert_eq!(Mode::Live.label(), "LIVE");
    }

    #[test]
    fn display_paper() {
        assert_eq!(format!("{}", Mode::Paper), "PAPER");
    }

    #[test]
    fn display_live() {
        assert_eq!(format!("{}", Mode::Live), "LIVE");
    }

    #[test]
    fn equality() {
        assert_eq!(Mode::Paper, Mode::Paper);
        assert_ne!(Mode::Paper, Mode::Live);
    }
}
