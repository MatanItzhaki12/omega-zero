//! The menu a tap of MENU opens over a game, laid out as Slot's in-game menu and drawn in
//! omega-zero's kit. The game pauses behind it; the bar across the top, with the clock and the
//! battery, stands over the game as Slot's corners do; and a lone panel holds the page, the lit
//! row's description under its rows, with the kit's bar under it, B RETURN and A CONFIRM.
//!
//! - MENU: Resume; Undo Save and Undo Load, for 30 seconds after a save or a load; States, while
//!   the game has any; Settings; and Exit Game, which saves a state and ejects the cart, but for
//!   a card with one cart.
//! - SETTINGS: Screen, Gameplay and Controls.
//! - SCREEN, for the system being played: Screen Filter, the Game Boy's Palette, Colour
//!   Correction, Frame Blending, Brightness and Blue Light. The game is not dimmed behind it and
//!   the panel stands at the bottom, so every change shows on the game as it is made.
//! - GAMEPLAY: Fast Forward, Fast Forward Sound, Rewind, Turbo Buttons, Rumble, Load State on
//!   Insert and Save State on Eject.
//! - CONTROLS: what each button does, as the shelf's CONTROLS shows it, for this system.
//!
//! B goes back a page, and from MENU back to the game, as MENU does from any page. The menu
//! comes up a moment after the tap that opens it, so a second tap straight after goes to the save
//! states without the menu flashing up between.

use omega_gfx::{out_h, out_w};
use omega_input::{Action, Btn, Millis};
use omega_store::{Core, Platform, BLUE_LIGHT_MAX, BRIGHTNESS_MAX, RING_MAX};
use omega_ui::kit::layout::{page, screen_px};
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::pixel::quad;
use omega_ui::kit::row::{panel_height, row_face, row_rect, Row};
use omega_ui::{Draw, QuickValue, Toast, UndoFace};

use crate::app::{App, Phase};
use crate::options::OptionRow;
use crate::screens::lone::{notes, notes_height, over, rows_height, Lone, NOTE_GAP};
use crate::screens::settings::SettingsRow;

/// How wide the panel is: the GAMEPLAY and SHELF panels' width.
pub const PANEL_W: u32 = crate::options::PANEL_W;

/// How dark the game is behind every page but SCREEN.
const DIM: f32 = 0.55;

/// How long after the tap that opens it the menu comes up, as Slot's does: room for the second
/// tap of a double tap, which goes to the save states.
pub const MENU_REVEAL_MS: Millis = 180;

/// How long after a save or a load it can be undone, as on Slot.
pub const UNDO_GRACE_MS: Millis = 30_000;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PausePage {
    Main,
    Settings,
    Screen,
    Gameplay,
    Controls,
}

impl PausePage {
    pub fn title(self) -> &'static str {
        match self {
            PausePage::Main => "MENU",
            PausePage::Settings => "SETTINGS",
            PausePage::Screen => "SCREEN",
            PausePage::Gameplay => "GAMEPLAY",
            PausePage::Controls => "CONTROLS",
        }
    }

    /// Every row the page can have, top to bottom; a game shows those it has a use for.
    pub fn rows(self) -> &'static [PauseRow] {
        match self {
            PausePage::Main => &[
                PauseRow::Resume,
                PauseRow::UndoSave,
                PauseRow::UndoLoad,
                PauseRow::States,
                PauseRow::Settings,
                PauseRow::ExitGame,
            ],
            PausePage::Settings => &[PauseRow::Screen, PauseRow::Gameplay, PauseRow::Controls],
            PausePage::Screen => &[
                PauseRow::ScreenFilter,
                PauseRow::Palette,
                PauseRow::ColourCorrection,
                PauseRow::Blending,
                PauseRow::Brightness,
                PauseRow::BlueLight,
            ],
            PausePage::Gameplay => &[
                PauseRow::FastForward,
                PauseRow::FastForwardSound,
                PauseRow::Rewind,
                PauseRow::Turbo,
                PauseRow::Rumble,
                PauseRow::LoadOnInsert,
                PauseRow::SaveOnEject,
            ],
            PausePage::Controls => &[],
        }
    }

    /// The page B goes back to, and the row it comes back to there; none from MENU.
    pub fn parent(self) -> Option<(PausePage, PauseRow)> {
        match self {
            PausePage::Main => None,
            PausePage::Settings => Some((PausePage::Main, PauseRow::Settings)),
            PausePage::Screen => Some((PausePage::Settings, PauseRow::Screen)),
            PausePage::Gameplay => Some((PausePage::Settings, PauseRow::Gameplay)),
            PausePage::Controls => Some((PausePage::Settings, PauseRow::Controls)),
        }
    }
}

