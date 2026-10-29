//! The boot logo fading into the shelf: drawn over everything on the shelf's first frame, where
//! and as big as the bootloader drew it, eased out over half a second in four even steps, ended
//! at once by a press, which is then handled, and shown only by a boot that lands on the shelf.

use crate::common;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use omega_input::{Action, Btn};
use omega_store::{write_app_state, AppState};
use omega_ui::{out_h, out_w, Draw, TexId};
use omega_zero::app::{App, Phase};
use omega_zero::boot_fade::{alpha, FADE_MS, STEP_MS};
use tempfile::TempDir;

const LOGO: TexId = TexId::from_raw(900);

/// A frame at 60 fps, in seconds.
const FRAME: f32 = 1.0 / 60.0;

fn card(state: AppState) -> TempDir {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_app_state(d.path(), &state).unwrap();
    d
}

fn clock_set() -> AppState {
    AppState {
        clock_set: true,
        ..AppState::default()
    }
}

/// A boot that has landed on the shelf, handed the logo before its first frame, as the frontend
/// hands it over.
fn booted_on_the_shelf() -> (TempDir, App) {
    let d = card(clock_set());
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(matches!(a.phase(), Phase::Shelf));
    a.set_boot_logo(LOGO, out_w(), out_h());
    (d, a)
}

/// The logo in this frame, as its rect and alpha, and whether nothing is drawn over it.
fn logo(a: &App) -> Option<([f32; 4], f32, bool)> {
    let mut out = Vec::new();
    a.draw(&mut out);
    let last = out.len().checked_sub(1)?;
    out.iter().enumerate().find_map(|(i, d)| match *d {
        Draw::Tex {
            x,
            y,
            w,
            h,
            tex,
            alpha,
        } if tex == LOGO => Some(([x, y, w, h], alpha, i == last)),
        _ => None,
    })
}

fn frames(a: &mut App, n: u32) {
    for _ in 0..n {
        a.update(FRAME);
    }
}

#[test]
fn the_logo_eases_out_over_half_a_second() {
    assert_eq!(FADE_MS, 500.0);
    assert_eq!(alpha(0.0), 1.0);
    assert_eq!(alpha(FADE_MS / 2.0), 0.5);
    assert_eq!(alpha(FADE_MS), 0.0);
    assert_eq!(alpha(2.0 * FADE_MS), 0.0, "it went past");
    assert!(alpha(FADE_MS / 4.0) > 0.75, "it does not ease in");
    assert!(alpha(3.0 * FADE_MS / 4.0) < 0.25, "it does not ease out");
    let mut was = 1.0;
    let mut steps = vec![1.0];
    for ms in 0..=500 {
        let now = alpha(ms as f32);
        assert!(now <= was, "it came back at {ms} ms");
        if now != was {
            steps.push(now);
        }
        was = now;
    }
    assert_eq!(
        steps,
        [1.0, 0.75, 0.5, 0.25, 0.0],
        "it does not fade in the kit's four even steps"
    );
}

#[test]
fn the_first_frame_is_the_logo_whole_over_everything_where_the_bootloader_drew_it() {
    let (_d, mut a) = booted_on_the_shelf();
    assert_eq!(a.boot_logo_alpha(), Some(1.0), "before the first frame");
    a.update(FRAME);
    let (rect, alpha, on_top) = logo(&a).expect("the first frame has no logo");
    assert_eq!(rect, [0.0, 0.0, out_w() as f32, out_h() as f32]);
    assert_eq!(alpha, 1.0, "the first frame is not the bootloader's logo");
    assert!(on_top, "something is drawn over the logo");
}

#[test]
fn the_fade_starts_at_the_first_frame_however_long_the_boot_took() {
    let (_d, mut a) = booted_on_the_shelf();
    a.update(3.0);
    assert_eq!(a.boot_logo_alpha(), Some(1.0));
}

