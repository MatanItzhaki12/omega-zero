//! The GAMEPLAY and SHELF panels Settings opens from CONFIGS: Slot's settings omega-zero did not
//! have, in a lone panel each, as THEME is. Left and Right change the row the cursor is on, A
//! steps it on, and B or MENU go back to Settings. Each row's note, under the rows, says what it
//! does.
//!
//! GAMEPLAY: Fast Forward's speed, its Sound and Rumble, which stood in CONFIGS, then Load State
//! on Insert, Save State on Eject, Rewind, Turbo Buttons, Frame Blending and the Game Boy's
//! Palette. Its last row, Emulators, opens EMULATORS: the core each system's games run on unless
//! one was switched by hand in the core picker, and a row a system to put those back on it.
//!
//! SHELF: Recents, Game Titles, the Clock's 12 or 24 hours, Lid Power Off, and the Boot
//! Animation, the opening's terminal, off unless switched on.
//!
//! ACHIEVEMENTS, opened from CONFIGS' RetroAchievements: switching them on, the account and
//! Hardcore. Its Account row signs in on the keyboard, as Labels signs in to ScreenScraper: A
//! asks for the user name and then the password, and Y signs out.

use omega_input::{Action, Btn};
use omega_store::{Blending, Core, Platform, LID_OFF_MINUTES};
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::row::{panel_height, row_face, row_rect, Row};
use omega_ui::{Draw, Keyboard, QuickValue, Toast, UndoFace};

use crate::app::{App, Phase};
use crate::nav::Dir;
use crate::screens::lone::{notes, notes_height, over, rows_height, Lone, NOTE_GAP};
use crate::screens::settings::{ff_next, SettingsRow};

/// How wide a panel is, in art pixels: room for Load State on Insert and a palette's name.
pub const PANEL_W: u32 = 268;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OptionsPage {
    Gameplay,
    Shelf,
    /// RetroAchievements, opened from CONFIGS.
    Achievements,
    /// Each system's default core, opened from GAMEPLAY's last row.
    Emulators,
    /// The name and the relay of online play, opened from GAMEPLAY.
    Online,
}

/// The systems with more than one core to run on, so a default to pick.
pub const EMULATOR_SYSTEMS: [Platform; 2] = [Platform::Gba, Platform::Snes];

