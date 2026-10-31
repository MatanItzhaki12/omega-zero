//! The save-state switcher, MENU ×2 in a game: the screenshot of the save it shows, drawn through
//! the game's pipeline where the game is, and over it, at the top in the middle under the top
//! bar, the kit's SAVE STATES panel (kit/saves.rs) saying how old that save is and which of them
//! it is. The top bar and the bottom bar, B RETURN and A LOAD with X DELETE between them, are
//! the shelf's and the menus' own, which the app draws over it.

use omega_gfx::{out_h, out_w, Draw, TexId};
use omega_store::{parse_stamp, StateEntry};

use crate::art;
use crate::kit::layout::screen_px;
use crate::kit::pixel::quad;
use crate::sprite::SPRITE_PX;
use crate::status_bar::TOP_BAR_H;

/// A handheld game's screenshot, the GBA's screen; a blank photo is this size too.
pub const PHOTO_W: u32 = 240;
pub const PHOTO_H: u32 = 160;

/// The biggest screenshot kept at its own size, the biggest picture a core hands on. A bigger
/// one is cut down to a handheld's.
const PHOTO_MAX: (u32, u32) = (1024, 576);

const BLANK: [u8; 3] = [0x3a, 0x3a, 0x3e];

/// How far under the top bar the panel stands, in art pixels.
const PANEL_GAP: u32 = 8;

pub struct PhotoFace {
    pub rgba: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

/// The save's screenshot at its own size, a GBA game's 240x160 or a PlayStation game's 640x480,
/// to be drawn through the game's view as the game itself is.
pub fn photo_face(entry: &StateEntry) -> PhotoFace {
    let fits = |w: u32, h: u32| (1..=PHOTO_MAX.0).contains(&w) && (1..=PHOTO_MAX.1).contains(&h);
    if let Some((rgba, w, h)) = art::decode(&entry.thumb).filter(|(_, w, h)| fits(*w, *h)) {
        return PhotoFace { rgba, w, h };
    }
    let rgba = art::cover(&entry.thumb, PHOTO_W, PHOTO_H).unwrap_or_else(|| {
        std::iter::repeat_n(
            [BLANK[0], BLANK[1], BLANK[2], 255],
            (PHOTO_W * PHOTO_H) as usize,
        )
        .flatten()
        .collect()
    });
    PhotoFace {
        rgba,
        w: PHOTO_W,
        h: PHOTO_H,
    }
}

pub struct Polaroids {
    pub entries: Vec<StateEntry>,
    pub index: usize,
    faces: Vec<TexId>,
    /// The panel's face, uploaded, and its size in art pixels.
    panel: Option<(TexId, u32, u32)>,
}

impl Polaroids {
    pub fn new(entries: Vec<StateEntry>) -> Self {
        Polaroids {
            entries,
            index: 0,
            faces: Vec::new(),
            panel: None,
        }
    }

    pub fn set_faces(&mut self, faces: Vec<TexId>) {
        self.faces = faces;
    }

    /// The SAVE STATES panel, `kit::saves::saves_face` uploaded as `tex`, `w` by `h` art pixels.
    pub fn set_panel_face(&mut self, tex: TexId, w: u32, h: u32) {
        self.panel = Some((tex, w, h));
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn selected(&self) -> Option<&StateEntry> {
        self.entries.get(self.index)
    }

    pub fn dots(&self) -> (usize, usize) {
        (self.entries.len(), self.index)
    }

    /// How long before `now` the save shown was made, as its panel says it.
    pub fn age(&self, now: &str) -> String {
        self.selected()
            .map_or_else(String::new, |e| Self::age_of(&e.stamp, now))
    }

    /// Every save's age, newest first: the panel takes the room of the widest.
    pub fn ages(&self, now: &str) -> Vec<String> {
        self.entries
            .iter()
            .map(|e| Self::age_of(&e.stamp, now))
            .collect()
    }

    /// Takes the save shown out, its face with it, and shows the one after it, or the one
    /// before when it was the last.
    pub fn remove_selected(&mut self) -> Option<StateEntry> {
        if self.index >= self.entries.len() {
            return None;
        }
        let gone = self.entries.remove(self.index);
        if self.index < self.faces.len() {
            self.faces.remove(self.index);
        }
        self.index = self.index.min(self.entries.len().saturating_sub(1));
        Some(gone)
    }

    pub fn left(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.index = (self.index + 1).min(self.entries.len().saturating_sub(1));
    }

    /// The switcher over the game: the screenshot, then the panel once it is made.
    pub fn draw(&self, out: &mut Vec<Draw>) {
        self.draw_photo(out);
        if let Some((tex, w, h)) = self.panel {
            let [x, y, w, h] = Self::panel_rect(w, h);
            out.push(quad(tex, w, h, screen_px(x), screen_px(y)));
        }
    }

    /// Where a panel `w` by `h` art pixels goes, `[x, y, w, h]` in art pixels: across the
    /// middle of the screen, rounded down to a whole art pixel, `PANEL_GAP` under the top bar.
    pub fn panel_rect(w: u32, h: u32) -> [u32; 4] {
        let across = out_w() / SPRITE_PX;
        let top = TOP_BAR_H as u32 / SPRITE_PX + PANEL_GAP;
        [across.saturating_sub(w) / 2, top, w, h]
    }

    fn draw_photo(&self, out: &mut Vec<Draw>) {
        let (w, h) = (out_w() as f32, out_h() as f32);
        out.push(match self.faces.get(self.index) {
            Some(tex) => Draw::Shot { tex: *tex },
            None => Draw::Rect {
                x: 0.0,
                y: 0.0,
                w,
                h,
                colour: [
                    BLANK[0] as f32 / 255.0,
                    BLANK[1] as f32 / 255.0,
                    BLANK[2] as f32 / 255.0,
                    1.0,
                ],
            },
        });
    }

    /// `relative_time` as the panel's well says it, its first letter a capital: "Just now",
    /// "4 min ago", "2026-08-08 20:00".
    pub fn age_of(stamp: &str, now: &str) -> String {
        let said = Self::relative_time(stamp, now);
        let mut chars = said.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().chain(chars).collect(),
            None => said,
        }
    }

    pub fn relative_time(stamp: &str, now: &str) -> String {
        const RELATIVE_WINDOW: i64 = 12 * 3600;
        let (Some(then), Some(parsed)) = (parse_stamp(stamp), parse_stamp(now)) else {
            return stamp.to_string();
        };
        let delta = parsed - then;
        if delta < 60 {
            return "just now".into();
        }
        if delta < 3600 {
            return format!("{} min ago", delta / 60);
        }
        if delta < RELATIVE_WINDOW {
            return format!("{} hr ago", delta / 3600);
        }
        format!("{} {}:{}", &stamp[..10], &stamp[11..13], &stamp[14..16])
    }
}