#[test]
fn the_logo_fades_frame_by_frame_and_is_gone_in_half_a_second() {
    let (_d, mut a) = booted_on_the_shelf();
    a.update(FRAME);
    frames(&mut a, 15);
    let half = a.boot_logo_alpha().expect("gone before halfway");
    assert!((half - 0.5).abs() < 1e-3, "{half} halfway");
    let (_, drawn, _) = logo(&a).expect("no logo halfway");
    assert_eq!(drawn, half);
    frames(&mut a, 14);
    assert!(a.boot_logo_alpha().is_some_and(|v| v < 0.01));
    frames(&mut a, 2);
    assert_eq!(
        a.boot_logo_alpha(),
        None,
        "still fading after half a second"
    );
    assert!(logo(&a).is_none(), "the logo is drawn after its fade");
    assert!(matches!(a.phase(), Phase::Shelf));
}

#[test]
fn a_slow_frame_slows_the_fade_rather_than_skipping_part_of_it() {
    let (_d, mut a) = booted_on_the_shelf();
    a.update(FRAME);
    a.update(0.4);
    assert_eq!(a.boot_logo_alpha(), Some(alpha(STEP_MS)));
}

#[test]
fn a_press_ends_the_fade_at_once_and_is_then_handled() {
    let (_d, mut a) = booted_on_the_shelf();
    frames(&mut a, 4);
    a.apply(Action::QuickMenu);
    assert_eq!(a.boot_logo_alpha(), None);
    assert!(
        logo(&a).is_none(),
        "the logo is still drawn after the press"
    );
    assert!(
        matches!(a.phase(), Phase::Settings { .. }),
        "START did not open Settings: {:?}",
        a.phase()
    );
}

#[test]
fn a_press_on_the_shelf_moves_it_as_well_as_ending_the_fade() {
    let (_d, mut a) = booted_on_the_shelf();
    frames(&mut a, 4);
    let before = a.selected_stem().map(str::to_string);
    a.apply(Action::GbaDown(Btn::Right));
    a.update(FRAME);
    assert_eq!(a.boot_logo_alpha(), None);
    assert_ne!(
        a.selected_stem().map(str::to_string),
        before,
        "Right did not move"
    );
}

#[test]
fn a_release_or_the_lid_found_open_leaves_the_fade_going() {
    let (_d, mut a) = booted_on_the_shelf();
    a.update(FRAME);
    a.apply(Action::GbaUp(Btn::A));
    a.apply(Action::LidOpen);
    assert_eq!(a.boot_logo_alpha(), Some(1.0));
    assert!(matches!(a.phase(), Phase::Shelf));
}

#[test]
fn a_boot_straight_back_into_a_game_shows_no_logo() {
    let d = card(AppState {
        cart: Some("Emerald".into()),
        ..clock_set()
    });
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(matches!(a.phase(), Phase::Inserting { .. }));
    assert!(!a.boot_fades());
    a.set_boot_logo(LOGO, out_w(), out_h());
    a.update(FRAME);
    assert_eq!(a.boot_logo_alpha(), None);
    assert!(logo(&a).is_none());
}

#[test]
fn a_first_boot_on_the_clock_shows_no_logo_then_nor_on_the_shelf_after_it() {
    let d = card(AppState::default());
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(matches!(a.phase(), Phase::SetClock { .. }));
    assert!(!a.boot_fades());
    a.set_boot_logo(LOGO, out_w(), out_h());
    a.update(FRAME);
    assert_eq!(a.boot_logo_alpha(), None);
    a.confirm_clock();
    a.update(FRAME);
    assert!(matches!(a.phase(), Phase::Shelf));
    assert!(logo(&a).is_none(), "the logo came up after the clock");
}

#[test]
fn coming_back_to_the_shelf_never_brings_the_logo_back() {
    let (_d, mut a) = booted_on_the_shelf();
    frames(&mut a, 40);
    a.apply(Action::QuickMenu);
    a.apply(Action::GbaUp(Btn::Start));
    a.finish_slide();
    a.apply(Action::GbaDown(Btn::B));
    a.apply(Action::GbaUp(Btn::B));
    a.finish_slide();
    frames(&mut a, 2);
    assert!(matches!(a.phase(), Phase::Shelf));
    assert_eq!(a.boot_logo_alpha(), None);
    assert!(logo(&a).is_none());
}
