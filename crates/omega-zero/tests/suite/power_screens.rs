//! The power screens. Holding POWER puts the POWER menu over the shelf, a menu or a game, a lone
//! panel over the backdrop that slides down in deeper from the screen it was held on and back up
//! out to exactly where it was; its bar is every menu's. Its choice slides on to RESTART or POWER OFF,
//! where nothing can be pressed and the bar sinks away, until the time comes to restart or power
//! off.

use crate::common;

use std::mem::discriminant;

use common::{app_booting_at, app_playing_in, tmp_root_with_carts, CLOCK_IS_SET};
use omega_input::{Action, Btn, Millis};
use omega_power::{Battery, Charge};
use omega_store::{write_app_state, AppState};
use omega_ui::kit::action::{action_pair, Font, Side};
use omega_ui::kit::bar::{action_swap, bar_layout, SINK};
use omega_ui::kit::layout::screen_px;
use omega_ui::{out_h, out_w, Draw, PowerChoice, SettingsRow, TexId, SPRITE_PX};
use omega_zero::app::{App, Phase};
use omega_zero::nav::{dropping, Dir, End, SLIDE_MS};
use omega_zero::screens::bar::{Actions, BarPlace, BarSet};
use omega_zero::screens::lone::Lone;
use omega_zero::screens::power::PowerScreen;
use tempfile::TempDir;

const MENU_PANEL: TexId = TexId::from_raw(960);
const GOING_PANEL: TexId = TexId::from_raw(961);
const CONTROLS_PANEL: TexId = TexId::from_raw(801);
const CARTS: [TexId; 2] = [TexId::from_raw(900), TexId::from_raw(901)];

fn menu_panel() -> Lone {
    Lone {
        tex: MENU_PANEL,
        w: 200,
        h: 49,
    }
}

/// A card with two carts, its clock set, booted to the shelf with every face these tests look
/// for uploaded as a made-up texture.
fn on_shelf() -> (TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_app_state(
        d.path(),
        &AppState {
            clock_set: true,
            ..AppState::default()
        },
    )
    .unwrap();
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    a.tick_ms(16);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    faces(&mut a);
    (d, a)
}

/// A game going, its picture drawn, with the same faces.
fn in_a_game() -> (TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.set_game_ready(true);
    a.update(1.0);
    faces(&mut a);
    assert!(drew_game(&frame(&a)), "the game is not drawn");
    (d, a)
}

