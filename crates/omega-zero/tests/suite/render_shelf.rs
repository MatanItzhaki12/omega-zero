#![cfg(any(target_os = "macos", target_os = "linux"))]

use crate::common;

use std::sync::{Mutex, MutexGuard, PoisonError};

use omega_gfx::{out_h, out_w, Compositor, Draw, HeadlessSurface, TexId};
use omega_input::{Action, Btn};
use omega_store::{write_app_state, AppState, Core, Platform};
use omega_ui::{
    board_face, cart_face, cart_shadow, chip_face, chip_shadow_face, clean_label, gb_cart_shadow,
    label_colour, lid_shadow_face, padded, shelf_box, socket_face, GbShell, TURN_PAD,
};
use omega_zero::app::{App, Phase};
use tempfile::TempDir;

static GL: Mutex<()> = Mutex::new(());

fn compositor() -> Option<(MutexGuard<'static, ()>, HeadlessSurface, Compositor)> {
    let guard = GL.lock().unwrap_or_else(PoisonError::into_inner);
    let surface = HeadlessSurface::new().ok()?;
    let compositor = Compositor::new(&surface).ok()?;
    Some((guard, surface, compositor))
}

fn tex(c: &mut Compositor, w: u32, h: u32, rgba: &[u8]) -> TexId {
    c.create_texture(w, h, rgba)
}

fn upload_faces(app: &mut App, c: &mut Compositor) {
    let row: Vec<TexId> = app
        .carts()
        .map(|cart| {
            let f = cart_face(cart);
            tex(c, f.w, f.h, &f.rgba)
        })
        .collect();
    app.set_faces(row);
    let shadow = cart_shadow();
    let shadow = tex(c, shadow.w, shadow.h, &shadow.rgba);
    app.set_cart_shadow(shadow);
    let notched = gb_cart_shadow(GbShell::Notched);
    let notched = tex(c, notched.w, notched.h, &notched.rgba);
    let rounded = gb_cart_shadow(GbShell::Rounded);
    let rounded = tex(c, rounded.w, rounded.h, &rounded.rgba);
    app.set_gb_cart_shadows(notched, rounded);

    let sockets = Core::for_platform(omega_store::Platform::Gba)
        .iter()
        .map(|k| {
            let f = socket_face(*k);
            tex(c, f.w, f.h, &f.rgba)
        })
        .collect();
    let chips = Core::ALL
        .iter()
        .map(|k| {
            let f = chip_face(Some(*k));
            tex(c, f.w, f.h, &f.rgba)
        })
        .collect();
    let blank = chip_face(None);
    let blank = tex(c, blank.w, blank.h, &blank.rgba);
    let chip_shadow = chip_shadow_face();
    let chip_shadow = tex(c, chip_shadow.w, chip_shadow.h, &chip_shadow.rgba);
    let lid_shadow = lid_shadow_face();
    let lid_shadow = tex(c, lid_shadow.w, lid_shadow.h, &lid_shadow.rgba);
    app.set_core_part_faces(sockets, chips, blank, [chip_shadow, lid_shadow]);

    let highlighted = app.selected_stem().map(str::to_string);
    let Some(cart) = app
        .carts()
        .find(|c| highlighted.as_deref() == Some(c.stem.as_str()))
        .cloned()
    else {
        return;
    };
    let board = board_face(&cart);
    let board = tex(c, board.w, board.h, &board.rgba);
    let lid = padded(&cart_face(&cart), TURN_PAD);
    let lid = tex(c, lid.w, lid.h, &lid.rgba);
    app.set_core_board_faces(board, lid);
}

fn shelf(d: &TempDir, c: &mut Compositor) -> App {
    write_app_state(
        d.path(),
        &AppState {
            clock_set: true,
            ..Default::default()
        },
    )
    .unwrap();
    let mut app = App::boot(d.path());
    upload_faces(&mut app, c);
    app
}

fn shot(app: &App, c: &mut Compositor, name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    app.draw(&mut out);
    shot_of(&out, c, name)
}

