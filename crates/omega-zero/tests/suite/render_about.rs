#![cfg(target_os = "macos")]

//! About drawn by the whole frontend on a headless GL canvas: its modal (held to flow.html in
//! about.rs) at two screen pixels to an art pixel where every modal goes, over the backdrop, in
//! the palette of the backdrop showing round it, saying what the platform says of its battery.
//!
//! Set SCRATCH_PNG_DIR to a folder to have each frame written there as a PNG.

use crate::common;
use crate::lone::colours;

use std::collections::VecDeque;

use common::{clocked, tmp_root_with_carts};
use omega_gfx::{out_h, out_w, Compositor, HeadlessSurface};
use omega_input::{Btn, InputSource, Millis, RawEvent};
use omega_power::SimPlatform;
use omega_store::{read_app_state, write_app_state, BackdropStyle};
use omega_ui::kit::layout::modal_rect;
use omega_ui::kit::palette::palette;
use omega_ui::{SettingsRow, UndoFace, SPRITE_PX};
use omega_zero::app::Phase;
use omega_zero::frontend::Frontend;
use omega_zero::screens::about::about_face;

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

fn tap(f: &mut Frontend, input: &mut Script, btn: Btn) {
    input.0.push_back(vec![RawEvent::Down(btn)]);
    f.advance(input);
    input.0.push_back(vec![RawEvent::Up(btn)]);
    f.advance(input);
}

/// A frontend on a card with a `colour` grid behind the menus, About opened from Settings.
fn about_over(colour: u8, c: &mut Compositor) -> (tempfile::TempDir, Frontend) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let mut s = read_app_state(d.path());
    s.backdrop = Some(BackdropStyle::Grid);
    s.backdrop_colour = colour;
    write_app_state(d.path(), &s).expect("write omega-zero.state");
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(c);
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::Menu);
    for _ in 0..SettingsRow::About.index() {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    assert!(matches!(f.app().phase(), Phase::About), "A opened no About");
    (d, f)
}

/// The frame once About has slid into place, written to SCRATCH_PNG_DIR if it is set.
fn composed(f: &mut Frontend, c: &mut Compositor, name: &str) -> Vec<u8> {
    f.app_mut().finish_slide();
    f.compose(c);
    let px = c.read_frame();
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let path = format!("{dir}/about-{name}-{}x{}.png", out_w(), out_h());
        let face = UndoFace {
            rgba: px.clone(),
            w: out_w(),
            h: out_h(),
        };
        crate::lone::write(&path, &face);
    }
    px
}

#[test]
fn about_is_its_modal_where_every_modal_goes_in_night_and_amber() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    for (colour, name) in colours() {
        let (_d, mut f) = about_over(colour, &mut c);
        let px = composed(&mut f, &mut c, name);
        let view = f.app().about_view().expect("About is open");
        assert_eq!(
            view.rows[4],
            ("BATTERY", "100% CHARGING".to_string()),
            "the battery is not what the platform says"
        );
        let face = about_face(&view, palette(colour));
        let [x0, y0, w, h] = modal_rect();
        assert_eq!((face.w, face.h), (w, h));
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
            "{name}: {} screen pixels are not the modal's, first (x, y, drawn, face) {:?}",
            off.len(),
            &off[..off.len().min(6)]
        );
    }
}
