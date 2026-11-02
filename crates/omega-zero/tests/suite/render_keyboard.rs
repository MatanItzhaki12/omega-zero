#![cfg(any(target_os = "macos", target_os = "linux"))]
//! The keyboard as the whole frontend draws it: a modal on the backdrop in the kit palette, as
//! assets/ui/flow.html's #screen=keyboard. With SCRATCH_PNG_DIR set, every frame is written
//! there as keyboard-<name>-<canvas>.png.

use crate::common;

use common::{clocked, tmp_root_with_carts};
use omega_gfx::{out_h, out_w, Compositor, HeadlessSurface};
use omega_input::{Action, Btn};
use omega_power::SimPlatform;
use omega_store::{colour_index, read_app_state, write_app_state, BackdropStyle};
use omega_ui::kit::palette::{palette, Palette};
use omega_ui::{key_box, keyboard_place, Keyboard, Layer, KEY_ROWS};
use omega_zero::app::Phase;
use omega_zero::background_screen::{BackgroundScreen, Sel as BackgroundSel};
use omega_zero::frontend::Frontend;
use omega_zero::nav::Dir;
use omega_zero::wifi_screen::{Typing, WifiScreen};
use tempfile::TempDir;

fn gl() -> Option<(HeadlessSurface, Compositor)> {
    let surface = HeadlessSurface::new().ok()?;
    let compositor = Compositor::new(&surface).ok()?;
    Some((surface, compositor))
}

fn amber() -> u8 {
    colour_index("amber").expect("amber is a backdrop colour")
}

/// The Network screen with its keyboard up for Home, as `keys` has it.
fn typing(keys: Keyboard) -> Phase {
    Phase::Network(WifiScreen {
        typing: Some(Typing {
            ssid: b"Home".to_vec(),
            replace: None,
            keys,
            refused: false,
        }),
        ..WifiScreen::default()
    })
}

/// A frontend on a card whose background is `style` in backdrop colour `colour`, with the
/// passphrase keyboard up as `keys` has it.
fn keyboard_up(
    c: &mut Compositor,
    colour: u8,
    style: BackdropStyle,
    keys: Keyboard,
) -> (TempDir, Frontend) {
    let d = tmp_root_with_carts(&["Emerald"]);
    clocked(d.path());
    let mut s = read_app_state(d.path());
    s.backdrop = Some(style);
    s.backdrop_colour = colour;
    write_app_state(d.path(), &s).expect("write omega-zero.state");
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(c);
    f.app_mut().go(typing(keys), Dir::Deeper);
    (d, f)
}

/// The frame once the keyboard has slid into place.
fn frame(f: &mut Frontend, c: &mut Compositor, name: &str) -> Vec<u8> {
    f.app_mut().finish_slide();
    f.compose(c);
    let px = c.read_frame();
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let path = format!("{dir}/keyboard-{name}-{}x{}.png", out_w(), out_h());
        let file = std::fs::File::create(&path).expect("create png");
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), out_w(), out_h());
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .expect("png header")
            .write_image_data(&px)
            .expect("png data");
    }
    px
}

/// Art pixel (`x`, `y`) of the screen: the colour of its 2x2 block, which must be one colour.
fn art(px: &[u8], x: u32, y: u32) -> [u8; 3] {
    let at = |sx: u32, sy: u32| {
        let o = ((sy * out_w() + sx) * 4) as usize;
        [px[o], px[o + 1], px[o + 2]]
    };
    let c = at(2 * x, 2 * y);
    for (dx, dy) in [(1, 0), (0, 1), (1, 1)] {
        assert_eq!(
            at(2 * x + dx, 2 * y + dy),
            c,
            "art pixel ({x}, {y}) is not one colour"
        );
    }
    c
}

