//! The menu a tap of MENU opens over a game, as Slot's in-game menu: the game pauses behind it,
//! dimmed, and a lone panel titled MENU stands in the middle of the screen with the kit's bar
//! under it, B RETURN and A CONFIRM. Its rows are those of a game in play:
//!
//! - RESUME, back to the game, as B or MENU again do;
//! - SAVE STATE and LOAD STATE, as SELECT+R1 and SELECT+L1 do;
//! - STATES, the save-state switcher, as a double tap of MENU opens it;
//! - SCREEN FILTER, the LCD, a grid or nothing, changed in place with Left and Right, so the
//!   game behind shows it at once;
//! - COLOUR CORRECTION, on a Game Boy or GBA game, changed the same way;
//! - EXIT GAME, which saves a state and ejects the cart, as holding MENU does.
//!
//! The panel is one face, baked again only when what it shows, or the palette, changes.

use omega_gfx::{out_h, out_w};
use omega_input::{Action, Btn};
use omega_store::Platform;
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::row::{panel_height, row_face, row_rect, Row};
use omega_ui::{clean_label, Draw, QuickValue, UndoFace};

use crate::app::{App, Phase};
use crate::screens::lone::{notes, notes_height, over, rows_height, Lone, NOTE_GAP};

/// How wide the MENU panel is: the LINK and DISC panels' width.
pub const PANEL_W: u32 = crate::link_screen::PANEL_W;

/// How dark the game is behind the menu.
const DIM: f32 = 0.55;

/// A row of the MENU panel, in the order they are listed.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PauseRow {
    Resume,
    SaveState,
    LoadState,
    States,
    ScreenFilter,
    ColourCorrection,
    ExitGame,
}

impl PauseRow {
    pub const ALL: [PauseRow; 7] = [
        PauseRow::Resume,
        PauseRow::SaveState,
        PauseRow::LoadState,
        PauseRow::States,
        PauseRow::ScreenFilter,
        PauseRow::ColourCorrection,
        PauseRow::ExitGame,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PauseRow::Resume => "Resume",
            PauseRow::SaveState => "Save State",
            PauseRow::LoadState => "Load State",
            PauseRow::States => "States",
            PauseRow::ScreenFilter => "Screen Filter",
            PauseRow::ColourCorrection => "Colour Correction",
            PauseRow::ExitGame => "Exit Game",
        }
    }

    /// Left and Right change the row's value in place.
    pub fn carets(self) -> bool {
        matches!(self, PauseRow::ScreenFilter | PauseRow::ColourCorrection)
    }

    /// A opens a screen of its own for this row: the row ends in a ›.
    pub fn opens(self) -> bool {
        self == PauseRow::States
    }
}

/// The MENU panel while it is open: the row the cursor is on.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PauseMenu {
    pub row: PauseRow,
}

/// One row as the panel shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PauseLine {
    pub row: PauseRow,
    pub value: Option<String>,
    pub dim: bool,
}

/// What the MENU panel shows: its rows, the one lit, and the game's title as a note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PauseView {
    pub lines: Vec<PauseLine>,
    pub lit: usize,
    pub title: String,
}

/// The MENU panel in `pal`: a row a line, the lit one under the cursor, and the game's title
/// under them, the panel as tall as what it holds.
pub fn pause_panel_face(view: &PauseView, pal: &Palette) -> UndoFace {
    let held = rows_height(view.lines.len());
    let notes_of = [view.title.clone()];
    let mut panel = panel_face(
        PANEL_W,
        panel_height(held + notes_height(notes_of.len())),
        "MENU",
        true,
        pal,
    );
    for (i, line) in view.lines.iter().enumerate() {
        let [x, y, w, _] = row_rect(panel.well, i as u32);
        let row = Row {
            label: line.row.label(),
            value: line.value.as_deref(),
            lit: i == view.lit,
            carets: line.row.carets() && !line.dim,
            opens: line.row.opens(),
            dim: line.dim,
        };
        over(&mut panel.face, &row_face(w, &row, pal), x, y);
    }
    notes(&mut panel, &notes_of, held + NOTE_GAP, pal);
    panel.face
}

impl App {
    /// The rows the MENU panel lists for the game in the handheld: COLOUR CORRECTION and SCREEN
    /// FILTER only on the Game Boys and the GBA, whose screens have them.
    pub fn pause_rows(&self) -> Vec<PauseRow> {
        let handheld = matches!(self.platform, Platform::Gba | Platform::Gb | Platform::Gbc);
        PauseRow::ALL
            .into_iter()
            .filter(|r| match r {
                PauseRow::ScreenFilter => handheld,
                PauseRow::ColourCorrection => self.platform.colour_correction(),
                _ => true,
            })
            .collect()
    }

