#![cfg(any(target_os = "macos", target_os = "linux"))]

use crate::common;

use std::collections::VecDeque;
use std::path::Path;
use std::time::{Duration, Instant};

use common::{clocked, core_lock, tmp_root_with_gb_carts};
use omega_gfx::{out_h, out_w, Compositor, HeadlessSurface};
use omega_input::{Btn, InputSource, Millis, RawEvent};
use omega_power::SimPlatform;
use omega_store::Platform;
use omega_zero::frontend::Frontend;
use omega_zero::video_mode::{picture_rect, screen, VideoMode};

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

const RED: [u8; 3] = [255, 0, 0];

fn at(px: &[u8], x: usize, y: usize) -> [u8; 3] {
    let o = (y * out_w() as usize + x) * 4;
    [px[o], px[o + 1], px[o + 2]]
}

/// A red frame over the whole canvas with a clear hole at `hole`.
fn write_overlay(root: &Path, hole: [f32; 4]) {
    let (w, h) = (out_w(), out_h());
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let inside =
                fx > hole[0] && fx < hole[0] + hole[2] && fy > hole[1] && fy < hole[1] + hole[3];
            rgba.extend_from_slice(if inside {
                &[0, 0, 0, 0]
            } else {
                &[255, 0, 0, 255]
            });
        }
    }
    let dir = root.join("Overlays/GB");
    std::fs::create_dir_all(&dir).expect("overlay folder");
    let file = std::fs::File::create(dir.join(format!("{w}x{h}.png"))).expect("overlay file");
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .expect("png header")
        .write_image_data(&rgba)
        .expect("png data");
}

fn playing(root: &Path, c: &mut Compositor) -> Vec<u8> {
    let mut f = Frontend::boot(Box::new(SimPlatform::at(root.to_path_buf())));
    f.upload_faces(c);
    let mut input = Script(VecDeque::new());
    input.0.push_back(vec![RawEvent::Down(Btn::A)]);
    f.advance(&mut input);
    input.0.push_back(vec![RawEvent::Up(Btn::A)]);
    let until = Instant::now() + Duration::from_secs(3);
    while Instant::now() < until {
        f.advance(&mut input);
        std::thread::sleep(Duration::from_millis(2));
    }
    f.compose(c);
    c.read_frame()
}

#[test]
fn a_card_overlay_frames_the_game_without_covering_it() {
    let _core = core_lock();
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let picture = picture_rect(Platform::Gb, screen(Platform::Gb, VideoMode::Actual).view);
    let d = tmp_root_with_gb_carts(&["Tetris"]);
    clocked(d.path());
    write_overlay(d.path(), picture);

    let px = playing(d.path(), &mut c);
    assert_eq!(at(&px, 2, 2), RED, "the overlay is not over the corner");
    let (cx, cy) = (
        (picture[0] + picture[2] / 2.0) as usize,
        (picture[1] + picture[3] / 2.0) as usize,
    );
    assert_ne!(at(&px, cx, cy), RED, "the overlay covers the game");
}

#[test]
fn an_overlay_that_would_cover_the_game_is_left_off() {
    let _core = core_lock();
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let picture = picture_rect(Platform::Gb, screen(Platform::Gb, VideoMode::Actual).view);
    let d = tmp_root_with_gb_carts(&["Tetris"]);
    clocked(d.path());
    let small = [
        picture[0] + 40.0,
        picture[1] + 40.0,
        picture[2] - 80.0,
        picture[3] - 80.0,
    ];
    write_overlay(d.path(), small);

    let px = playing(d.path(), &mut c);
    assert_ne!(
        at(&px, picture[0] as usize + 10, picture[1] as usize + 10),
        RED,
        "an overlay over the edge of the game was drawn anyway"
    );
}
