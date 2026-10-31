#![cfg(target_os = "macos")]
//! The opening as the frontend draws it: its lines in X5's mint on black, where the terminal
//! stands, and the shelf once it is over.

use crate::common;

use common::tmp_root_with_carts;
use omega_gfx::{out_h, out_w, Compositor, HeadlessSurface};
use omega_input::{InputSource, Millis, RawEvent};
use omega_power::SimPlatform;
use omega_store::{write_app_state, AppState};
use omega_zero::frontend::Frontend;
use omega_zero::opening::Facts;
use tempfile::TempDir;

struct Still;

impl InputSource for Still {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        Vec::new()
    }
}

fn compositor() -> Option<(HeadlessSurface, Compositor)> {
    let surface = HeadlessSurface::new().ok()?;
    let c = Compositor::new(&surface).ok()?;
    Some((surface, c))
}

fn boot(c: &mut Compositor) -> (TempDir, Frontend) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let state = AppState {
        clock_set: true,
        boot_animation: true,
        ..AppState::default()
    };
    write_app_state(d.path(), &state).unwrap();
    let platform = SimPlatform::at(d.path().to_path_buf()).stopped_at(1_790_784_000);
    let mut f = Frontend::boot(Box::new(platform));
    f.upload_faces(c);
    f.start_opening(&Facts::default());
    (d, f)
}

/// Frame `n` at 60 fps.
fn frame(f: &mut Frontend, c: &mut Compositor, n: u64) -> Vec<u8> {
    f.advance_at(&mut Still, n * 1000 / 60, 1.0 / 60.0);
    f.compose(c);
    c.read_frame()
}

fn rgb(frame: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = ((y * out_w() + x) * 4) as usize;
    [frame[i], frame[i + 1], frame[i + 2]]
}

fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 2)
}

#[test]
fn the_terminal_is_x5s_mint_on_black_and_then_the_shelf() {
    let Some((_s, mut c)) = compositor() else {
        eprintln!("no GL on this host, skipping");
        return;
    };
    let (_d, mut f) = boot(&mut c);
    let first = frame(&mut f, &mut c, 1);
    assert!(
        first.chunks_exact(4).all(|p| p[..3] == [0, 0, 0]),
        "the first frame is not black"
    );
    let mut lit = first;
    for n in 2..=600 {
        lit = frame(&mut f, &mut c, n);
    }
    assert_eq!(f.app().opening_levels(), Some((1.0, 1.0)));
    let x = ((out_w() - 576) / 2) & !1;
    let y = ((out_h() - 396) / 2) & !1;
    // the O of OMEGA: its left stem, mint at the capitals' top, pale lower down
    assert!(
        near(rgb(&lit, x + 1, y + 2), [161, 228, 208]),
        "{:?}",
        rgb(&lit, x + 1, y + 2)
    );
    assert!(
        near(rgb(&lit, x + 1, y + 6), [214, 238, 232]),
        "{:?}",
        rgb(&lit, x + 1, y + 6)
    );
    assert_eq!(rgb(&lit, x + 6, y + 6), [0, 0, 0], "the O is not hollow");
    assert_eq!(rgb(&lit, out_w() - 4, out_h() - 4), [0, 0, 0]);
    let inked = lit.chunks_exact(4).filter(|p| p[..3] != [0, 0, 0]).count();
    let all = (out_w() * out_h()) as usize;
    assert!(
        inked > all / 50 && inked < all / 4,
        "{inked} of {all} pixels lit: not sixteen lines of letters"
    );
    let mut after = lit;
    for n in 601..=700 {
        after = frame(&mut f, &mut c, n);
    }
    assert!(!f.app().opening_shown(), "still playing at 11.6 s");
    let black = after
        .chunks_exact(4)
        .filter(|p| p[..3] == [0, 0, 0])
        .count();
    assert!(
        black < all / 2,
        "{black} of {all} pixels still black: no shelf"
    );
}