/// A row of the menu.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PauseRow {
    Resume,
    UndoSave,
    UndoLoad,
    States,
    Settings,
    ExitGame,
    Screen,
    Gameplay,
    Controls,
    ScreenFilter,
    Palette,
    ColourCorrection,
    Blending,
    Brightness,
    BlueLight,
    FastForward,
    FastForwardSound,
    Rewind,
    Turbo,
    Rumble,
    LoadOnInsert,
    SaveOnEject,
}

impl PauseRow {
    pub fn label(self) -> &'static str {
        match self {
            PauseRow::Resume => "Resume",
            PauseRow::UndoSave => "Undo Save",
            PauseRow::UndoLoad => "Undo Load",
            PauseRow::States => "States",
            PauseRow::Settings => "Settings",
            PauseRow::ExitGame => "Exit Game",
            PauseRow::Screen => "Screen",
            PauseRow::Gameplay => "Gameplay",
            PauseRow::Controls => "Controls",
            PauseRow::ScreenFilter => "Screen Filter",
            PauseRow::Palette => "Palette",
            PauseRow::ColourCorrection => "Colour Correction",
            PauseRow::Blending => "Frame Blending",
            PauseRow::Brightness => "Brightness",
            PauseRow::BlueLight => "Blue Light",
            PauseRow::FastForward => "Fast Forward",
            PauseRow::FastForwardSound => "Fast Forward Sound",
            PauseRow::Rewind => "Rewind",
            PauseRow::Turbo => "Turbo Buttons",
            PauseRow::Rumble => "Rumble",
            PauseRow::LoadOnInsert => "Load State on Insert",
            PauseRow::SaveOnEject => "Save State on Eject",
        }
    }

    /// What the row does, in the line under the rows while it is lit.
    pub fn note(self) -> &'static str {
        match self {
            PauseRow::Resume => "Back to the game",
            PauseRow::UndoSave => "Take back the state just saved",
            PauseRow::UndoLoad => "Back to before the state loaded",
            PauseRow::States => "This game's save states",
            PauseRow::Settings => "Screen, gameplay and controls",
            PauseRow::ExitGame => "Save a state and eject the cart",
            PauseRow::Screen => "How this system's games look",
            PauseRow::Gameplay => "Fast forward, rewind and more",
            PauseRow::Controls => "What every button does",
            PauseRow::ScreenFilter => "LCD, a grid between pixels or none",
            PauseRow::Palette => "Original Game Boy games only",
            PauseRow::ColourCorrection => "Colours as the old screens had",
            PauseRow::Blending => "Smooths flicker; ghosting is mGBA's",
            PauseRow::Brightness => "SELECT+Up and Down do this too",
            PauseRow::BlueLight => "Warms the picture for the dark",
            PauseRow::FastForward => "The top speed while R2 is held",
            PauseRow::FastForwardSound => "Sound while fast-forwarding",
            PauseRow::Rewind => "Hold L2 to go back",
            PauseRow::Turbo => "Hold X and Y for turbo A and B",
            PauseRow::Rumble => "For games with a rumble pak",
            PauseRow::LoadOnInsert => "A picks up where you left off",
            PauseRow::SaveOnEject => "Ejecting saves where you were",
        }
    }

    /// A opens a page or does what the row says: the row ends in a › where it opens a page.
    pub fn opens(self) -> bool {
        matches!(
            self,
            PauseRow::Settings | PauseRow::Screen | PauseRow::Gameplay | PauseRow::Controls
        )
    }

    /// The page A opens on the row.
    fn child(self) -> Option<PausePage> {
        Some(match self {
            PauseRow::Settings => PausePage::Settings,
            PauseRow::Screen => PausePage::Screen,
            PauseRow::Gameplay => PausePage::Gameplay,
            PauseRow::Controls => PausePage::Controls,
            _ => return None,
        })
    }
}

/// The menu while it is open: the page, the row the cursor is on, and when it was opened.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PauseMenu {
    pub page: PausePage,
    pub row: PauseRow,
    pub opened: Millis,
}

/// A save or a load that can still be taken back: the game it was in, and what undoes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PendingUndo {
    /// The state saved, by its stamp, and the oldest state the save pushed out of the ring.
    Save {
        stem: String,
        stamp: String,
        evicted: Option<(String, Vec<u8>, Vec<u8>)>,
    },
    /// The game as it was before the load.
    Load { stem: String, prior: Vec<u8> },
}

/// One row as the panel shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PauseLine {
    pub row: PauseRow,
    pub value: Option<String>,
}