impl OptionsPage {
    pub fn title(self) -> &'static str {
        match self {
            OptionsPage::Gameplay => "GAMEPLAY",
            OptionsPage::Shelf => "SHELF",
            OptionsPage::Achievements => "ACHIEVEMENTS",
            OptionsPage::Emulators => "EMULATORS",
            OptionsPage::Online => "ONLINE PLAY",
        }
    }

    pub fn rows(self) -> &'static [OptionRow] {
        match self {
            OptionsPage::Gameplay => &[
                OptionRow::FastForward,
                OptionRow::FastForwardSound,
                OptionRow::Rumble,
                OptionRow::LoadOnInsert,
                OptionRow::SaveOnEject,
                OptionRow::Rewind,
                OptionRow::Turbo,
                OptionRow::RunAhead,
                OptionRow::Blending,
                OptionRow::Palette,
                OptionRow::OnlinePlay,
                OptionRow::Emulators,
            ],
            OptionsPage::Online => &[OptionRow::OnlineName, OptionRow::Relay],
            OptionsPage::Achievements => {
                &[OptionRow::Cheevos, OptionRow::Account, OptionRow::Hardcore]
            }
            OptionsPage::Emulators => &[
                OptionRow::DefaultCore(Platform::Gba),
                OptionRow::Switched(Platform::Gba),
                OptionRow::DefaultCore(Platform::Snes),
                OptionRow::Switched(Platform::Snes),
            ],
            OptionsPage::Shelf => &[
                OptionRow::Recents,
                OptionRow::Titles,
                OptionRow::Clock,
                OptionRow::LidOff,
                OptionRow::BootAnimation,
            ],
        }
    }

    /// The Settings row that opens it, or the panel it opens from.
    pub fn settings_row(self) -> SettingsRow {
        match self {
            OptionsPage::Gameplay | OptionsPage::Emulators | OptionsPage::Online => {
                SettingsRow::Gameplay
            }
            OptionsPage::Shelf => SettingsRow::Shelf,
            OptionsPage::Achievements => SettingsRow::Achievements,
        }
    }

    /// The panel and row B goes back to, for a panel opened from another; none for one
    /// Settings opens.
    pub fn parent(self) -> Option<(OptionsPage, OptionRow)> {
        match self {
            OptionsPage::Emulators => Some((OptionsPage::Gameplay, OptionRow::Emulators)),
            OptionsPage::Online => Some((OptionsPage::Gameplay, OptionRow::OnlinePlay)),
            OptionsPage::Gameplay | OptionsPage::Shelf | OptionsPage::Achievements => None,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OptionRow {
    /// Fast Forward's speed, which stood in CONFIGS.
    FastForward,
    FastForwardSound,
    Rumble,
    LoadOnInsert,
    SaveOnEject,
    Rewind,
    Turbo,
    RunAhead,
    Blending,
    Palette,
    Recents,
    Titles,
    Clock,
    LidOff,
    BootAnimation,
    /// GAMEPLAY's way into the EMULATORS panel.
    Emulators,
    /// The core a system's games run on unless switched by hand.
    DefaultCore(Platform),
    /// How many of a system's games were switched to another core by hand: A puts them back.
    Switched(Platform),
    Cheevos,
    /// The RetroAchievements account: A signs in on the keyboard, Y signs out.
    Account,
    Hardcore,
    /// GAMEPLAY's way into the ONLINE PLAY panel.
    OnlinePlay,
    /// The name online rooms show: A types it on the keyboard.
    OnlineName,
    /// The relay an online room is opened through.
    Relay,
}

impl OptionRow {
    /// A opens the keyboard or a panel on the row rather than stepping it: the row ends in a ›.
    pub fn opens(self) -> bool {
        matches!(
            self,
            OptionRow::Account
                | OptionRow::Emulators
                | OptionRow::OnlinePlay
                | OptionRow::OnlineName
        )
    }

    /// Left and Right step the row's value, which stands between a ◀ and a ▶ while it is lit.
    pub fn carets(self) -> bool {
        !self.opens() && !matches!(self, OptionRow::Switched(_))
    }

    pub fn label(self) -> &'static str {
        match self {
            OptionRow::FastForward => "Fast Forward",
            OptionRow::FastForwardSound => "Fast Forward Sound",
            OptionRow::Rumble => "Rumble",
            OptionRow::LoadOnInsert => "Load State on Insert",
            OptionRow::SaveOnEject => "Save State on Eject",
            OptionRow::Rewind => "Rewind",
            OptionRow::Turbo => "Turbo Buttons",
            OptionRow::RunAhead => "Run-Ahead",
            OptionRow::Blending => "Frame Blending",
            OptionRow::Palette => "Game Boy Palette",
            OptionRow::Recents => "Recents",
            OptionRow::Titles => "Game Titles",
            OptionRow::Clock => "Clock",
            OptionRow::LidOff => "Lid Power Off",
            OptionRow::BootAnimation => "Boot Animation",
            OptionRow::Emulators => "Emulators",
            OptionRow::DefaultCore(Platform::Snes) => "SNES Default",
            OptionRow::DefaultCore(_) => "GBA Default",
            OptionRow::Switched(Platform::Snes) => "SNES Switched Games",
            OptionRow::Switched(_) => "GBA Switched Games",
            OptionRow::Cheevos => "Achievements",
            OptionRow::Account => "Account",
            OptionRow::Hardcore => "Hardcore",
            OptionRow::OnlinePlay => "Online Play",
            OptionRow::OnlineName => "Name",
            OptionRow::Relay => "Relay",
        }
    }

    /// What the row does, in the note under the rows while it is lit.
    pub fn note(self) -> &'static str {
        match self {
            OptionRow::FastForward => "How fast fast-forward plays",
            OptionRow::FastForwardSound => "Sound while fast-forwarding",
            OptionRow::Rumble => "For games with a rumble pak",
            OptionRow::LoadOnInsert => "A picks up where you left off",
            OptionRow::SaveOnEject => "Ejecting saves where you were",
            OptionRow::Rewind => "Hold L2 to go back",
            OptionRow::Turbo => "Hold X and Y for turbo A and B",
            OptionRow::RunAhead => "A frame sooner on gpSP",
            OptionRow::Blending => "Smooths flicker; ghosting is mGBA's",
            OptionRow::Palette => "Original Game Boy games only",
            OptionRow::Recents => "A shelf of your last 10 games",
            OptionRow::Titles => "The cart's name over the shelf",
            OptionRow::Clock => "12 or 24-hour time",
            OptionRow::LidOff => "A shut lid powers off after this",
            OptionRow::BootAnimation => "The terminal before the shelf",
            OptionRow::Emulators => "Each system's default core",
            OptionRow::DefaultCore(_) => "Games not switched by hand",
            OptionRow::Switched(_) => "A puts them on the default",
            OptionRow::Cheevos => "Check games for achievements",
            OptionRow::Account => "A signs in, Y signs out",
            OptionRow::Hardcore => "No state loads or rewind, from the next game",
            OptionRow::OnlinePlay => "Your name and relay for online rooms",
            OptionRow::OnlineName => "What the other player sees",
            OptionRow::Relay => "The nearest is the quickest",
        }
    }
}

