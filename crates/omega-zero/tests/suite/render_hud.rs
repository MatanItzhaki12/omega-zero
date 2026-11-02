#![cfg(any(target_os = "macos", target_os = "linux"))]
//! The HUD as the frontend draws it: the gauge and the toast are their kit faces, in the
//! palette's colours, at two screen pixels to an art pixel, centred on the art grid just under
//! the shelf's top bar, and a gauge's face is drawn again as its cells light.
//!
//! Set SCRATCH_PNG_DIR to a folder to have each frame written there as a PNG.

use crate::common;
use crate::lone::colours;

use common::tmp_root_with_carts;
use omega_gfx::{out_h, out_w, Compositor, HeadlessSurface};
use omega_input::Action;
use omega_power::SimPlatform;
use omega_store::{read_app_state, write_app_state};
use omega_ui::kit::palette::palette;
use omega_ui::{UndoFace, MARGIN, SPRITE_PX, TOP_BAR_H};
use omega_zero::frontend::Frontend;
use tempfile::TempDir;

/// A compositor, and the surface it draws on, which has to outlive it.
fn compositor() -> Option<(HeadlessSurface, Compositor)> {
    let surface = HeadlessSurface::new().ok()?;
    let c = Compositor::new(&surface).ok()?;
    Some((surface, c))
}

/// The frontend on the shelf of a card whose backdrop is `colour`.
fn shelf(c: &mut Compositor, colour: u8) -> (TempDir, Frontend) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut s = read_app_state(d.path());
    s.clock_set = true;
    s.backdrop_colour = colour;
    write_app_state(d.path(), &s).expect("write omega-zero.state");
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(c);
    (d, f)
}

fn frame(f: &mut Frontend, c: &mut Compositor, name: &str) -> Vec<u8> {
    f.compose(c);
    let px = c.read_frame();
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let path = format!("{dir}/hud-{name}-{}x{}.png", out_w(), out_h());
        let face = UndoFace {
            rgba: px.clone(),
            w: out_w(),
            h: out_h(),
        };
        crate::lone::write(&path, &face);
    }
    px
}

/// `face` is in the frame `px` centred across it on the art grid, its top `MARGIN` under the
/// top bar, at two screen pixels to each of its art pixels; its clear corners show whatever
/// is under it.
fn under_the_top_bar(px: &[u8], face: &UndoFace, name: &str) {
    let x0 = (out_w() / SPRITE_PX - face.w) / 2;
    let y0 = (TOP_BAR_H + MARGIN) as u32 / SPRITE_PX;
    let mut off = Vec::new();
    for y in 0..face.h {
        for x in 0..face.w {
            let i = ((y * face.w + x) * 4) as usize;
            if face.rgba[i + 3] == 0 {
                continue;
            }
            let want = [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (sx, sy) = (SPRITE_PX * (x0 + x) + dx, SPRITE_PX * (y0 + y) + dy);
                let o = ((sy * out_w() + sx) * 4) as usize;
                let got = [px[o], px[o + 1], px[o + 2]];
                if got != want {
                    off.push((sx, sy, got, want));
                }
            }
        }
    }
    assert!(
        off.is_empty(),
        "{name}: {} screen pixels are not the face's, first (x, y, drawn, face) {:?}",
        off.len(),
        &off[..off.len().min(6)]
    );
}

#[test]
fn the_volume_gauge_is_its_face_under_the_top_bar_in_night_and_amber() {
    let Some((_surface, mut c)) = compositor() else {
        return;
    };
    for (colour, name) in colours() {
        let (_d, mut f) = shelf(&mut c, colour);
        f.app_mut().apply(Action::VolumeUp);
        let px = frame(&mut f, &mut c, &format!("volume-{name}"));
        let gauge = f.app().hud().gauge().expect("the volume shows no gauge");
        under_the_top_bar(&px, &gauge.face(palette(colour)), name);
    }
}

#[test]
fn the_gauge_is_drawn_again_as_its_cells_light() {
    let Some((_surface, mut c)) = compositor() else {
        return;
    };
    let (_d, mut f) = shelf(&mut c, 0);
    f.app_mut().apply(Action::VolumeDown);
    let before = frame(&mut f, &mut c, "volume-down");
    f.app_mut().apply(Action::VolumeUp);
    f.app_mut().apply(Action::VolumeUp);
    let px = frame(&mut f, &mut c, "volume-up");
    assert_ne!(before, px, "the gauge did not change");
    let gauge = f.app().hud().gauge().expect("no gauge");
    under_the_top_bar(&px, &gauge.face(palette(0)), "volume up");
}

#[test]
fn a_toast_is_its_face_where_the_gauge_stands() {
    let Some((_surface, mut c)) = compositor() else {
        return;
    };
    for (colour, name) in colours() {
        let (_d, mut f) = shelf(&mut c, colour);
        f.app_mut().apply(Action::VolumeUp);
        f.app_mut().apply(Action::ColourCorrectionToggle);
        let px = frame(&mut f, &mut c, &format!("toast-{name}"));
        let said = f.app().toast().expect("the toggle said nothing");
        under_the_top_bar(&px, &said.face(palette(colour)), name);
    }
}
