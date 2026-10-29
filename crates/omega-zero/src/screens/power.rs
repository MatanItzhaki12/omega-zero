//! Holding POWER, on any screen: the POWER menu, RESTART and POWER OFF, and after its choice the
//! screen shown until the handheld goes down, as flow.html's `#screen=power`, `#screen=restarting`
//! and `#screen=poweroff`. Both are lone panels over the backdrop, baked once for what they show
//! and the palette.
//!
//! They are put over the screen POWER was held on rather than taking its place, so whatever is
//! under them carries on as it was (a game held still) and B goes back to exactly where it was.
//! The menu opens deeper from that screen, sliding down rather than across: it comes down from
//! the top as the screen drops out of the bottom, the shelf as one, and B is the mirror, the
//! menu going back up as the screen rises into place. A game's picture cannot slide, so it gives
//! way to the backdrop at once and comes back once the menu has gone: the menus are drawn over
//! the backdrop, and the power menu is one of them. Its bar is every menu's, B RETURN and A
//! CONFIRM.
//!
//! A choice slides on deeper to the screen after it, titled RESTART or POWER OFF, which says what
//! is happening, coming down from the top as the menu drops away. Nothing can be pressed there,
//! so it has no actions: the whole bar sinks away as it slides in, as it does when a cart goes
//! in. A power off omega-zero decides on its own, for a flat battery or a long doze, slides the
//! same screen in over whatever is shown. It stays until the machine goes down: the last frame
//! drawn before the platform restarts or powers off is left on the panel while it does.

use omega_gfx::out_h;
use omega_input::{Action, Btn, Millis};
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::pixel::LINE;
use omega_ui::kit::row::panel_height;
use omega_ui::kit::text::text_face;
use omega_ui::{Draw, PowerChoice, UndoFace};

use crate::app::{backdrop_under, App};
use crate::list_view::{RowLabel, ViewRow};
use crate::nav::{dropping, progress, Dir, End, Offsets, SLIDE_MS};
use crate::screens::lone::{hold, over, rows_height, Lone, NOTE_GAP, NOTE_X};

/// How wide both panels are, in art pixels: flow.css's .lone.small.
pub const WIDTH: u32 = 200;

/// The menu's title.
pub const TITLE: &str = "POWER";

/// How tall a well holding a note on its own is: the note's line, `NOTE_GAP` above it and 5
/// below, so its capitals stand as far from the well's bottom as from its top (flow.css's
/// `.going .note`, as SSH's note on its own).
pub const NOTE_ALONE: u32 = NOTE_GAP + LINE + 5;

/// What holding POWER has put over the screen.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PowerScreen {
    /// RESTART and POWER OFF, the cursor on one.
    Menu(PowerChoice),
    /// The choice made, shown until the handheld restarts or turns off.
    Going(PowerChoice),
}

/// A slide to or from a power screen: the one it leaves, None for the screen POWER was held on,
/// which way it goes and when it started. The one it arrives at is the power screen shown, or
/// with none the screen it was held on.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PowerSlide {
    pub from: Option<PowerScreen>,
    pub dir: Dir,
    pub started: Millis,
}

impl PowerSlide {
    /// How far it has gone at `now`, eased.
    pub fn progress(&self, now: Millis) -> f32 {
        progress(self.started, now)
    }

    /// It has gone all the way by `now`.
    pub fn over(&self, now: Millis) -> bool {
        now.saturating_sub(self.started) >= SLIDE_MS
    }
}

/// The screen after `choice`'s title, and what it says while the handheld goes down.
pub fn going_words(choice: PowerChoice) -> (&'static str, &'static str) {
    match choice {
        PowerChoice::Restart => ("RESTART", "Restarting…"),
        PowerChoice::PowerOff => ("POWER OFF", "Turning off…"),
    }
}

/// The POWER menu as flow.html draws it, in `pal`, the cursor on `row`.
pub fn power_menu_face(row: PowerChoice, pal: &Palette) -> UndoFace {
    let rows: Vec<ViewRow> = PowerChoice::ALL
        .iter()
        .map(|choice| ViewRow {
            label: RowLabel::Setting(choice.text()),
            value: None,
            dim: false,
        })
        .collect();
    let content = rows_height(rows.len());
    let mut panel = panel_face(WIDTH, panel_height(content), TITLE, true, pal);
    hold(&mut panel, &rows, Some(row.index()), pal);
    panel.face
}

/// The screen after `choice` as flow.html draws it, in `pal`: its title, and its note on its own
/// in the dim colour.
pub fn going_face(choice: PowerChoice, pal: &Palette) -> UndoFace {
    let (title, note) = going_words(choice);
    let mut panel = panel_face(WIDTH, panel_height(NOTE_ALONE), title, true, pal);
    let [x, y, ..] = panel.well;
    over(
        &mut panel.face,
        &text_face(note, pal.dim),
        x + NOTE_X,
        y + NOTE_GAP,
    );
    panel.face
}

