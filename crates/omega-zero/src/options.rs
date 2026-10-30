//! The GAMEPLAY and SHELF panels Settings opens from CONFIGS: Slot's settings omega-zero did not
//! have, in a lone panel each, as THEME is. Left and Right change the row the cursor is on, A
//! steps it on, and B or MENU go back to Settings. Each row's note, under the rows, says what it
//! does.
//!
//! GAMEPLAY: Fast Forward Sound and Rumble, which stood in CONFIGS, then Load State on Insert,
//! Save State on Eject, Rewind, Turbo Buttons, Frame Blending and the Game Boy's Palette.
//!
//! SHELF: Recents, Game Titles, the Clock's 12 or 24 hours, and Lid Power Off.
//!
//! GAMEPLAY's last row, RetroAchievements, opens a panel of its own, ACHIEVEMENTS: switching
//! them on, the account and Hardcore. Its Account row signs in on the keyboard, as Labels signs
//! in to ScreenScraper: A asks for the user name and then the password, and Y signs out.

use omega_input::{Action, Btn};
use omega_store::{Blending, LID_OFF_MINUTES};
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::row::{panel_height, row_face, row_rect, Row};
use omega_ui::Keyboard;
use omega_ui::{Draw, UndoFace};

use crate::app::{App, Phase};
use crate::nav::Dir;
use crate::screens::lone::{notes, notes_height, over, rows_height, Lone, NOTE_GAP};
use crate::screens::settings::SettingsRow;

/// How wide a panel is, in art pixels: room for Load State on Insert and a palette's name.
pub const PANEL_W: u32 = 268;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OptionsPage {
    Gameplay,
    Shelf,
    /// RetroAchievements, opened from GAMEPLAY's last row.
    Achievements,
}

impl OptionsPage {
    pub fn title(self) -> &'static str {
        match self {
            OptionsPage::Gameplay => "GAMEPLAY",
            OptionsPage::Shelf => "SHELF",
            OptionsPage::Achievements => "ACHIEVEMENTS",
        }
    }

    pub fn rows(self) -> &'static [OptionRow] {
        match self {
            OptionsPage::Gameplay => &[
                OptionRow::FastForwardSound,
                OptionRow::Rumble,
                OptionRow::LoadOnInsert,
                OptionRow::SaveOnEject,
                OptionRow::Rewind,
                OptionRow::Turbo,
                OptionRow::Blending,
                OptionRow::Palette,
                OptionRow::Achievements,
            ],
            OptionsPage::Achievements => {
                &[OptionRow::Cheevos, OptionRow::Account, OptionRow::Hardcore]
            }
            OptionsPage::Shelf => &[
                OptionRow::Recents,
                OptionRow::Titles,
                OptionRow::Clock,
                OptionRow::LidOff,
            ],
        }
    }

    /// The Settings row that opens it, or the panel it opens from.
    pub fn settings_row(self) -> SettingsRow {
        match self {
            OptionsPage::Gameplay | OptionsPage::Achievements => SettingsRow::Gameplay,
            OptionsPage::Shelf => SettingsRow::Shelf,
        }
    }

    /// The panel and row B goes back to, for a panel opened from another; none for one
    /// Settings opens.
    pub fn parent(self) -> Option<(OptionsPage, OptionRow)> {
        match self {
            OptionsPage::Achievements => Some((OptionsPage::Gameplay, OptionRow::Achievements)),
            OptionsPage::Gameplay | OptionsPage::Shelf => None,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OptionRow {
    FastForwardSound,
    Rumble,
    LoadOnInsert,
    SaveOnEject,
    Rewind,
    Turbo,
    Blending,
    Palette,
    Recents,
    Titles,
    Clock,
    LidOff,
    /// GAMEPLAY's way into the ACHIEVEMENTS panel.
    Achievements,
    Cheevos,
    /// The RetroAchievements account: A signs in on the keyboard, Y signs out.
    Account,
    Hardcore,
}

impl OptionRow {
    /// A opens the keyboard or a panel on the row rather than stepping it: the row ends in a ›.
    pub fn opens(self) -> bool {
        matches!(self, OptionRow::Account | OptionRow::Achievements)
    }

    pub fn label(self) -> &'static str {
        match self {
            OptionRow::FastForwardSound => "Fast Forward Sound",
            OptionRow::Rumble => "Rumble",
            OptionRow::LoadOnInsert => "Load State on Insert",
            OptionRow::SaveOnEject => "Save State on Eject",
            OptionRow::Rewind => "Rewind",
            OptionRow::Turbo => "Turbo Buttons",
            OptionRow::Blending => "Frame Blending",
            OptionRow::Palette => "Game Boy Palette",
            OptionRow::Recents => "Recents",
            OptionRow::Titles => "Game Titles",
            OptionRow::Clock => "Clock",
            OptionRow::LidOff => "Lid Power Off",
            OptionRow::Achievements => "RetroAchievements",
            OptionRow::Cheevos => "Achievements",
            OptionRow::Account => "Account",
            OptionRow::Hardcore => "Hardcore",
        }
    }

    /// What the row does, in the note under the rows while it is lit.
    pub fn note(self) -> &'static str {
        match self {
            OptionRow::FastForwardSound => "Sound while fast-forwarding",
            OptionRow::Rumble => "For games with a rumble pak",
            OptionRow::LoadOnInsert => "A picks up where you left off",
            OptionRow::SaveOnEject => "Ejecting saves where you were",
            OptionRow::Rewind => "Hold L2 to go back",
            OptionRow::Turbo => "Hold X and Y for turbo A and B",
            OptionRow::Blending => "Smooths flicker; ghosting is mGBA's",
            OptionRow::Palette => "Original Game Boy games only",
            OptionRow::Recents => "A shelf of your last 10 games",
            OptionRow::Titles => "The cart's name over the shelf",
            OptionRow::Clock => "12 or 24-hour time",
            OptionRow::LidOff => "A shut lid powers off after this",
            OptionRow::Achievements => "Account, hardcore and more",
            OptionRow::Cheevos => "Check games for achievements",
            OptionRow::Account => "A signs in, Y signs out",
            OptionRow::Hardcore => "No state loads or rewind, from the next game",
        }
    }
}

