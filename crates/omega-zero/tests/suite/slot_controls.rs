//! The buttons as Slot has them: on the shelf MENU opens Settings, START picks the core and Y
//! stars the cart onto FAVORITES; in a game a tap of MENU opens the MENU panel over it, and
//! SELECT+A and SELECT+B step the screen filter.

use crate::common;

use common::{app_playing_in, boot, tmp_root_with_carts};
use omega_input::{Action, Btn};
use omega_store::{read_app_state, ScreenFilter};
use omega_zero::app::{App, Phase};
use omega_zero::pause_menu::PauseRow;

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn favorites_file(d: &tempfile::TempDir) -> String {
    std::fs::read_to_string(d.path().join("Config/favorites.txt")).unwrap_or_default()
}

#[test]
fn menu_opens_settings_on_the_shelf_and_start_picks_the_core() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    a.apply(Action::QuickMenu);
    assert!(
        matches!(a.phase(), Phase::Settings { .. }),
        "{:?}",
        a.phase()
    );
    a.apply(Action::QuickMenu);
    a.finish_slide();
    assert!(matches!(a.phase(), Phase::Shelf), "MENU did not close it");

    press(&mut a, Btn::Start);
    assert!(a.core_picker().is_some(), "START opened no core picker");
    assert!(matches!(a.phase(), Phase::Shelf), "START left the shelf");
}

#[test]
fn y_stars_the_cart_onto_favorites_first_and_y_again_takes_it_off() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    assert_eq!(a.pill_text().as_deref(), Some("GBA"), "no favorites yet");
    press(&mut a, Btn::Right);
    assert_eq!(a.selected_stem(), Some("Fusion"));
    press(&mut a, Btn::Y);
    assert_eq!(favorites_file(&d), "gba/Fusion\n");

    // FAVORITES is the shelf before the GBA's
    press(&mut a, Btn::L1);
    assert!(a.on_favorites());
    assert_eq!(a.pill_text().as_deref(), Some("FAVORITES"));
    assert_eq!(a.selected_stem(), Some("Fusion"));

    // taking the last star off leaves FAVORITES empty, and the shelf moves on
    press(&mut a, Btn::Y);
    assert_eq!(favorites_file(&d), "");
    assert!(!a.on_favorites(), "still on an empty FAVORITES");
    assert_eq!(a.pill_text().as_deref(), Some("GBA"));
}

#[test]
fn a_card_from_slot_boots_on_its_favorites() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion", "Zero"]);
    std::fs::write(
        d.path().join("Config/favorites.txt"),
        "gba/Zero\r\ngba/Emerald\ngbc/Not On This Card\n",
    )
    .unwrap();
    let mut a = boot(d.path());
    assert!(a.on_favorites());
    assert_eq!(a.selected_stem(), Some("Emerald"), "not in name order");
    press(&mut a, Btn::Right);
    assert_eq!(a.selected_stem(), Some("Zero"));
    press(&mut a, Btn::Right);
    assert_eq!(
        a.selected_stem(),
        Some("Emerald"),
        "a missing cart was shelved"
    );
}

#[test]
fn a_tap_of_menu_in_a_game_opens_the_menu_and_holds_the_game() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    a.apply(Action::QuickMenu);
    assert!(a.pause_menu_open());
    assert!(a.game_menu_open(), "the game is not held under the menu");
    let view = a.pause_view().expect("a view");
    assert_eq!(view.lines[view.lit].row, PauseRow::Resume);

    // B, or MENU again, goes back to the game
    press(&mut a, Btn::B);
    assert!(!a.pause_menu_open());
    a.apply(Action::QuickMenu);
    a.apply(Action::QuickMenu);
    assert!(!a.pause_menu_open());
}

#[test]
fn the_menu_changes_the_screen_filter_in_place_and_keeps_it() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::QuickMenu);
    for _ in 0..PauseRow::ALL.len() {
        if a.pause_view().map(|v| v.lines[v.lit].row) == Some(PauseRow::ScreenFilter) {
            break;
        }
        press(&mut a, Btn::Down);
    }
    let lit = a.pause_view().map(|v| v.lines[v.lit].row);
    assert_eq!(lit, Some(PauseRow::ScreenFilter), "Down never reached it");
    assert_eq!(a.screen_filter(), ScreenFilter::Lcd);
    press(&mut a, Btn::Right);
    assert_eq!(a.screen_filter(), ScreenFilter::Grid);
    press(&mut a, Btn::Right);
    assert_eq!(a.screen_filter(), ScreenFilter::Off);
    assert_eq!(read_app_state(d.path()).screen_filter, ScreenFilter::Off);
    let view = a.pause_view().unwrap();
    assert_eq!(view.lines[view.lit].value.as_deref(), Some("OFF"));
    assert!(a.pause_menu_open(), "changing a value closed the menu");
}