fn faces(a: &mut App) {
    a.set_faces(CARTS.to_vec());
    a.set_controls_panel(Lone {
        tex: CONTROLS_PANEL,
        w: 304,
        h: 190,
    });
    a.set_power_menu_panel(menu_panel());
    a.set_going_panel(Lone {
        tex: GOING_PANEL,
        w: 200,
        h: 42,
    });
    let width = out_w() / SPRITE_PX;
    let pal = a.palette();
    let sets = Actions::ALL.map(|actions| {
        let [l, r] = action_pair(
            actions.action(Side::Left),
            actions.action(Side::Right),
            Font::Bold,
            pal,
        );
        let base = 800 + 10 * actions.index() + 20;
        BarSet {
            layout: bar_layout(width, &l, &r),
            left: (TexId::from_raw(base), l.face.w, l.face.h),
            right: (TexId::from_raw(base + 1), r.face.w, r.face.h),
        }
    });
    a.set_bar_sets(sets);
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn frame(a: &App) -> Vec<Draw> {
    let mut out = Vec::new();
    a.draw(&mut out);
    out
}

fn drew_game(out: &[Draw]) -> bool {
    out.iter().any(|d| matches!(d, Draw::Game))
}

fn x_of(out: &[Draw], tex: TexId) -> Option<f32> {
    at(out, tex).map(|(x, _)| x)
}

fn y_of(out: &[Draw], tex: TexId) -> Option<f32> {
    at(out, tex).map(|(_, y)| y)
}

/// Where `tex` is drawn: its top left.
fn at(out: &[Draw], tex: TexId) -> Option<(f32, f32)> {
    out.iter().find_map(|d| match *d {
        Draw::Tex { x, y, tex: t, .. } if t == tex => Some((x, y)),
        Draw::Spun { x, y, tex: t, .. } if t == tex => Some((x, y)),
        Draw::Turned { x, y, tex: t, .. } if t == tex => Some((x, y)),
        _ => None,
    })
}

/// Whether the top bar is drawn `dy` down from its place, across the whole screen.
fn top_bar_at(a: &App, out: &[Draw], dy: f32) -> bool {
    let w = out_w() as f32;
    out.iter().any(|d| {
        matches!(*d, Draw::Rect { x, y, w: bw, colour, .. }
            if x == 0.0 && y == dy && bw == w && colour == a.chrome_colours().housing)
    })
}

/// Whether the selected cart is drawn where the shelf at rest has it, `rest`, moved `dy` down
/// and not at all across. The carts bob up and down a few pixels on their own as time goes by,
/// so how far down is held to within that.
fn cart_at(out: &[Draw], rest: (f32, f32), dy: f32) -> bool {
    at(out, CARTS[0]).is_some_and(|(x, y)| x == rest.0 && (y - (rest.1 + dy)).abs() <= 8.0)
}

/// Where a lone panel stands at rest, down.
fn home(panel: Lone) -> f32 {
    screen_px(panel.rect()[1])
}

/// The slide to or from a power screen, its two ends and its way.
fn power_slid(a: &App) -> (Option<PowerScreen>, Dir, Option<PowerScreen>) {
    let slide = a.power_slide().expect("it did not slide");
    (slide.from, slide.dir, a.power_screen())
}

const MENU: Option<PowerScreen> = Some(PowerScreen::Menu(PowerChoice::Restart));

#[test]
fn holding_power_opens_the_menu_over_the_shelf_a_menu_and_a_game() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::PowerHold);
    assert_eq!(a.power_menu(), Some(0), "the menu opens on RESTART");
    assert_eq!(power_slid(&a), (None, Dir::Deeper, MENU));
    assert!(
        matches!(a.phase(), Phase::Shelf),
        "the shelf stays under it"
    );

    let (_d, mut a) = on_shelf();
    a.apply(Action::QuickMenu);
    a.finish_slide();
    press(&mut a, Btn::Down);
    a.apply(Action::PowerHold);
    assert_eq!(power_slid(&a), (None, Dir::Deeper, MENU));
    assert_eq!(a.settings_row(), Some(SettingsRow::ALL[1]));

    let (_d, mut a) = on_shelf();
    a.apply(Action::SelectTap);
    a.apply(Action::PowerHold);
    assert_eq!(power_slid(&a), (None, Dir::Deeper, MENU));
    assert!(matches!(a.phase(), Phase::Controls));

    let (_d, mut a) = in_a_game();
    a.apply(Action::PowerHold);
    assert_eq!(power_slid(&a), (None, Dir::Deeper, MENU));
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