/// What the keyboard asks for while signing in: the user name, then the password.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Step {
    User,
    Password,
}

/// The keyboard up over the panel, signing in to RetroAchievements.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignIn {
    pub step: Step,
    /// The user name typed, once the password is being asked for.
    pub user: String,
    pub keys: Keyboard,
}

impl SignIn {
    pub fn prompt(&self) -> String {
        match self.step {
            Step::User => "RetroAchievements user name".to_string(),
            Step::Password => format!("RetroAchievements password for {}", self.user),
        }
    }
}

/// An options panel while it is open, the row the cursor is on, and the keyboard while it is
/// up for the account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionsScreen {
    pub page: OptionsPage,
    pub row: OptionRow,
    pub typing: Option<SignIn>,
}

impl OptionsScreen {
    pub fn new(page: OptionsPage) -> Self {
        OptionsScreen {
            page,
            row: page.rows()[0],
            typing: None,
        }
    }
}

/// What a panel shows: its title, each row's name and value, the lit row and its note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionsView {
    pub title: &'static str,
    pub lines: Vec<(OptionRow, String)>,
    pub lit: usize,
    /// The lit row's note: what it does, or for RetroAchievements who is signed in.
    pub note: String,
}

/// A panel in `pal`, every value between carets while its row is lit, and the lit row's note.
pub fn options_panel_face(view: &OptionsView, pal: &Palette) -> UndoFace {
    let held = rows_height(view.lines.len());
    let mut panel = panel_face(
        PANEL_W,
        panel_height(held + notes_height(1)),
        view.title,
        true,
        pal,
    );
    for (i, (row, value)) in view.lines.iter().enumerate() {
        let [x, y, w, _] = row_rect(panel.well, i as u32);
        let face = row_face(
            w,
            &Row {
                label: row.label(),
                value: Some(value),
                lit: i == view.lit,
                carets: !row.opens(),
                opens: row.opens(),
                dim: false,
            },
            pal,
        );
        over(&mut panel.face, &face, x, y);
    }
    notes(
        &mut panel,
        std::slice::from_ref(&view.note),
        held + NOTE_GAP,
        pal,
    );
    panel.face
}

fn on_off(on: bool) -> String {
    if on { "ON" } else { "OFF" }.to_string()
}

fn step<T: Copy + PartialEq>(items: &[T], at: T, right: bool) -> T {
    let n = items.len();
    let i = items.iter().position(|x| *x == at).unwrap_or(0);
    items[if right { (i + 1) % n } else { (i + n - 1) % n }]
}