/// What the keyboard asks for while signing in: the user name, then the password.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Step {
    User,
    Password,
    /// The name for online rooms, typed on its own.
    OnlineName,
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
            Step::OnlineName => "Your name in online rooms".to_string(),
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
                value: (!value.is_empty()).then_some(value.as_str()),
                lit: i == view.lit,
                carets: row.carets(),
                opens: row.opens(),
                dim: false,
                caret_room: false,
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

/// A relay's name as the panel prints it, by its place in RetroArch's list.
pub fn relay_word(relay: u8) -> &'static str {
    match crate::netplay_lobby::RELAYS.get(relay as usize).copied() {
        Some("nyc") => "NEW YORK",
        Some("madrid") => "MADRID",
        Some("saopaulo") => "SAO PAULO",
        Some("singapore") => "SINGAPORE",
        _ => "MADRID",
    }
}

impl App {
    /// What `row` is set to, as its panel prints it.
    pub fn option_text(&self, row: OptionRow) -> String {
        let s = &self.state;
        match row {
            OptionRow::FastForward => QuickValue::speed(s.ff_speed)
                .map(|v| v.text().to_string())
                .unwrap_or_default(),
            OptionRow::FastForwardSound => on_off(s.ff_sound),
            OptionRow::Rumble => on_off(s.rumble),
            OptionRow::LoadOnInsert => on_off(s.auto_load),
            OptionRow::SaveOnEject => on_off(s.eject_save),
            OptionRow::Rewind => on_off(s.rewind),
            OptionRow::Turbo => on_off(s.turbo),
            OptionRow::RunAhead => on_off(s.run_ahead),
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
            OptionRow::BootAnimation => on_off(s.boot_animation),
            // a row that opens a panel shows only its ›
            OptionRow::Emulators => String::new(),
            OptionRow::DefaultCore(platform) => self.default_core(platform).name().to_uppercase(),
            OptionRow::Switched(platform) => match self.switched_games(platform) {
                0 => "NONE".to_string(),
                n => n.to_string(),
            },
            OptionRow::Cheevos => on_off(s.cheevos),
            OptionRow::Account => match &self.cheevos.signed_in {
                Some(name) => name.clone(),
                None => "SIGN IN".to_string(),
            },
            OptionRow::Hardcore => on_off(s.cheevos_hardcore),
            OptionRow::OnlinePlay => String::new(),
            OptionRow::OnlineName => self.online_name(),
            OptionRow::Relay => relay_word(s.relay).to_string(),
        }
    }