/// Whether any art pixel of `[x, y, w, h]` is `colour`.
fn any_in(px: &[u8], [x, y, w, h]: [u32; 4], colour: [u8; 3]) -> bool {
    (y..y + h).any(|y| (x..x + w).any(|x| art(px, x, y) == colour))
}

/// The screen's bytes over the art rect `[x, y, w, h]`.
fn bytes_of(px: &[u8], [x, y, w, h]: [u32; 4]) -> Vec<u8> {
    let mut out = Vec::new();
    for sy in 2 * y..2 * (y + h) {
        let from = ((sy * out_w() + 2 * x) * 4) as usize;
        out.extend_from_slice(&px[from..from + (2 * w * 4) as usize]);
    }
    out
}

/// A key's rect on the screen in art pixels.
fn key_at(row: usize, col: usize) -> [u32; 4] {
    let [kx, ky, ..] = keyboard_place().keys;
    let [x, y, w, h] = key_box(row, col);
    [kx + x, ky + y, w, h]
}

/// What a frame of the keyboard in `pal` must show, with the cursor on `lit`.
fn holds_the_look(px: &[u8], pal: &Palette, lit: (usize, usize), name: &str) {
    let p = keyboard_place();
    assert_eq!(art(px, 1, 1), pal.bg, "{name}: the backdrop is covered");
    // the frame's outline and bright line, then its panel two pixels up into it: the panel's
    // stripe and the strip beside its title
    let [mx, my, mw, _] = p.modal;
    let edge: Vec<[u8; 3]> = (0..4).map(|dy| art(px, mx + mw / 2, my + dy)).collect();
    assert_eq!(
        edge,
        [pal.dark, pal.frame_hi, pal.stripe, pal.frame_hi],
        "{name}: the modal's frame"
    );
    let title = [mx + p.panel[0] + 6, my + p.panel[1] + 4, 100, 7];
    assert!(any_in(px, title, pal.cream), "{name}: no title");

    let [fx, fy, fw, _] = p.field;
    assert_eq!(art(px, fx, fy + 7), pal.edge, "{name}: the field's edge");
    assert_eq!(art(px, fx + fw - 10, fy + 7), pal.dark, "{name}: the field");

    let [hx, hy] = p.hints;
    assert!(
        any_in(px, [hx, hy + 2, 5, 7], pal.frame_hi),
        "{name}: no B in the hint line"
    );
    assert!(
        any_in(px, [hx + 12, hy + 2, 35, 7], pal.dim),
        "{name}: no DELETE in the hint line"
    );

    for row in 0..KEY_ROWS {
        for col in 0..omega_ui::row_len(row) {
            let [x, y, w, h] = key_at(row, col);
            let is_lit = (row, col) == lit;
            let (face, ink) = if is_lit {
                (pal.frame_hi, pal.dark)
            } else {
                (pal.tab, pal.cream)
            };
            assert_eq!(art(px, x, y), pal.well, "{name}: key {row},{col}'s corner");
            assert_eq!(
                art(px, x, y + 9),
                pal.dark,
                "{name}: key {row},{col}'s outline"
            );
            assert_eq!(
                art(px, x + 1, y + 1),
                face,
                "{name}: key {row},{col}'s face"
            );
            let inside = [x + 1, y + 1, w - 2, h - 2];
            assert!(
                any_in(px, inside, ink),
                "{name}: key {row},{col} has no label"
            );
        }
    }
}

#[test]
fn the_keyboard_is_a_modal_on_the_backdrop_in_the_kit_palette() {
    let Some((_surface, mut c)) = gl() else {
        return;
    };
    for colour in [0, amber()] {
        let keys = Keyboard::default();
        let lit = (keys.row, keys.col);
        let (_d, mut f) = keyboard_up(&mut c, colour, BackdropStyle::Solid, keys);
        let px = frame(&mut f, &mut c, &format!("solid-{colour}"));
        holds_the_look(&px, palette(colour), lit, &format!("colour {colour}"));
        let [fx, fy, ..] = keyboard_place().field;
        let pal = palette(colour);
        let cursor: Vec<[u8; 3]> = (3..12).map(|dy| art(&px, fx + 5, fy + dy)).collect();
        let mut want = vec![pal.dark];
        want.extend([pal.frame_hi; 7]);
        want.push(pal.dark);
        assert_eq!(cursor, want, "the cursor is not a bar the capitals' height");
    }
}

