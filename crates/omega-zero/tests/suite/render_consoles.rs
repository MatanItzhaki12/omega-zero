#![cfg(any(target_os = "macos", target_os = "linux"))]

//! A SNES and a PlayStation picture drawn as the game is: every frame size through the game
//! pass into its system's rect on this canvas, with the console's body round it. With Snes9x or
//! pcsx_rearmed in vendor/ the same is done with a real core's picture. SCRATCH_PNG_DIR keeps
//! what was drawn.

use std::path::Path;

use crate::common::{core_lock, repo_root};

use omega_gfx::{out_h, out_w, Bezel, Compositor, Draw, HeadlessSurface};
use omega_retro::{ButtonMask, LibretroCore, RetroCore};
use omega_store::Platform;
use omega_ui::BEZEL_FILL;
use omega_zero::video_mode::{screen_for, VideoMode};

fn at(px: &[u8], x: usize, y: usize) -> [u8; 3] {
    let o = (y * out_w() as usize + x) * 4;
    [px[o], px[o + 1], px[o + 2]]
}

fn keep(name: &str, rgba: &[u8]) {
    let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") else {
        return;
    };
    let path = format!("{dir}/{name}-{}x{}.png", out_w(), out_h());
    let file = std::fs::File::create(&path).expect("create png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), out_w(), out_h());
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .expect("png header")
        .write_image_data(rgba)
        .expect("png data");
}

/// Draws a `size` picture of `platform` as the game, returning the canvas and the rect it took.
fn draw(
    c: &mut Compositor,
    platform: Platform,
    mode: VideoMode,
    picture: &[u8],
    size: (u32, u32),
) -> (Vec<u8>, [f32; 4]) {
    let screen = screen_for(platform, mode, size);
    c.set_screen_power(1.0);
    c.set_game_view(screen.view);
    c.set_bezel((!screen.bands.is_empty()).then(|| Bezel {
        bands: screen.bands.clone(),
        fill: BEZEL_FILL,
        ..Bezel::default()
    }));
    c.begin_frame();
    c.upload_game_sized(picture, size);
    c.draw_list(&[Draw::Game]);
    (c.read_frame(), screen.view.rect)
}

/// A picture whose every texel tells where it is: red across, green down.
fn located(w: u32, h: u32) -> Vec<u8> {
    let mut bgrx = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            bgrx.extend_from_slice(&[
                0x40,
                (y * 255 / (h - 1)) as u8,
                (x * 255 / (w - 1)) as u8,
                0,
            ]);
        }
    }
    bgrx
}

fn bezel_colour() -> [u8; 3] {
    [0, 1, 2].map(|i| (BEZEL_FILL[i] * 255.0).round() as u8)
}

/// The canvas outside `rect` is the console's body and the picture's corners are its own.
fn assert_framed(px: &[u8], rect: [f32; 4], size: (u32, u32), what: &str) {
    let [x, y, w, h] = rect.map(|v| v as usize);
    let near = |a: [u8; 3], b: [u8; 3]| a.iter().zip(b).all(|(p, q)| p.abs_diff(q) <= 2);
    if x > 0 {
        assert!(
            near(at(px, x - 1, y + h / 2), bezel_colour()),
            "{what}: no body left of the picture"
        );
    }
    if y > 0 {
        assert!(
            near(at(px, x + w / 2, y - 1), bezel_colour()),
            "{what}: no body above the picture"
        );
    }
    let (fw, fh) = size;
    let texel = |tx: u32, ty: u32| {
        [
            (tx * 255 / (fw - 1)) as u8,
            (ty * 255 / (fh - 1)) as u8,
            0x40,
        ]
    };
    assert!(
        near(at(px, x, y), texel(0, 0)),
        "{what}: the top left is not the picture's"
    );
    assert!(
        near(at(px, x + w - 1, y + h - 1), texel(fw - 1, fh - 1)),
        "{what}: the bottom right is {:?}, not the picture's",
        at(px, x + w - 1, y + h - 1)
    );
}