/// From the shelf the menu comes down from the top as the whole shelf drops out of the bottom,
/// its carts and top bar together; B is the mirror, the menu going back up as the shelf
/// rises into place. Nothing moves across.
#[test]
fn halfway_in_from_the_shelf_and_back_both_are_where_the_slide_has_them() {
    let (_d, mut a) = on_shelf();
    let h = out_h() as f32;
    let rest = at(&frame(&a), CARTS[0]).expect("the selected cart is not drawn");
    let menu_x = screen_px(menu_panel().rect()[0]);

    let start: Millis = 10_000;
    a.apply_at(Action::PowerHold, start);
    a.tick_ms(start + SLIDE_MS / 2);
    let out = frame(&a);
    let half = dropping(Dir::Deeper, End::Leaving, 0.5, h);
    assert_eq!(half.carts_dy, h / 2.0);
    assert!(cart_at(&out, rest, half.carts_dy), "the carts");
    assert!(top_bar_at(&a, &out, h / 2.0), "the top bar");
    assert_eq!(
        at(&out, MENU_PANEL),
        Some((menu_x, home(menu_panel()) - h / 2.0)),
        "the menu is not halfway down from the top"
    );

    a.finish_slide();
    let out = frame(&a);
    assert_eq!(y_of(&out, MENU_PANEL), Some(home(menu_panel())));
    assert_eq!(
        x_of(&out, CARTS[0]),
        None,
        "the shelf still shows under the menu"
    );

    a.apply_at(Action::GbaDown(Btn::B), start + 1_000);
    assert_eq!(a.power_menu(), None);
    assert_eq!(power_slid(&a), (MENU, Dir::Back, None));
    a.tick_ms(start + 1_000 + SLIDE_MS / 2);
    let out = frame(&a);
    let half = dropping(Dir::Back, End::Arriving, 0.5, h);
    assert_eq!(half.carts_dy, h / 2.0, "the shelf rises from the bottom");
    assert!(cart_at(&out, rest, half.carts_dy), "the carts");
    assert!(top_bar_at(&a, &out, h / 2.0), "the top bar");
    assert_eq!(
        at(&out, MENU_PANEL),
        Some((menu_x, home(menu_panel()) - h / 2.0)),
        "the menu is not halfway back up"
    );

    a.update(SLIDE_MS as f32 / 1000.0);
    assert!(a.power_slide().is_none(), "the slide outlasted its time");
    let out = frame(&a);
    assert!(cart_at(&out, rest, 0.0), "the shelf is not back");
    assert_eq!(x_of(&out, MENU_PANEL), None);
}

/// From a menu the same: the menu drops out of the bottom as the power menu comes down from the
/// top, and back up.
#[test]
fn from_a_menu_the_menu_drops_out_of_the_bottom_and_rises_back() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::SelectTap);
    a.finish_slide();
    let h = out_h() as f32;
    let rest = at(&frame(&a), CONTROLS_PANEL).expect("Controls is not drawn");

    let start: Millis = 10_000;
    a.apply_at(Action::PowerHold, start);
    a.tick_ms(start + SLIDE_MS / 2);
    let out = frame(&a);
    assert_eq!(at(&out, CONTROLS_PANEL), Some((rest.0, rest.1 + h / 2.0)));
    assert_eq!(y_of(&out, MENU_PANEL), Some(home(menu_panel()) - h / 2.0));

    a.finish_slide();
    a.apply_at(Action::GbaDown(Btn::B), start + 1_000);
    a.tick_ms(start + 1_000 + SLIDE_MS / 2);
    let out = frame(&a);
    assert_eq!(at(&out, CONTROLS_PANEL), Some((rest.0, rest.1 + h / 2.0)));
    assert_eq!(y_of(&out, MENU_PANEL), Some(home(menu_panel()) - h / 2.0));
    a.finish_slide();
    assert!(matches!(a.phase(), Phase::Controls), "{:?}", a.phase());
    assert_eq!(at(&frame(&a), CONTROLS_PANEL), Some(rest));
}

