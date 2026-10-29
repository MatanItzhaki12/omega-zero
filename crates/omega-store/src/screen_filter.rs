//! What a Game Boy or GBA game at 3x is drawn through, as Settings > Screen Filter and SELECT+A
//! and SELECT+B in a game pick: the LCD mask it always had, a plain grid, or nothing.

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum ScreenFilter {
    /// Scanlines and the panel's stripes, as omega-zero always drew a game.
    #[default]
    Lcd,
    /// A thin dark line between the game's pixels.
    Grid,
    /// The game's pixels as they are.
    Off,
}

impl ScreenFilter {
    pub const ALL: [ScreenFilter; 3] = [ScreenFilter::Lcd, ScreenFilter::Grid, ScreenFilter::Off];

    pub fn key(self) -> &'static str {
        match self {
            ScreenFilter::Lcd => "lcd",
            ScreenFilter::Grid => "grid",
            ScreenFilter::Off => "off",
        }
    }

    pub fn from_key(key: &str) -> Option<ScreenFilter> {
        Self::ALL.into_iter().find(|f| f.key() == key)
    }

    /// The next filter along, or the one before, wrapping round.
    pub fn step(self, forward: bool) -> ScreenFilter {
        let n = Self::ALL.len();
        let at = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[if forward {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
    }
}
