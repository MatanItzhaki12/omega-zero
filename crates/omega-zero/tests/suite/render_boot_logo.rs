#![cfg(any(target_os = "macos", target_os = "linux"))]
//! The boot logo as the frontend draws it: the first frame of a boot onto the shelf is the logo
//! omega-zero ships for the canvas, pixel for pixel, and once it has faded the frame is the shelf
//! a boot without the logo draws.

use crate::common;

use std::path::Path;

use common::tmp_root_with_carts;
use omega_gfx::{out_h, out_w, Compositor, HeadlessSurface};
use omega_input::{InputSource, Millis, RawEvent};
use omega_power::SimPlatform;
use omega_store::{write_app_state, AppState};
use omega_ui::UndoFace;
use omega_zero::bootlogo::shown;
use omega_zero::frontend::Frontend;
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

/// The logo omega-zero ships on the card for this canvas.
fn shipped() -> UndoFace {
    let system = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../card/System");
    shown(&system, (out_w(), out_h())).expect("no logo shipped for this canvas")
}

/// A boot onto the shelf, at the time the screens example draws.
fn boot(c: &mut Compositor) -> (TempDir, Frontend) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let state = AppState {
        clock_set: true,
        ..AppState::default()
    };
    write_app_state(d.path(), &state).unwrap();
    let platform = SimPlatform::at(d.path().to_path_buf()).stopped_at(1_790_784_000);
    let mut f = Frontend::boot(Box::new(platform));
    f.upload_faces(c);
    (d, f)
}

/// Frame `n` at 60 fps.
fn frame(f: &mut Frontend, c: &mut Compositor, n: u64) -> Vec<u8> {
    f.advance_at(&mut Still, n * 1000 / 60, 1.0 / 60.0);
    f.compose(c);
    c.read_frame()
}

/// How many pixels of `a` and `b` differ by more than a GPU's noise.
fn changed(a: &[u8], b: &[u8]) -> usize {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(p, q)| p[..3].iter().zip(&q[..3]).any(|(x, y)| x.abs_diff(*y) > 32))
        .count()
}

#[test]
fn the_first_frame_is_the_shipped_logo_pixel_for_pixel_and_then_the_shelf() {
    let Some((_s, mut c)) = compositor() else {
        eprintln!("no GL on this host, skipping");
        return;
    };
    let logo = shipped();
    let (_d, mut faded) = boot(&mut c);
    faded.show_boot_logo(&mut c, &logo);
    let first = frame(&mut faded, &mut c, 1);
    let opaque: Vec<u8> = first
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2], 0xff])
        .collect();
    assert!(opaque == logo.rgba, "the first frame is not the logo");
    for n in 2..=32 {
        frame(&mut faded, &mut c, n);
    }
    assert_eq!(
        faded.app().boot_logo_alpha(),
        None,
        "the logo outlived its fade"
    );
    let after = frame(&mut faded, &mut c, 33);

    let (_e, mut plain) = boot(&mut c);
    let mut shelf = Vec::new();
    for n in 1..=33 {
        shelf = frame(&mut plain, &mut c, n);
    }
    let left = changed(&after, &shelf);
    assert!(
        left < (out_w() * out_h() / 100) as usize,
        "{left} pixels still differ from the shelf once the logo has faded"
    );
    assert!(changed(&after, &logo.rgba) > left);
}