/// What the panel shows: its title, its rows, the lit one, and the lit row's description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PauseView {
    pub title: &'static str,
    pub lines: Vec<PauseLine>,
    pub lit: usize,
}

/// The panel in `pal`: a row a line, the lit one under the cursor, the ◀ ▶ round a value that
/// steps and a › on a row that opens a page, and the lit row's description under them.
pub fn pause_panel_face(view: &PauseView, pal: &Palette) -> UndoFace {
    let held = rows_height(view.lines.len());
    let mut panel = panel_face(
        PANEL_W,
        panel_height(held + notes_height(1)),
        view.title,
        true,
        pal,
    );
    for (i, line) in view.lines.iter().enumerate() {
        let [x, y, w, _] = row_rect(panel.well, i as u32);
        let row = Row {
            label: line.row.label(),
            value: line.value.as_deref(),
            lit: i == view.lit,
            carets: line.value.is_some(),
            opens: line.row.opens(),
            dim: false,
        };
        over(&mut panel.face, &row_face(w, &row, pal), x, y);
    }
    let note: Vec<String> = view
        .lines
        .get(view.lit)
        .map(|l| l.row.note().to_string())
        .into_iter()
        .collect();
    notes(&mut panel, &note, held + NOTE_GAP, pal);
    panel.face
}

fn step<T: Copy + PartialEq>(items: &[T], at: T, right: bool) -> T {
    let n = items.len();
    let i = items.iter().position(|x| *x == at).unwrap_or(0);
    items[if right { (i + 1) % n } else { (i + n - 1) % n }]
}

impl App {
    fn handheld(&self) -> bool {
        matches!(self.platform, Platform::Gba | Platform::Gb | Platform::Gbc)
    }

    /// The rows `page` shows for the game in play, as Slot hides those it has no use for.
    pub fn pause_rows(&self, page: PausePage) -> Vec<PauseRow> {
        let undo = self.undo_offer();
        page.rows()
            .iter()
            .copied()
            .filter(|r| match r {
                PauseRow::UndoSave => matches!(undo, Some(PendingUndo::Save { .. })),
                PauseRow::UndoLoad => {
                    matches!(undo, Some(PendingUndo::Load { .. })) && self.may_load_state()
                }
                PauseRow::States => !self.link_active() && !self.entries().is_empty(),
                PauseRow::ExitGame => !self.single_cart(),
                PauseRow::ScreenFilter | PauseRow::Blending => self.handheld(),
                PauseRow::ColourCorrection => self.platform.colour_correction(),
                PauseRow::Palette => self.platform == Platform::Gb && self.core == Core::Mgba,
                PauseRow::Rewind => self.platform.rewinds(),
                PauseRow::Turbo => self.platform.turbo_xy(),
                _ => true,
            })
            .collect()
    }

    pub fn pause_menu_open(&self) -> bool {
        self.pause_menu.is_some()
    }

    /// The menu once it has come up: a moment after the tap that opened it.
    pub fn pause_menu_shown(&self) -> Option<PauseMenu> {
        self.pause_menu
            .filter(|m| self.now().saturating_sub(m.opened) >= MENU_REVEAL_MS)
    }

    /// A tap of MENU in a game: the menu over it, on MENU's first row.
    pub(crate) fn open_pause_menu(&mut self) {
        if !matches!(self.phase, Phase::Playing { .. }) || self.screen < 1.0 {
            return;
        }
        self.pause_menu = Some(PauseMenu {
            page: PausePage::Main,
            row: PauseRow::Resume,
            opened: self.now(),
        });
    }

    pub(crate) fn close_pause_menu(&mut self) {
        self.pause_menu = None;
    }

    /// The menu's page while it is up, for the CONTROLS panel's sync.
    pub fn pause_page(&self) -> Option<PausePage> {
        self.pause_menu_shown().map(|m| m.page)
    }

    /// What `row` is set to, for a row with a value.
    fn pause_value(&self, row: PauseRow) -> Option<String> {
        let s = &self.state;
        Some(match row {
            PauseRow::ScreenFilter => QuickValue::filter(s.screen_filter).text().to_string(),
            PauseRow::ColourCorrection => QuickValue::flag(s.colour_correction).text().to_string(),
            PauseRow::Palette => self.option_text(OptionRow::Palette),
            PauseRow::Blending => self.option_text(OptionRow::Blending),
            PauseRow::Brightness => s.brightness.to_string(),
            PauseRow::BlueLight => match s.blue_light {
                0 => "OFF".to_string(),
                n => n.to_string(),
            },
            PauseRow::FastForward => QuickValue::speed(s.ff_speed)?.text().to_string(),
            PauseRow::FastForwardSound => self.option_text(OptionRow::FastForwardSound),
            PauseRow::Rewind => self.option_text(OptionRow::Rewind),
            PauseRow::Turbo => self.option_text(OptionRow::Turbo),
            PauseRow::Rumble => self.option_text(OptionRow::Rumble),
            PauseRow::LoadOnInsert => self.option_text(OptionRow::LoadOnInsert),
            PauseRow::SaveOnEject => self.option_text(OptionRow::SaveOnEject),
            _ => return None,
        })
    }