/// The choice `by` rows on from `row`, round from either end, as Settings' cursor goes.
fn step(row: PowerChoice, by: isize) -> PowerChoice {
    let n = PowerChoice::ALL.len() as isize;
    PowerChoice::ALL[(row.index() as isize + by).rem_euclid(n) as usize]
}

impl App {
    /// Slides `screen` in deeper, over the power screen shown or the screen POWER was held on.
    fn show_power(&mut self, screen: PowerScreen) {
        let from = self.power_screen.replace(screen);
        self.power_slide = Some(PowerSlide {
            from,
            dir: Dir::Deeper,
            started: self.now(),
        });
    }

    /// The screen saying the handheld is going down by `choice`, slid in over whatever is shown.
    pub(crate) fn show_going(&mut self, choice: PowerChoice) {
        self.finish_slide();
        self.show_power(PowerScreen::Going(choice));
    }

    /// What a press does on the POWER menu: Up and Down move the cursor, round from either end;
    /// A does what it is on and B goes back to the screen POWER was held on. A press during the
    /// slide ends it first. Nothing can be pressed on the screen after the choice.
    pub(crate) fn power_input(&mut self, action: Action) {
        let Some(PowerScreen::Menu(row)) = self.power_screen else {
            return;
        };
        if matches!(action, Action::GbaDown(_)) {
            self.power_slide = None;
        }
        match action {
            Action::GbaDown(Btn::Up) => self.power_screen = Some(PowerScreen::Menu(step(row, -1))),
            Action::GbaDown(Btn::Down) => self.power_screen = Some(PowerScreen::Menu(step(row, 1))),
            Action::GbaDown(Btn::B) => {
                self.power_screen = None;
                self.power_slide = Some(PowerSlide {
                    from: Some(PowerScreen::Menu(row)),
                    dir: Dir::Back,
                    started: self.now(),
                });
            }
            Action::GbaDown(Btn::A) => match row {
                PowerChoice::Restart => self.begin_restart(),
                PowerChoice::PowerOff => self.begin_power_off(),
            },
            _ => {}
        }
    }

    /// The POWER menu's lit row while it is open, as an index into `PowerChoice::ALL`.
    pub fn power_menu(&self) -> Option<usize> {
        match self.power_screen {
            Some(PowerScreen::Menu(row)) => Some(row.index()),
            _ => None,
        }
    }

    /// The power screen shown, if there is one.
    pub fn power_screen(&self) -> Option<PowerScreen> {
        self.power_screen
    }

    /// The slide to or from a power screen under way.
    pub fn power_slide(&self) -> Option<&PowerSlide> {
        self.power_slide.as_ref()
    }

    /// A power screen is shown, or sliding in or out: the game under it is held still.
    pub fn power_shown(&self) -> bool {
        self.power_screen.is_some() || self.power_slide.is_some()
    }

    pub fn set_power_menu_panel(&mut self, panel: Lone) {
        self.power_menu_panel = Some(panel);
    }

    pub fn set_going_panel(&mut self, panel: Lone) {
        self.going_panel = Some(panel);
    }

    /// The power screens over the backdrop and, through a slide, the screen POWER was held on
    /// leaving or coming back, each moving down or up; then the bar and the HUD over them.
    pub(crate) fn draw_power(&self, out: &mut Vec<Draw>) {
        self.draw_background(out);
        match &self.power_slide {
            Some(slide) => {
                let (p, h) = (slide.progress(self.now()), out_h() as f32);
                let leaving = dropping(slide.dir, End::Leaving, p, h);
                self.draw_power_end(slide.from, leaving, out);
                let arriving = dropping(slide.dir, End::Arriving, p, h);
                self.draw_power_end(self.power_screen, arriving, out);
            }
            None => self.draw_power_end(self.power_screen, Offsets::NONE, out),
        }
        self.draw_bar(out);
        self.hud.draw(self.now(), out);
    }

    /// `screen`'s panel moved by `off`, or with None the screen POWER was held on: a menu or the
    /// shelf, as it slides, but not a game, whose picture cannot.
    fn draw_power_end(&self, screen: Option<PowerScreen>, off: Offsets, out: &mut Vec<Draw>) {
        let panel = match screen {
            None if backdrop_under(&self.phase) => return self.draw_screen(&self.phase, off, out),
            None => return,
            Some(PowerScreen::Menu(_)) => self.power_menu_panel,
            Some(PowerScreen::Going(_)) => self.going_panel,
        };
        let Some(panel) = panel else {
            return;
        };
        let start = out.len();
        panel.draw(out);
        for d in &mut out[start..] {
            *d = d.shifted(off.dx, off.dy);
        }
    }
}