fn shot_of(out: &[Draw], c: &mut Compositor, name: &str) -> Vec<u8> {
    c.begin_frame();
    c.draw_list(out);
    let px = c.read_frame();
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let path = format!("{dir}/shelf-{name}.png");
        let file = std::fs::File::create(&path).expect("create png");
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), out_w(), out_h());
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .expect("png header")
            .write_image_data(&px)
            .expect("png data");
        println!("wrote {path}");
    }
    px
}

fn let_it_hop(app: &mut App) {
    app.update(0.25);
}

/// How many pixels of two shots differ by more than a GPU's noise. The selected cart sways, and
/// the GL on CI does not draw a turning face the same way twice, so two shots of the same shelf
/// need not match to the byte; anything drawn over them changes far more.
fn changed(a: &[u8], b: &[u8]) -> usize {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > 32))
        .count()
}

fn screen_px() -> usize {
    (out_w() * out_h()) as usize
}

#[test]
fn x_draws_the_plain_shelf_on_a_game_boy_cart() {
    let Some((_g, _s, mut c)) = compositor() else {
        eprintln!("no GL on this host, skipping");
        return;
    };

    let d = common::tmp_root_with_gb_carts(&["Tetris", "Zzz"]);
    let twin = common::tmp_root_with_gb_carts(&["Tetris", "Zzz"]);
    let mut pressed = shelf(&d, &mut c);
    let mut untouched = shelf(&twin, &mut c);
    pressed.apply(Action::GbaDown(Btn::X));
    let_it_hop(&mut pressed);
    let_it_hop(&mut untouched);
    let plain = shot(&untouched, &mut c, "gb-plain");
    let after = shot(&pressed, &mut c, "gb-after-x");
    assert_eq!(
        pressed.core_picker(),
        None,
        "a Game Boy cart opened the GBA picker"
    );
    let n = changed(&after, &plain);
    assert!(
        n < screen_px() / 1000,
        "X put something on screen over a Game Boy cart: {n} pixels changed"
    );

    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    let twin = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    let mut pressed = shelf(&d, &mut c);
    let mut untouched = shelf(&twin, &mut c);
    pressed.apply(Action::GbaDown(Btn::X));
    let_it_hop(&mut pressed);
    let_it_hop(&mut untouched);
    let plain = shot(&untouched, &mut c, "gba-plain");
    let after = shot(&pressed, &mut c, "gba-after-x");
    assert_eq!(
        pressed.core_picker(),
        Some(Core::Mgba),
        "X stopped opening the picker"
    );
    let n = changed(&after, &plain);
    assert!(
        n > screen_px() / 100,
        "the picker opened and the panel showed the same shelf: {n} pixels changed"
    );
}

/// A card with a Game Boy Advance Tetris and a Game Boy one resumes the one its app state names,
/// and the one that comes back out when the game is left is that cartridge: a pale pak at the
/// size the Game Boy shelf holds it, or the dark Game Boy Advance cart, its label showing on
/// both. A card with no cart_platform line resumes the Game Boy Advance cart.
#[test]
fn the_card_says_which_tetris_comes_back_out() {
    let Some((_g, _s, mut c)) = compositor() else {
        eprintln!("no GL on this host, skipping");
        return;
    };
    let d = common::tmp_root_with_carts(&["Tetris", "Emerald"]);
    common::write_gb_cart(&d, "Tetris", "TETRIS");

    let (gba, gba_at) = ejected_shot(&d, &mut c, Some(Platform::Gba), "eject-gba");
    let (gb, gb_at) = ejected_shot(&d, &mut c, Some(Platform::Gb), "eject-gb");
    let (unstated, unstated_at) = ejected_shot(&d, &mut c, None, "eject-unstated");

    for (name, at, platform) in [
        ("Game Boy Advance", gba_at, Platform::Gba),
        ("Game Boy", gb_at, Platform::Gb),
    ] {
        let (w, h) = shelf_box(platform);
        assert!(
            (at[2] - w).abs() < 0.5 && (at[3] - h).abs() < 0.5,
            "the {name} Tetris came out {}x{}, not the {w}x{h} its shelf holds it at",
            at[2],
            at[3]
        );
    }

    let ink = label_colour(&clean_label("Tetris"));
    let (dark, pale) = (shell(&gba, gba_at, ink), shell(&gb, gb_at, ink));
    assert!(
        (0..3).all(|k| pale[k] as i32 - dark[k] as i32 > 40),
        "the card named the Game Boy shelf and the cart coming out reads {pale:?} where the \
         Game Boy Advance cartridge reads {dark:?}: the wrong cartridge came back"
    );
    for (name, px) in [("Game Boy Advance", &gba), ("Game Boy", &gb)] {
        assert!(
            paper(px, ink) > 200,
            "the {name} cartridge came out without its label showing"
        );
    }

    assert_eq!(unstated_at, gba_at);
    // to a GPU's noise: the first frame a software renderer draws can sit a shade off the rest
    assert!(
        changed(&unstated, &gba) == 0,
        "a card with no cart_platform line did not resume the Game Boy Advance cartridge"
    );
}