    /// Steps `row` on, or back, and keeps it; a change a game in play can take reaches it.
    pub(crate) fn change_option(&mut self, row: OptionRow, right: bool) {
        let s = &mut self.state;
        match row {
            OptionRow::FastForward => {
                let to = ff_next(s.ff_speed, right);
                if to == s.ff_speed {
                    return;
                }
                s.ff_speed = to;
            }
            OptionRow::FastForwardSound => s.ff_sound = !s.ff_sound,
            OptionRow::Rumble => s.rumble = !s.rumble,
            OptionRow::LoadOnInsert => s.auto_load = !s.auto_load,
            OptionRow::SaveOnEject => s.eject_save = !s.eject_save,
            OptionRow::Rewind => s.rewind = !s.rewind,
            OptionRow::Turbo => s.turbo = !s.turbo,
            OptionRow::RunAhead => s.run_ahead = !s.run_ahead,
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
            OptionRow::BootAnimation => s.boot_animation = !s.boot_animation,
            OptionRow::Cheevos => {
                s.cheevos = !s.cheevos;
                match s.cheevos {
                    true => self.start_achievements(),
                    false => self.stop_achievements(),
                }
            }
            // the keyboard signs in, not the arrows, and A opens the panel
            OptionRow::Account
            | OptionRow::Emulators
            | OptionRow::OnlinePlay
            | OptionRow::OnlineName => return,
            OptionRow::Relay => {
                let n = crate::netplay_lobby::RELAYS.len() as u8;
                s.relay = match right {
                    true => (s.relay + 1) % n,
                    false => (s.relay + n - 1) % n,
                };
            }
            // kept in default_core.ini, not the state
            OptionRow::DefaultCore(platform) => return self.step_default_core(platform, right),
            // A alone puts them back, not the arrows
            OptionRow::Switched(_) => return,
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
                OptionRow::Cheevos | OptionRow::Account => self.achievements_note(),
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
            Action::GbaDown(Btn::A) if row == OptionRow::OnlineName => self.open_online_name(),
            Action::GbaDown(Btn::A) if row == OptionRow::OnlinePlay => self.go(
                Phase::Options(OptionsScreen::new(OptionsPage::Online)),
                Dir::Deeper,
            ),
            Action::GbaDown(Btn::A) if row == OptionRow::Emulators => {
                self.count_switched_games();
                self.go(
                    Phase::Options(OptionsScreen::new(OptionsPage::Emulators)),
                    Dir::Deeper,
                )
            }
            Action::GbaDown(Btn::A) => match row {
                OptionRow::Switched(platform) => self.unswitch_games(platform),
                row => self.change_option(row, true),
            },
            Action::GbaDown(Btn::Y) if row == OptionRow::Account => self.sign_out_achievements(),
            Action::GbaDown(Btn::Left) => self.change_option(row, false),
            Action::GbaDown(Btn::Right) => self.change_option(row, true),
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

    /// A on Name: the keyboard slides in over the panel, the name rooms show already typed.
    fn open_online_name(&mut self) {
        let name = self.online_name();
        let Phase::Options(screen) = &mut self.phase else {
            return;
        };
        let before = Phase::Options(screen.clone());
        screen.typing = Some(SignIn {
            step: Step::OnlineName,
            user: String::new(),
            keys: Keyboard {
                text: name,
                ..Keyboard::default()
            },
        });
        self.slide_from(before, Dir::Deeper);
    }

    /// The name online rooms show: the one typed, else the RetroAchievements user's, else
    /// "RG SP".
    pub fn online_name(&self) -> String {
        if !self.state.online_name.is_empty() {
            return self.state.online_name.clone();
        }
        self.root
            .as_deref()
            .and_then(crate::achievements::Account::load)
            .map(|a| omega_store::online_name(&a.user))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "RG SP".to_string())
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
            Step::OnlineName => {
                screen.typing = None;
                self.slide_from(before, Dir::Back);
                self.state.online_name = omega_store::online_name(&typed);
                self.persist();
            }
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

    /// `platform`'s default core, as Settings set it.
    pub fn default_core(&self, platform: Platform) -> Core {
        match &self.root {
            Some(root) => omega_store::default_core(root, platform),
            None => Core::default_for(platform),
        }
    }

    /// Steps `platform`'s default core on to the next that runs it.
    fn step_default_core(&mut self, platform: Platform, right: bool) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let to = step(
            Core::for_platform(platform),
            self.default_core(platform),
            right,
        );
        if let Err(e) = omega_store::write_default_core(&root, platform, to) {
            eprintln!("omega-zero: core: could not write default_core.ini: {e}");
        }
        self.count_switched_games();
    }

    /// How many of `platform`'s games run on a core other than its default, switched by hand,
    /// as last counted.
    pub fn switched_games(&self, platform: Platform) -> usize {
        self.switched
            .iter()
            .find(|(p, _)| *p == platform)
            .map_or(0, |(_, stems)| stems.len())
    }

    /// Counts each system's games switched off its default by hand: on opening EMULATORS and
    /// after each change there, not each frame, since it reads the card.
    pub(crate) fn count_switched_games(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let picked = omega_store::read_selected_cores(&root);
        self.switched = EMULATOR_SYSTEMS
            .into_iter()
            .map(|platform| {
                let default = omega_store::default_core(&root, platform);
                let stems = self
                    .carts()
                    .filter(|c| c.platform == platform)
                    .filter(|c| {
                        picked
                            .get(&c.stem)
                            .is_some_and(|core| core.runs(platform) && *core != default)
                    })
                    .map(|c| c.stem.clone())
                    .collect();
                (platform, stems)
            })
            .collect();
    }

    /// A on a Switched row: every one of `platform`'s games switched by hand runs on its
    /// default again.
    fn unswitch_games(&mut self, platform: Platform) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let stems: Vec<String> = self
            .switched
            .iter()
            .find(|(p, _)| *p == platform)
            .map(|(_, stems)| stems.clone())
            .unwrap_or_default();
        if stems.is_empty() {
            return self.refuse();
        }
        for stem in &stems {
            if let Err(e) = omega_store::clear_selected_core(&root, stem) {
                eprintln!("omega-zero: core: could not write selected_core.ini: {e}");
            }
        }
        self.count_switched_games();
        let text = match stems.len() {
            1 => "1 game on the default".to_string(),
            n => format!("{n} games on the default"),
        };
        self.hud.toast(
            Toast::Emulator(Box::leak(text.into_boxed_str())),
            self.now(),
        );
    }

    pub fn set_options_panel(&mut self, title: &'static str, panel: Lone) {
        match self.options_panels.iter_mut().find(|(t, _)| *t == title) {
            Some((_, kept)) => *kept = panel,
            None => self.options_panels.push((title, panel)),
        }
    }

    /// `page`'s panel, as it was last baked: the one leaving a slide keeps its own face.
    pub(crate) fn draw_options(&self, page: OptionsPage, out: &mut Vec<Draw>) {
        let title = page.title();
        if let Some((_, panel)) = self.options_panels.iter().find(|(t, _)| *t == title) {
            panel.draw(out);
        }
    }
}