/// A game's picture cannot slide: the backdrop takes its place at once, under the menu as it
/// comes in, and the game comes back once the menu has gone, held still until then.
#[test]
fn in_a_game_the_backdrop_takes_the_games_place_until_the_menu_has_gone() {
    let (_d, mut a) = in_a_game();
    let h = out_h() as f32;
    let at: Millis = 100_000;
    a.apply_at(Action::PowerHold, at);
    for t in [0, SLIDE_MS / 2, SLIDE_MS] {
        a.tick_ms(at + t);
        let out = frame(&a);
        assert!(!drew_game(&out), "the game shows under the menu at {t} ms");
        assert!(
            matches!(out.first(), Some(Draw::Rect { .. } | Draw::Tex { .. })),
            "the backdrop is not first: {:?}",
            out.first()
        );
    }
    a.finish_slide();
    assert_eq!(y_of(&frame(&a), MENU_PANEL), Some(home(menu_panel())));

    a.apply_at(Action::GbaDown(Btn::B), at + 1_000);
    a.tick_ms(at + 1_000 + SLIDE_MS / 2);
    let out = frame(&a);
    assert!(
        !drew_game(&out),
        "the game is back before the menu has gone"
    );
    assert!(a.power_shown(), "the game would run under the menu leaving");
    assert_eq!(y_of(&out, MENU_PANEL), Some(home(menu_panel()) - h / 2.0));

    a.update(SLIDE_MS as f32 / 1000.0);
    assert!(!a.power_shown());
    assert!(drew_game(&frame(&a)), "the game did not come back");
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn up_and_down_go_round_and_b_goes_back_to_where_power_was_held() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::QuickMenu);
    a.finish_slide();
    while a.settings_row() != Some(SettingsRow::Network) {
        press(&mut a, Btn::Down);
    }
    a.apply(Action::PowerHold);
    press(&mut a, Btn::Up);
    assert_eq!(a.power_menu(), Some(1), "Up from RESTART is POWER OFF");
    press(&mut a, Btn::Down);
    assert_eq!(a.power_menu(), Some(0), "Down from POWER OFF is RESTART");
    press(&mut a, Btn::Down);
    assert_eq!(a.power_menu(), Some(1));
    press(&mut a, Btn::Left);
    press(&mut a, Btn::Start);
    assert_eq!(
        a.power_menu(),
        Some(1),
        "a key the menu has no use for moved it"
    );
    press(&mut a, Btn::B);
    assert_eq!(a.power_menu(), None);
    assert_eq!(
        power_slid(&a),
        (
            Some(PowerScreen::Menu(PowerChoice::PowerOff)),
            Dir::Back,
            None
        )
    );
    assert_eq!(
        a.settings_row(),
        Some(SettingsRow::Network),
        "B did not go back to where POWER was held"
    );
    assert!(!a.shutting_down());
}

#[test]
fn a_press_during_the_slide_ends_it_and_acts_and_a_release_does_not() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::PowerHold);
    a.apply(Action::PowerOff);
    a.apply(Action::GbaUp(Btn::A));
    assert!(a.power_slide().is_some(), "a release ended the slide");
    a.apply(Action::GbaDown(Btn::Down));
    assert!(a.power_slide().is_none(), "the press left it sliding");
    assert_eq!(a.power_menu(), Some(1), "and did not act on the menu");
}

/// RESTART comes down deeper from the menu as it drops out of the bottom; the handheld may only
/// restart once it has slid all the way in.
#[test]
fn restart_slides_in_its_screen_and_the_machine_waits_until_it_is_in() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::PowerHold);
    a.finish_slide();
    let h = out_h() as f32;
    let at: Millis = 10_000;
    a.apply_at(Action::GbaDown(Btn::A), at);
    let going = Some(PowerScreen::Going(PowerChoice::Restart));
    assert_eq!(power_slid(&a), (MENU, Dir::Deeper, going));
    assert!(a.restarting() && !a.powering_off());
    assert_eq!(a.power_menu(), None, "the menu closes on the choice");

    a.tick_ms(at + SLIDE_MS / 2);
    let out = frame(&a);
    assert_eq!(y_of(&out, MENU_PANEL), Some(home(menu_panel()) + h / 2.0));
    let going_home = home(Lone {
        tex: GOING_PANEL,
        w: 200,
        h: 42,
    });
    assert_eq!(y_of(&out, GOING_PANEL), Some(going_home - h / 2.0));
    assert!(
        !a.ready_to_restart(),
        "it may restart halfway through the slide"
    );

    a.tick_ms(at + SLIDE_MS);
    assert!(!a.ready_to_restart(), "it may restart the moment it is in");
    assert_eq!(y_of(&frame(&a), GOING_PANEL), Some(going_home));
    a.tick_ms(at + 1_000);
    assert!(a.ready_to_restart(), "then it may restart");
    assert!(!a.ready_to_power_off());
}