    /// What the panel shows, once the menu is up; CONTROLS has a panel of its own.
    pub fn pause_view(&self) -> Option<PauseView> {
        let menu = self.pause_menu_shown()?;
        if menu.page == PausePage::Controls {
            return None;
        }
        let rows = self.pause_rows(menu.page);
        Some(PauseView {
            title: menu.page.title(),
            lines: rows
                .iter()
                .map(|&row| PauseLine {
                    row,
                    value: self.pause_value(row),
                })
                .collect(),
            lit: rows.iter().position(|r| *r == menu.row).unwrap_or(0),
        })
    }

    pub(crate) fn pause_input(&mut self, action: Action) {
        let Some(menu) = self.pause_menu else {
            return;
        };
        let rows = self.pause_rows(menu.page);
        // a row that went away, an undo whose time ran out, leaves the cursor on the first
        let row = match rows.contains(&menu.row) {
            true => menu.row,
            false => rows.first().copied().unwrap_or(PauseRow::Resume),
        };
        match action {
            Action::GbaDown(Btn::Up) if !rows.is_empty() => {
                self.pause_at(menu.page, step(&rows, row, false))
            }
            Action::GbaDown(Btn::Down) if !rows.is_empty() => {
                self.pause_at(menu.page, step(&rows, row, true))
            }
            Action::GbaDown(Btn::Left) => self.change_pause_row(row, false),
            Action::GbaDown(Btn::Right) => self.change_pause_row(row, true),
            Action::GbaDown(Btn::A) => self.choose_pause_row(menu.page, row),
            Action::GbaDown(Btn::B) => match menu.page.parent() {
                Some((page, row)) => self.pause_at(page, row),
                None => self.close_pause_menu(),
            },
            Action::QuickMenu => self.close_pause_menu(),
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

    fn pause_at(&mut self, page: PausePage, row: PauseRow) {
        if let Some(menu) = &mut self.pause_menu {
            menu.page = page;
            menu.row = row;
        }
    }

    fn change_pause_row(&mut self, row: PauseRow, right: bool) {
        match row {
            PauseRow::ScreenFilter => {
                self.state.screen_filter = self.state.screen_filter.step(right);
                self.persist();
            }
            PauseRow::ColourCorrection => self.change_setting(SettingsRow::ColourCorrection, right),
            PauseRow::FastForward => self.change_setting(SettingsRow::FastForward, right),
            PauseRow::Brightness | PauseRow::BlueLight => {
                let (level, max) = match row {
                    PauseRow::Brightness => (&mut self.state.brightness, BRIGHTNESS_MAX),
                    _ => (&mut self.state.blue_light, BLUE_LIGHT_MAX),
                };
                *level = match right {
                    true => (*level + 1).min(max),
                    false => level.saturating_sub(1),
                };
                let brightness = self.state.brightness;
                if let Some(power) = &mut self.power {
                    power.set_backlight(brightness);
                }
                self.persist();
            }
            _ => {
                let option = match row {
                    PauseRow::Palette => OptionRow::Palette,
                    PauseRow::Blending => OptionRow::Blending,
                    PauseRow::FastForwardSound => OptionRow::FastForwardSound,
                    PauseRow::Rewind => OptionRow::Rewind,
                    PauseRow::Turbo => OptionRow::Turbo,
                    PauseRow::Rumble => OptionRow::Rumble,
                    PauseRow::LoadOnInsert => OptionRow::LoadOnInsert,
                    PauseRow::SaveOnEject => OptionRow::SaveOnEject,
                    _ => return,
                };
                self.change_option(option, right);
            }
        }
    }

    fn choose_pause_row(&mut self, page: PausePage, row: PauseRow) {
        if let Some(child) = row.child() {
            let first = self.pause_rows(child).first().copied();
            return self.pause_at(child, first.unwrap_or(row));
        }
        match row {
            PauseRow::Resume => self.close_pause_menu(),
            PauseRow::UndoSave | PauseRow::UndoLoad => {
                self.close_pause_menu();
                self.undo();
            }
            PauseRow::States => {
                self.close_pause_menu();
                self.open_polaroids();
            }
            PauseRow::ExitGame => {
                self.close_pause_menu();
                self.eject();
            }
            _ if page != PausePage::Main => self.change_pause_row(row, true),
            _ => {}
        }
    }

    pub fn set_pause_panel(&mut self, panel: Lone) {
        self.pause_panel = Some(panel);
    }

    /// The save or load that can still be undone, for the game in play.
    fn undo_offer(&self) -> Option<&PendingUndo> {
        let (undo, at) = self.pending_undo.as_ref()?;
        let stem = match undo {
            PendingUndo::Save { stem, .. } | PendingUndo::Load { stem, .. } => stem,
        };
        (self.now().saturating_sub(*at) <= UNDO_GRACE_MS && self.seated() == Some(stem))
            .then_some(undo)
    }

    /// Notes a state just saved as `stamp`, with the oldest it pushed out, for Undo Save.
    pub(crate) fn note_saved(
        &mut self,
        stamp: String,
        evicted: Option<(String, Vec<u8>, Vec<u8>)>,
    ) {
        let Some(stem) = self.seated().map(str::to_string) else {
            return;
        };
        let now = self.now();
        self.pending_undo = Some((
            PendingUndo::Save {
                stem,
                stamp,
                evicted,
            },
            now,
        ));
    }

    /// Notes the game as it was before a load, for Undo Load.
    pub(crate) fn note_loaded(&mut self, prior: Vec<u8>) {
        let Some(stem) = self.seated().map(str::to_string) else {
            return;
        };
        let now = self.now();
        self.pending_undo = Some((PendingUndo::Load { stem, prior }, now));
    }

    /// The oldest state, which a save into a full ring pushes out, kept to be put back.
    pub(crate) fn oldest_if_full(&self) -> Option<(String, Vec<u8>, Vec<u8>)> {
        let ring = self.ring()?;
        let entries = ring.list().ok()?;
        if entries.len() < RING_MAX {
            return None;
        }
        let oldest = entries.last()?;
        let (state, thumb) = ring.read(&oldest.stamp).ok()?;
        Some((oldest.stamp.clone(), state, thumb))
    }

    /// Takes back the save or load made last, as Slot's Undo Save and Undo Load do.
    pub(crate) fn undo(&mut self) {
        let Some(undo) = self.undo_offer().cloned() else {
            return self.refuse();
        };
        self.pending_undo = None;
        match undo {
            PendingUndo::Save { stamp, evicted, .. } => {
                let Some(ring) = self.ring() else {
                    return;
                };
                if let Err(e) = ring.remove(&stamp) {
                    eprintln!("omega-zero: undo save: {e}");
                    return self.refuse();
                }
                if let Some((old, state, thumb)) = evicted {
                    if let Err(e) = ring.push(&state, &thumb, &old) {
                        eprintln!("omega-zero: undo save: could not put {old} back: {e}");
                    }
                }
                self.hud.toast(Toast::SaveUndone, self.now());
            }
            PendingUndo::Load { prior, .. } => {
                if !self.may_load_state() {
                    return self.refuse();
                }
                if let Some(snapshot) = &self.snapshot {
                    snapshot.load(prior);
                }
                self.hud.toast(Toast::LoadUndone, self.now());
            }
        }
    }

    /// The menu over the game: the game dimmed, but on SCREEN, whose changes show on it; the
    /// bar across the top with the clock and the battery; and the page's panel, in the middle,
    /// or at the bottom on SCREEN, out of the game's way.
    pub(crate) fn draw_pause_menu(&self, out: &mut Vec<Draw>) {
        let Some(menu) = self.pause_menu_shown() else {
            return;
        };
        let screen = menu.page == PausePage::Screen;
        if !screen {
            out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: out_w() as f32,
                h: out_h() as f32,
                colour: [0.0, 0.0, 0.0, DIM],
            });
            self.draw_top_bar(out);
        }
        let panel = match menu.page {
            PausePage::Controls => self.controls_panel.as_ref(),
            _ => self.pause_panel.as_ref(),
        };
        let Some(panel) = panel else {
            return;
        };
        match screen {
            false => panel.draw(out),
            true => {
                let [pw, ph] = page();
                let x = pw.saturating_sub(panel.w) / 2;
                let y = ph.saturating_sub(panel.h + 2);
                out.push(quad(
                    panel.tex,
                    panel.w,
                    panel.h,
                    screen_px(x),
                    screen_px(y),
                ));
            }
        }
    }
}