#[test]
fn exit_game_ejects_the_cart() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::QuickMenu);
    press(&mut a, Btn::Up);
    let view = a.pause_view().unwrap();
    assert_eq!(
        view.lines[view.lit].row,
        PauseRow::ExitGame,
        "Up wraps to the end"
    );
    press(&mut a, Btn::A);
    assert!(!a.pause_menu_open());
    assert!(
        matches!(a.phase(), Phase::Ejecting { .. }),
        "{:?}",
        a.phase()
    );
}

#[test]
fn a_second_tap_of_menu_opens_the_save_states_as_before() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    // the switcher opens only with a state to show
    a.apply(Action::SaveState);
    a.apply(Action::QuickMenu);
    a.apply(Action::Polaroids);
    assert!(!a.pause_menu_open());
    assert!(
        matches!(a.phase(), Phase::Polaroids { .. }),
        "{:?}",
        a.phase()
    );
}

#[test]
fn select_a_and_b_step_the_screen_filter_in_a_game() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::FilterNext);
    assert_eq!(a.screen_filter(), ScreenFilter::Grid);
    a.apply(Action::FilterPrev);
    a.apply(Action::FilterPrev);
    assert_eq!(a.screen_filter(), ScreenFilter::Off);
    assert_eq!(read_app_state(d.path()).screen_filter, ScreenFilter::Off);
}

fn booted_with(d: &tempfile::TempDir, change: impl FnOnce(&mut omega_store::AppState)) -> App {
    let mut s = omega_store::AppState {
        clock_set: true,
        ..Default::default()
    };
    change(&mut s);
    omega_store::write_app_state(d.path(), &s).unwrap();
    App::boot(d.path())
}

#[test]
fn holding_power_powers_off_with_no_menu_between() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    a.apply(Action::PowerPress);
    a.apply(Action::PowerHold);
    assert_eq!(a.power_menu(), None, "the POWER menu opened");
    assert!(a.powering_off(), "holding POWER did not power off");
}

#[test]
fn with_load_state_on_insert_off_a_starts_the_game_afresh() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = booted_with(&d, |s| s.auto_load = false);
    press(&mut a, Btn::A);
    assert!(
        matches!(a.phase(), Phase::Inserting { .. }),
        "{:?}",
        a.phase()
    );
    assert!(a.starting_clean(), "the state was loaded anyway");

    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    press(&mut a, Btn::A);
    assert!(
        !a.starting_clean(),
        "on by default, A picks up where it left off"
    );
}

#[test]
fn fast_forward_off_leaves_r2_doing_nothing() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    assert!(a.may_fast_forward());
    let mut s = read_app_state(d.path());
    s.ff_speed = 0;
    omega_store::write_app_state(d.path(), &s).unwrap();
    let a2 = App::boot(d.path());
    assert!(!a2.may_fast_forward());
    a.apply(Action::FfStart);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn recents_stands_first_with_the_last_game_put_in() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = booted_with(&d, |s| s.show_recents = true);
    assert_ne!(
        a.pill_text().as_deref(),
        Some("RECENTS"),
        "nothing played yet"
    );
    press(&mut a, Btn::Right);
    press(&mut a, Btn::A);
    let file = std::fs::read_to_string(d.path().join("Config/recents.txt")).unwrap();
    assert_eq!(file, "gba/Fusion\n");
    let a = booted_with(&d, |s| s.show_recents = true);
    assert_eq!(a.pill_text().as_deref(), Some("RECENTS"));
    assert_eq!(a.selected_stem(), Some("Fusion"));
    let hidden = booted_with(&d, |_| {});
    assert_ne!(
        hidden.pill_text().as_deref(),
        Some("RECENTS"),
        "shown while hidden"
    );
}

#[test]
fn game_titles_name_the_picked_cart_only_while_shown() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    assert_eq!(boot(d.path()).shelf_title(), None);
    let a = booted_with(&d, |s| s.show_titles = true);
    assert_eq!(a.shelf_title().as_deref(), Some("Emerald"));
}