#[test]
fn power_off_slides_in_its_own_screen_and_waits_the_same() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::PowerHold);
    press(&mut a, Btn::Down);
    let at: Millis = 10_000;
    a.apply_at(Action::GbaDown(Btn::A), at);
    assert_eq!(
        power_slid(&a),
        (
            Some(PowerScreen::Menu(PowerChoice::PowerOff)),
            Dir::Deeper,
            Some(PowerScreen::Going(PowerChoice::PowerOff))
        )
    );
    assert!(a.powering_off() && !a.restarting());
    a.tick_ms(at + SLIDE_MS);
    assert!(!a.ready_to_power_off());
    assert!(
        x_of(&frame(&a), GOING_PANEL).is_some(),
        "POWER OFF is not drawn"
    );
    a.tick_ms(at + 1_000);
    assert!(a.ready_to_power_off());
}

#[test]
fn nothing_can_be_pressed_while_the_handheld_goes_down() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::PowerHold);
    press(&mut a, Btn::A);
    let going = a.power_screen();
    for action in [
        Action::GbaDown(Btn::B),
        Action::GbaDown(Btn::A),
        Action::GbaDown(Btn::Up),
        Action::GbaDown(Btn::Start),
        Action::SelectTap,
        Action::PowerHold,
        Action::PowerTap,
        Action::LidClose,
    ] {
        a.apply(action);
        assert_eq!(a.power_screen(), going, "{action:?} moved it");
        assert!(
            matches!(a.phase(), Phase::Shelf),
            "{action:?}: {:?}",
            a.phase()
        );
    }
    assert!(a.restarting());
}

/// A power off omega-zero decides on its own slides the same screen in over whatever is shown.
#[test]
fn a_flat_battery_slides_power_off_in_over_whatever_is_shown() {
    let flat = Battery {
        percent: 3,
        charge: Charge::Discharging,
    };
    let going = Some(PowerScreen::Going(PowerChoice::PowerOff));
    let (_d, mut a) = on_shelf();
    a.on_battery(flat);
    assert_eq!(power_slid(&a), (None, Dir::Deeper, going));

    let (_d, mut a) = in_a_game();
    a.on_battery(flat);
    assert_eq!(power_slid(&a), (None, Dir::Deeper, going));
    let out = frame(&a);
    assert!(!drew_game(&out), "the game shows under POWER OFF");
    assert!(x_of(&out, GOING_PANEL).is_some());
}

/// The bar's place `p` of the way through a swap from `from`'s actions to `to`'s.
fn swap_place(from: Actions, to: Actions, p: f32) -> BarPlace {
    let swap = action_swap(p);
    BarPlace {
        actions: if swap.new { to } else { from },
        sink: swap.sink,
        drop: 0,
    }
}