#[test]
fn typing_draws_the_field_again_and_leaves_the_keys_as_they_were() {
    let Some((_surface, mut c)) = gl() else {
        return;
    };
    let (_d, mut f) = keyboard_up(&mut c, 0, BackdropStyle::Solid, Keyboard::default());
    let p = keyboard_place();
    let before = frame(&mut f, &mut c, "empty");
    let keys_before = f.app().keyboard_faces().expect("no faces").keys;
    f.app_mut().apply(Action::GbaDown(Btn::A));
    f.app_mut().apply(Action::GbaDown(Btn::A));
    let after = frame(&mut f, &mut c, "typed-qq");
    assert_eq!(
        f.app().typing().map(|t| t.keys.text.clone()).as_deref(),
        Some("qq")
    );
    assert_eq!(
        bytes_of(&before, p.keys),
        bytes_of(&after, p.keys),
        "typing changed the keys"
    );
    assert_eq!(f.app().keyboard_faces().map(|k| k.keys), Some(keys_before));
    let [fx, fy, ..] = p.field;
    let pal = palette(0);
    assert!(
        any_in(&after, [fx + 5, fy + 4, 12, 7], pal.cream),
        "no qq in the field"
    );
    let cursor: Vec<[u8; 3]> = (4..11)
        .map(|dy| art(&after, fx + 5 + 12, fy + dy))
        .collect();
    assert_eq!(cursor, [pal.frame_hi; 7], "the cursor is not after qq");
}

#[test]
fn the_lit_key_follows_the_cursor_and_the_layer_shows_its_own_keys() {
    let Some((_surface, mut c)) = gl() else {
        return;
    };
    let (_d, mut f) = keyboard_up(&mut c, 0, BackdropStyle::Solid, Keyboard::default());
    let pal = palette(0);
    let p = keyboard_place();
    let lower = frame(&mut f, &mut c, "lower");
    for (press, lit) in [(Btn::Right, (1, 1)), (Btn::Down, (2, 1)), (Btn::Up, (1, 1))] {
        f.app_mut().apply(Action::GbaDown(press));
        let px = frame(&mut f, &mut c, &format!("lit-{}-{}", lit.0, lit.1));
        holds_the_look(&px, pal, lit, &format!("{press:?} to {lit:?}"));
    }
    f.app_mut().apply(Action::GbaDown(Btn::L1));
    assert_eq!(f.app().typing().map(|t| t.keys.layer), Some(Layer::Symbols));
    let symbols = frame(&mut f, &mut c, "symbols");
    holds_the_look(&symbols, pal, (1, 1), "symbols");
    let numbers = [p.keys[0], p.keys[1], p.keys[2], 19];
    assert_ne!(
        bytes_of(&lower, numbers),
        bytes_of(&symbols, numbers),
        "the symbols' first row looks like the digits"
    );
    f.app_mut().apply(Action::GbaDown(Btn::L1));
    f.app_mut().apply(Action::GbaDown(Btn::X));
    let upper = frame(&mut f, &mut c, "upper");
    assert_eq!(
        bytes_of(&lower, numbers),
        bytes_of(&upper, numbers),
        "shift changed the digits"
    );
    let letters = [p.keys[0], p.keys[1] + 22, p.keys[2], 63];
    assert_ne!(
        bytes_of(&lower, letters),
        bytes_of(&upper, letters),
        "shift left the letters small"
    );
}

