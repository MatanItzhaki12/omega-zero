//! Frame blending, as Slot offers it: some games flicker sprites on and off every other frame
//! to fake see-through effects, which the original screens were slow enough to blur. Mix blends
//! each frame with the one before; LCD Ghosting leaves a fading trail, on mGBA only, and gpSP
//! takes it as Mix.

use crate::core::Core;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Blending {
    #[default]
    Off,
    Mix,
    Ghosting,
}

impl Blending {
    pub const ALL: [Blending; 3] = [Blending::Off, Blending::Mix, Blending::Ghosting];

    pub fn key(self) -> &'static str {
        match self {
            Blending::Off => "off",
            Blending::Mix => "mix",
            Blending::Ghosting => "ghosting",
        }
    }

    pub fn from_key(key: &str) -> Option<Blending> {
        Self::ALL.into_iter().find(|b| b.key() == key)
    }

    /// The option `core` takes for it, spelt as that core declares it; none for a core without
    /// frame blending.
    pub fn option(self, core: Core) -> Option<(&'static str, &'static str)> {
        Some(match (core, self) {
            (Core::Mgba, Blending::Off) => ("mgba_interframe_blending", "OFF"),
            (Core::Mgba, Blending::Mix) => ("mgba_interframe_blending", "mix"),
            (Core::Mgba, Blending::Ghosting) => ("mgba_interframe_blending", "lcd_ghosting"),
            (Core::Gpsp, Blending::Off) => ("gpsp_frame_mixing", "disabled"),
            (Core::Gpsp, Blending::Mix | Blending::Ghosting) => ("gpsp_frame_mixing", "enabled"),
            _ => return None,
        })
    }
}