/// The menu's bar is every menu's, B RETURN and A CONFIRM: from the shelf they swap with its
/// actions as it slides in and out, from a menu they stay put.
#[test]
fn the_menus_bar_is_return_and_confirm_swapped_with_the_screens_under_it() {
    let (_d, mut a) = on_shelf();
    let at: Millis = 10_000;
    a.apply_at(Action::PowerHold, at);
    for t in [SLIDE_MS / 4, 3 * SLIDE_MS / 4] {
        a.tick_ms(at + t);
        let p = a.power_slide().unwrap().progress(a.now());
        assert_eq!(
            a.bar_place(),
            Some(swap_place(Actions::Shelf, Actions::Menu, p)),
            "at {t} ms"
        );
    }
    a.finish_slide();
    let menu = BarPlace {
        actions: Actions::Menu,
        sink: 0,
        drop: 0,
    };
    assert_eq!(a.bar_place(), Some(menu));
    assert!(a.bar_hints_shown().is_empty(), "the menu has hints");

    a.apply_at(Action::GbaDown(Btn::B), at + 1_000);
    a.tick_ms(at + 1_000 + SLIDE_MS / 4);
    let p = a.power_slide().unwrap().progress(a.now());
    assert_eq!(
        a.bar_place(),
        Some(swap_place(Actions::Menu, Actions::Shelf, p))
    );

    let (_d, mut a) = on_shelf();
    a.apply(Action::SelectTap);
    a.finish_slide();
    let at: Millis = 10_000;
    a.apply_at(Action::PowerHold, at);
    for t in [SLIDE_MS / 4, SLIDE_MS / 2, 3 * SLIDE_MS / 4] {
        a.tick_ms(at + t);
        assert_eq!(a.bar_place(), Some(menu), "the actions moved at {t} ms");
    }
}

/// In a game there is no bar: it rises with the menu over the second half of its slide, and
/// sinks away with it over the first half of the slide back.
#[test]
fn in_a_game_the_bar_rises_with_the_menu_and_sinks_with_it() {
    let (_d, mut a) = in_a_game();
    assert_eq!(a.bar_place(), None);
    let at: Millis = 100_000;
    a.apply_at(Action::PowerHold, at);
    let out_of_sight = BarPlace {
        actions: Actions::Menu,
        sink: SINK,
        drop: 0,
    };
    a.tick_ms(at + SLIDE_MS / 4);
    assert_eq!(a.bar_place(), Some(out_of_sight));
    a.tick_ms(at + 3 * SLIDE_MS / 4);
    let p = a.power_slide().unwrap().progress(a.now());
    assert_eq!(
        a.bar_place(),
        Some(swap_place(Actions::Shelf, Actions::Menu, p))
    );

    a.finish_slide();
    a.apply_at(Action::GbaDown(Btn::B), at + 1_000);
    a.tick_ms(at + 1_000 + 3 * SLIDE_MS / 4);
    assert_eq!(a.bar_place(), Some(out_of_sight));
    a.finish_slide();
    assert_eq!(a.bar_place(), None);
}

/// After the choice nothing can be pressed: the whole bar sinks away with the menu's actions as
/// the screen slides in, as it does when a cart goes in, and is gone once it is in.
#[test]
fn the_bar_sinks_away_as_the_screen_after_the_choice_slides_in() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::PowerHold);
    a.finish_slide();
    let at: Millis = 10_000;
    a.apply_at(Action::GbaDown(Btn::A), at);
    let mut last = 0;
    for t in [0, SLIDE_MS / 4, SLIDE_MS / 2, 3 * SLIDE_MS / 4] {
        a.tick_ms(at + t);
        let p = a.power_slide().unwrap().progress(a.now());
        let drop = (p * SINK as f32).round() as u32;
        assert_eq!(
            a.bar_place(),
            Some(BarPlace {
                actions: Actions::Menu,
                sink: drop,
                drop,
            }),
            "at {t} ms"
        );
        assert!(drop >= last, "the bar came back up");
        last = drop;
        assert!(a.bar_hints_shown().is_empty());
    }
    a.tick_ms(at + SLIDE_MS);
    assert_eq!(a.bar_place(), None, "the bar is still there once it is in");
    a.finish_slide();
    assert_eq!(a.bar_place(), None);
}

/// Opening the menu over a slide between two screens ends that slide first.
#[test]
fn holding_power_during_a_slide_ends_it() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::QuickMenu);
    assert!(a.slide().is_some());
    a.apply(Action::PowerHold);
    assert!(
        a.slide().is_none(),
        "the slide to Settings went on under the menu"
    );
    assert_eq!(
        discriminant(a.phase()),
        discriminant(&Phase::Settings {
            row: SettingsRow::ALL[0]
        })
    );
}