impl App {
    /// What `row` is set to, as its panel prints it.
    pub fn option_text(&self, row: OptionRow) -> String {
        let s = &self.state;
        match row {
            OptionRow::FastForwardSound => on_off(s.ff_sound),
            OptionRow::Rumble => on_off(s.rumble),
            OptionRow::LoadOnInsert => on_off(s.auto_load),
            OptionRow::SaveOnEject => on_off(s.eject_save),
            OptionRow::Rewind => on_off(s.rewind),
            OptionRow::Turbo => on_off(s.turbo),
            OptionRow::Blending => match s.blending {
                Blending::Off => "OFF",
                Blending::Mix => "MIX",
                Blending::Ghosting => "GHOSTING",
            }
            .to_string(),
            OptionRow::Palette => s.gb_colors.label().to_uppercase(),
            OptionRow::Recents => if s.show_recents { "SHOW" } else { "HIDE" }.to_string(),
            OptionRow::Titles => if s.show_titles { "SHOW" } else { "HIDE" }.to_string(),
            OptionRow::Clock => if s.clock_24h { "24-HOUR" } else { "12-HOUR" }.to_string(),
            OptionRow::LidOff => format!("{} MIN", s.lid_off_min),
            OptionRow::Achievements | OptionRow::Cheevos => on_off(s.cheevos),
            OptionRow::Account => match &self.cheevos.signed_in {
                Some(name) => name.clone(),
                None => "SIGN IN".to_string(),
            },
            OptionRow::Hardcore => on_off(s.cheevos_hardcore),
        }
    }

    /// Steps `row` on, or back, and keeps it; a change a game in play can take reaches it.
    pub(crate) fn change_option(&mut self, row: OptionRow, right: bool) {
        let s = &mut self.state;
        match row {
            OptionRow::FastForwardSound => s.ff_sound = !s.ff_sound,
            OptionRow::Rumble => s.rumble = !s.rumble,
            OptionRow::LoadOnInsert => s.auto_load = !s.auto_load,
            OptionRow::SaveOnEject => s.eject_save = !s.eject_save,
            OptionRow::Rewind => s.rewind = !s.rewind,
            OptionRow::Turbo => s.turbo = !s.turbo,
            OptionRow::Blending => {
                s.blending = step(&Blending::ALL, s.blending, right);
                self.queue_core_options();
            }
            OptionRow::Palette => {
                s.gb_colors = match right {
                    true => s.gb_colors.next(),
                    false => s.gb_colors.prev(),
                };
                self.queue_core_options();
            }
            OptionRow::Recents => {
                s.show_recents = !s.show_recents;
                self.refresh_recents();
            }
            OptionRow::Titles => s.show_titles = !s.show_titles,
            OptionRow::Clock => s.clock_24h = !s.clock_24h,
            OptionRow::Cheevos => {
                s.cheevos = !s.cheevos;
                match s.cheevos {
                    true => self.start_achievements(),
                    false => self.stop_achievements(),
                }
            }
            // the keyboard signs in, not the arrows, and A opens the panel
            OptionRow::Account | OptionRow::Achievements => return,
            OptionRow::Hardcore => {
                s.cheevos_hardcore = !s.cheevos_hardcore;
                self.switch_hardcore();
            }
            OptionRow::LidOff => {
                s.lid_off_min = step(&LID_OFF_MINUTES, s.lid_off_min, right);
                let timeout = self.lid_timeout();
                if let Some(power) = &mut self.power {
                    power.set_timeout(timeout);
                }
            }
        }
        self.persist();
    }

    /// What the open panel shows.
    pub fn options_view(&self) -> Option<OptionsView> {
        let Phase::Options(screen) = &self.phase else {
            return None;
        };
        if screen.typing.is_some() {
            return None;
        }
        let rows = screen.page.rows();
        Some(OptionsView {
            title: screen.page.title(),
            lines: rows.iter().map(|r| (*r, self.option_text(*r))).collect(),
            lit: rows.iter().position(|r| *r == screen.row).unwrap_or(0),
            note: match screen.row {
                OptionRow::Achievements | OptionRow::Cheevos | OptionRow::Account => {
                    self.achievements_note()
                }
                row => row.note().to_string(),
            },
        })
    }