/// The card resumed in Tetris, left as it boots: a shot of the cart on its way back out, all of
/// it on the screen, and where its face is drawn.
fn ejected_shot(
    d: &TempDir,
    c: &mut Compositor,
    platform: Option<Platform>,
    name: &str,
) -> (Vec<u8>, [f32; 4]) {
    write_app_state(
        d.path(),
        &AppState {
            cart: Some("Tetris".into()),
            cart_platform: platform,
            clock_set: true,
            ..Default::default()
        },
    )
    .unwrap();
    let mut app = App::boot(d.path());
    upload_faces(&mut app, c);
    assert!(
        matches!(app.phase(), Phase::Inserting { resumed: true, .. }),
        "the card did not resume its cart: {:?}",
        app.phase()
    );
    assert_eq!(app.seat(), 1.0, "a resumed cart is not all the way in");
    let mut out = Vec::new();
    app.draw(&mut out);
    assert_eq!(
        cart_drawn(&out),
        None,
        "a resumed cart is drawn on the screen though it is in the handheld"
    );
    app.apply(Action::Eject);
    while app.seat() > 0.15 {
        app.update(1.0 / 60.0);
        assert!(app.now() < 5_000, "the cart never came out");
    }
    assert!(matches!(app.phase(), Phase::Ejecting { .. }));
    let mut out = Vec::new();
    app.draw(&mut out);
    let at = cart_drawn(&out).expect("no cart on its way out");
    assert!(
        at[1] + at[3] < out_h() as f32,
        "the cart is not all on the screen: {at:?}"
    );
    (shot_of(&out, c, name), at)
}

/// Where the last cart's face in `out` is drawn, flat, turned or spun: the one on its way in or
/// out, drawn over the shelf.
fn cart_drawn(out: &[Draw]) -> Option<[f32; 4]> {
    out.iter().rev().find_map(|d| match *d {
        Draw::Tex { x, y, w, h, .. }
        | Draw::Turned { x, y, w, h, .. }
        | Draw::Spun {
            x,
            y,
            w,
            h,
            depth: 0.0,
            ..
        } if w > 150.0 && h > 150.0 && w < out_w() as f32 => Some([x, y, w, h]),
        _ => None,
    })
}

/// The middle of what is drawn of a cart's face at `at` but its label: its shell's colour,
/// whatever shows round its corners.
fn shell(px: &[u8], at: [f32; 4], ink: [u8; 3]) -> [u8; 3] {
    let [x, y, w, h] = at.map(|v| v.max(0.0) as usize);
    let mut seen: Vec<[u8; 3]> = (y..(y + h).min(out_h() as usize))
        .flat_map(|row| (x..(x + w).min(out_w() as usize)).map(move |col| (col, row)))
        .map(|(col, row)| {
            let o = (row * out_w() as usize + col) * 4;
            [px[o], px[o + 1], px[o + 2]]
        })
        .filter(|p| !(0..3).all(|k| p[k].abs_diff(ink[k]) <= 24))
        .collect();
    seen.sort_by_key(|p| p.iter().map(|v| *v as u32).sum::<u32>());
    seen[seen.len() / 2]
}

fn paper(px: &[u8], ink: [u8; 3]) -> usize {
    px.chunks(4)
        .filter(|p| (0..3).all(|k| p[k].abs_diff(ink[k]) <= 24))
        .count()
}