#[test]
fn every_console_picture_fills_its_rect_inside_the_console_s_body() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    for (platform, size) in [
        (Platform::Snes, (256, 224)),
        (Platform::Snes, (512, 224)),
        (Platform::Snes, (512, 448)),
        (Platform::Ps1, (320, 240)),
        (Platform::Ps1, (368, 240)),
        (Platform::Ps1, (640, 480)),
        (Platform::Ps1, (256, 240)),
    ] {
        for mode in [VideoMode::Actual, VideoMode::Stretch] {
            let (px, rect) = draw(&mut c, platform, mode, &located(size.0, size.1), size);
            let what = format!("{platform:?} {mode:?} {size:?}");
            assert_framed(&px, rect, size, &what);
            keep(
                &format!(
                    "console-{}-{}-{}x{}",
                    platform.dir_name(),
                    mode.as_str(),
                    size.0,
                    size.1
                ),
                &px,
            );
        }
    }
}

#[test]
fn a_ps1_picture_at_2x_lands_texel_for_texel() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let (px, rect) = draw(
        &mut c,
        Platform::Ps1,
        VideoMode::Actual,
        &located(320, 240),
        (320, 240),
    );
    if rect[2] != 640.0 {
        return;
    }
    let [x0, y0] = [rect[0] as usize, rect[1] as usize];
    for (tx, ty) in [(0u32, 0u32), (319, 239), (160, 120), (17, 201)] {
        let want = [(tx * 255 / 319) as u8, (ty * 255 / 239) as u8, 0x40];
        for (dx, dy) in [(0, 0), (1, 1)] {
            assert_eq!(
                at(&px, x0 + 2 * tx as usize + dx, y0 + 2 * ty as usize + dy),
                want,
                "texel {tx},{ty} is not exactly 2x"
            );
        }
    }
}

fn vendored(library: &str) -> Option<std::path::PathBuf> {
    let p = omega_retro::host_core(&repo_root().join("vendor"), library);
    p.exists().then_some(p)
}

/// A SNES cart of ours that paints its backdrop red and waits.
fn red_cart(path: &Path) {
    #[rustfmt::skip]
    let code = [
        0x78, 0x18, 0xfb, 0xe2, 0x20, 0xa9, 0x8f, 0x8d, 0x00, 0x21, 0x9c, 0x21, 0x21,
        0xa9, 0x1f, 0x8d, 0x22, 0x21, 0xa9, 0x00, 0x8d, 0x22, 0x21,
        0xa9, 0x0f, 0x8d, 0x00, 0x21, 0x80, 0xfe,
    ];
    let mut rom = vec![0u8; 0x8000];
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"OMEGA ZERO TEST      ");
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 0x05;
    rom[0x7fd9] = 0x01;
    rom[0x7fda] = 0x33;
    for v in (0x7fe4..0x7ffe).step_by(2) {
        rom[v..v + 2].copy_from_slice(&0x8000u16.to_le_bytes());
    }
    rom[0x7fdc..0x7fe0].copy_from_slice(&[0xff, 0xff, 0x00, 0x00]);
    let sum = rom.iter().fold(0u16, |a, &b| a.wrapping_add(u16::from(b)));
    rom[0x7fdc..0x7fde].copy_from_slice(&(!sum).to_le_bytes());
    rom[0x7fde..0x7fe0].copy_from_slice(&sum.to_le_bytes());
    std::fs::write(path, rom).expect("write the cart");
}

#[test]
fn a_real_snes9x_picture_is_drawn_at_twice_its_size() {
    let Some(dylib) = vendored("snes9x") else {
        eprintln!("no snes9x core in vendor/, skipping");
        return;
    };
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let _g = core_lock();
    let d = tempfile::tempdir().expect("tempdir");
    let cart = d.path().join("red.sfc");
    red_cart(&cart);
    let mut core = LibretroCore::open(&dylib).expect("snes9x would not open");
    core.load(&cart).expect("snes9x refused our cart");
    for _ in 0..30 {
        core.run_frame(ButtonMask::default());
        let _ = core.take_audio();
    }
    let size = core.video_size();
    let (px, rect) = draw(
        &mut c,
        Platform::Snes,
        VideoMode::Actual,
        core.video_xrgb8888(),
        size,
    );
    keep("snes9x-actual", &px);
    assert_eq!((rect[2], rect[3]), (512.0, 448.0));
    let [x, y, w, h] = rect.map(|v| v as usize);
    for (px_x, px_y) in [(x, y), (x + w - 1, y + h - 1), (x + w / 2, y + h / 2)] {
        let [r, g, b] = at(&px, px_x, px_y);
        assert!(
            r > 200 && g < 40 && b < 40,
            "{px_x},{px_y} is {r},{g},{b}, not the game's red"
        );
    }
}