    pub(crate) fn options_input(&mut self, action: Action) {
        let Phase::Options(screen) = &mut self.phase else {
            return;
        };
        if screen.typing.is_some() {
            return self.sign_in_input(action);
        }
        let rows = screen.page.rows();
        let (page, row) = (screen.page, screen.row);
        match action {
            Action::GbaDown(Btn::Up) => screen.row = step(rows, row, false),
            Action::GbaDown(Btn::Down) => screen.row = step(rows, row, true),
            Action::GbaDown(Btn::A) if row == OptionRow::Account => self.open_sign_in(),
            Action::GbaDown(Btn::A) if row == OptionRow::Achievements => self.go(
                Phase::Options(OptionsScreen::new(OptionsPage::Achievements)),
                Dir::Deeper,
            ),
            Action::GbaDown(Btn::Y) if row == OptionRow::Account => self.sign_out_achievements(),
            Action::GbaDown(Btn::Left) => self.change_option(row, false),
            Action::GbaDown(Btn::Right | Btn::A) => self.change_option(row, true),
            Action::GbaDown(Btn::B) | Action::QuickMenu => match page.parent() {
                Some((parent, row)) => self.go(
                    Phase::Options(OptionsScreen {
                        row,
                        ..OptionsScreen::new(parent)
                    }),
                    Dir::Back,
                ),
                None => self.go(
                    Phase::Settings {
                        row: page.settings_row(),
                    },
                    Dir::Back,
                ),
            },
            _ => {}
        }
    }

    /// A on Account: the keyboard slides in over the panel, asking for the user name, the one
    /// signed in as already typed.
    fn open_sign_in(&mut self) {
        let user = self
            .root
            .as_deref()
            .and_then(crate::achievements::Account::load)
            .map(|a| a.user)
            .unwrap_or_default();
        let Phase::Options(screen) = &mut self.phase else {
            return;
        };
        let before = Phase::Options(screen.clone());
        screen.typing = Some(SignIn {
            step: Step::User,
            user: String::new(),
            keys: Keyboard {
                text: user,
                ..Keyboard::default()
            },
        });
        self.slide_from(before, Dir::Deeper);
    }

    /// A key on the keyboard: the user name, then the password, then signing in; B on an empty
    /// field, or MENU, puts the keyboard away.
    fn sign_in_input(&mut self, action: Action) {
        let Phase::Options(screen) = &mut self.phase else {
            return;
        };
        let before = Phase::Options(screen.clone());
        let Some(t) = &mut screen.typing else {
            return;
        };
        let keys = &mut t.keys;
        let mut submit = false;
        match action {
            Action::GbaDown(Btn::Up) => keys.up(),
            Action::GbaDown(Btn::Down) => keys.down(),
            Action::GbaDown(Btn::Left) => keys.left(),
            Action::GbaDown(Btn::Right) => keys.right(),
            Action::GbaDown(Btn::A) => submit = keys.press(),
            Action::GbaDown(Btn::B) if keys.text.is_empty() => {
                screen.typing = None;
                return self.slide_from(before, Dir::Back);
            }
            Action::GbaDown(Btn::B) => keys.delete(),
            Action::GbaDown(Btn::X) => keys.shift(),
            Action::GbaDown(Btn::Y) => keys.type_char(' '),
            Action::GbaDown(Btn::L1 | Btn::R1) => keys.symbols(),
            Action::GbaDown(Btn::Start) => submit = true,
            Action::QuickMenu => {
                screen.typing = None;
                return self.slide_from(before, Dir::Back);
            }
            _ => return,
        }
        if !submit || t.keys.text.is_empty() {
            return;
        }
        let typed = std::mem::take(&mut t.keys.text);
        match t.step {
            // from the user name to the password the keyboard stays where it is
            Step::User => {
                t.step = Step::Password;
                t.user = typed;
                t.keys = Keyboard::default();
            }
            Step::Password => {
                let user = std::mem::take(&mut t.user);
                screen.typing = None;
                self.slide_from(before, Dir::Back);
                self.sign_in_achievements(&user, &typed);
            }
        }
    }

    pub fn set_options_panel(&mut self, panel: Lone) {
        self.options_panel = Some(panel);
    }

    pub(crate) fn draw_options(&self, out: &mut Vec<Draw>) {
        if let Some(panel) = &self.options_panel {
            panel.draw(out);
        }
    }
}