    pub fn pause_menu_open(&self) -> bool {
        self.pause_menu.is_some()
    }

    /// A tap of MENU in a game: the MENU panel over it, the cursor on RESUME.
    pub(crate) fn open_pause_menu(&mut self) {
        if !matches!(self.phase, Phase::Playing { .. }) || self.screen < 1.0 {
            return;
        }
        self.pause_menu = Some(PauseMenu {
            row: PauseRow::Resume,
        });
    }

    pub(crate) fn close_pause_menu(&mut self) {
        self.pause_menu = None;
    }

    /// What the MENU panel shows, while it is open.
    pub fn pause_view(&self) -> Option<PauseView> {
        let menu = self.pause_menu?;
        let rows = self.pause_rows();
        let lines = rows
            .iter()
            .map(|&row| PauseLine {
                row,
                value: match row {
                    PauseRow::ScreenFilter => Some(
                        QuickValue::filter(self.state.screen_filter)
                            .text()
                            .to_string(),
                    ),
                    PauseRow::ColourCorrection => Some(
                        QuickValue::flag(self.state.colour_correction)
                            .text()
                            .to_string(),
                    ),
                    _ => None,
                },
                dim: match row {
                    PauseRow::LoadState => !self.may_load_state(),
                    _ => false,
                },
            })
            .collect();
        Some(PauseView {
            lines,
            lit: rows.iter().position(|r| *r == menu.row).unwrap_or(0),
            title: self.seated().map(clean_label).unwrap_or_default(),
        })
    }

    pub(crate) fn pause_input(&mut self, action: Action) {
        let Some(menu) = self.pause_menu else {
            return;
        };
        let rows = self.pause_rows();
        let at = rows.iter().position(|r| *r == menu.row).unwrap_or(0);
        let n = rows.len();
        match action {
            Action::GbaDown(Btn::Up) => self.pause_at(rows[(at + n - 1) % n]),
            Action::GbaDown(Btn::Down) => self.pause_at(rows[(at + 1) % n]),
            Action::GbaDown(Btn::Left) => self.change_pause_row(menu.row, false),
            Action::GbaDown(Btn::Right) => self.change_pause_row(menu.row, true),
            Action::GbaDown(Btn::A) => self.choose_pause_row(menu.row),
            Action::GbaDown(Btn::B) | Action::QuickMenu => self.close_pause_menu(),
            // a second tap of MENU straight after the one that opened the menu: the switcher
            Action::Polaroids => {
                self.close_pause_menu();
                self.open_polaroids();
            }
            // MENU held down past its tap ejects, menu or not
            Action::Eject => {
                self.close_pause_menu();
                self.eject();
            }
            _ => {}
        }
    }

    fn pause_at(&mut self, row: PauseRow) {
        self.pause_menu = Some(PauseMenu { row });
    }

    fn change_pause_row(&mut self, row: PauseRow, right: bool) {
        match row {
            PauseRow::ScreenFilter => {
                self.state.screen_filter = self.state.screen_filter.step(right);
                self.persist();
            }
            PauseRow::ColourCorrection => self.change_setting(
                crate::screens::settings::SettingsRow::ColourCorrection,
                right,
            ),
            _ => {}
        }
    }

    fn choose_pause_row(&mut self, row: PauseRow) {
        match row {
            PauseRow::Resume => self.close_pause_menu(),
            PauseRow::SaveState => {
                self.close_pause_menu();
                self.save_state();
            }
            PauseRow::LoadState => {
                if !self.may_load_state() {
                    return self.refuse();
                }
                self.close_pause_menu();
                self.load_newest();
            }
            PauseRow::States => {
                self.close_pause_menu();
                self.open_polaroids();
            }
            PauseRow::ScreenFilter | PauseRow::ColourCorrection => self.change_pause_row(row, true),
            PauseRow::ExitGame => {
                self.close_pause_menu();
                self.eject();
            }
        }
    }

    pub fn set_pause_panel(&mut self, panel: Lone) {
        self.pause_panel = Some(panel);
    }

    /// The MENU panel over the game, dimmed behind it.
    pub(crate) fn draw_pause_menu(&self, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: out_w() as f32,
            h: out_h() as f32,
            colour: [0.0, 0.0, 0.0, DIM],
        });
        if let Some(panel) = &self.pause_panel {
            panel.draw(out);
        }
    }
}