#[test]
fn a_new_background_colour_tints_the_keyboard_again() {
    let Some((_surface, mut c)) = gl() else {
        return;
    };
    let keys = Keyboard::default();
    let (_d, mut f) = keyboard_up(&mut c, 0, BackdropStyle::Solid, keys.clone());
    frame(&mut f, &mut c, "night");
    let a = f.app_mut();
    a.go(
        Phase::Background(BackgroundScreen {
            sel: BackgroundSel::Colour,
        }),
        Dir::Back,
    );
    a.finish_slide();
    a.apply(Action::GbaDown(Btn::Right));
    let colour = a.palette_colour();
    assert_ne!(colour, 0, "Right did not change the colour");
    a.go(typing(keys.clone()), Dir::Deeper);
    let px = frame(&mut f, &mut c, "retinted");
    holds_the_look(&px, palette(colour), (keys.row, keys.col), "re-tinted");
}

/// The keyboard over the grid, in the states the proof shots compare with flow.html.
#[test]
fn the_keyboard_draws_every_state_flow_html_shows() {
    let Some((_surface, mut c)) = gl() else {
        return;
    };
    let shots = [
        ("night-lower-0-0", 0, Layer::Lower, (0, 0), ""),
        ("amber-lower-4-2", amber(), Layer::Lower, (4, 2), ""),
        ("night-lower-2-5", 0, Layer::Lower, (2, 5), ""),
        ("amber-lower-3-9", amber(), Layer::Lower, (3, 9), ""),
        ("night-symbols-1-3", 0, Layer::Symbols, (1, 3), ""),
        ("amber-symbols-4-1", amber(), Layer::Symbols, (4, 1), ""),
        ("night-upper-1-0", 0, Layer::Upper, (1, 0), "Hunter2 home"),
    ];
    for (name, colour, layer, (row, col), text) in shots {
        let keys = Keyboard {
            text: text.to_string(),
            layer,
            row,
            col,
        };
        let (_d, mut f) = keyboard_up(&mut c, colour, BackdropStyle::Grid, keys);
        let px = frame(&mut f, &mut c, name);
        let [x, y, ..] = key_at(row, col);
        assert_eq!(
            art(&px, x + 1, y + 1),
            palette(colour).frame_hi,
            "{name}: the key under the cursor is not lit"
        );
    }
}

#[test]
fn a_refused_passphrase_says_why_in_the_title_and_lights_the_fields_edge() {
    let Some((_surface, mut c)) = gl() else {
        return;
    };
    let (_d, mut f) = keyboard_up(&mut c, 0, BackdropStyle::Solid, Keyboard::default());
    let p = keyboard_place();
    let pal = palette(0);
    let [mx, my, ..] = p.modal;
    let title = [mx + p.panel[0], my + p.panel[1], p.panel[2], 13];
    let asked = frame(&mut f, &mut c, "asked");
    f.app_mut().apply(Action::GbaDown(Btn::A));
    f.app_mut().apply(Action::GbaDown(Btn::Start));
    assert!(
        f.app().typing().is_some_and(|t| t.refused),
        "q was not refused"
    );
    let refused = frame(&mut f, &mut c, "refused");
    assert_ne!(
        bytes_of(&asked, title),
        bytes_of(&refused, title),
        "the title does not say why"
    );
    let [fx, fy, ..] = p.field;
    assert_eq!(
        art(&refused, fx, fy + 7),
        pal.frame_hi,
        "the field does not say no"
    );
    assert_eq!(
        bytes_of(&asked, p.keys),
        bytes_of(&refused, p.keys),
        "the keys changed"
    );
    f.app_mut().apply(Action::GbaDown(Btn::Y));
    let typing_on = frame(&mut f, &mut c, "typing-on");
    assert_eq!(
        art(&typing_on, fx, fy + 7),
        pal.edge,
        "typing on left it refusing"
    );
    assert_eq!(bytes_of(&asked, title), bytes_of(&typing_on, title));
}
