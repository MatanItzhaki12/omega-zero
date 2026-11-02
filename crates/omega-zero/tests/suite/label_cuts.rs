#![cfg(any(target_os = "macos", target_os = "linux"))]

//! Label scans are cut to their faces once, and every face is drawn from its label off the main
//! thread: a boot comes up with every cart in its generated face, decoding no label, and the
//! label workers draw each cart that wears one in it after, cutting the scans with no cut first.
//! The next boot draws them from the cuts, and a label a sync saves is drawn the same way.

use crate::common;

use std::collections::VecDeque;
use std::path::Path;
use std::time::{Duration, Instant};

use common::{clocked, tmp_root_with_carts};
use omega_gfx::{Compositor, HeadlessSurface};
use omega_input::{InputSource, Millis, RawEvent};
use omega_power::SimPlatform;
use omega_store::Platform;
use omega_ui::label_uncut;
use omega_zero::frontend::Frontend;
use omega_zero::label_sync::{LabelJob, LabelJobs, LabelNews};

/// A colour no generated label and no backdrop is drawn in.
const MAGENTA: [u8; 3] = [0xff, 0x00, 0xff];

struct Idle;

impl InputSource for Idle {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        Vec::new()
    }
}

/// A ScreenScraper that has just saved the labels it is given.
struct Saved(VecDeque<LabelNews>);

impl LabelJobs for Saved {
    fn present(&self) -> bool {
        true
    }
    fn ask(&mut self, _job: LabelJob) {}
    fn news(&mut self) -> Vec<LabelNews> {
        self.0.drain(..).collect()
    }
}

fn write_scan(root: &Path, stem: &str) {
    let (w, h) = (600, 310);
    let rgb: Vec<u8> = (0..w * h).flat_map(|_| MAGENTA).collect();
    let dir = root.join("Labels/GBA");
    std::fs::create_dir_all(&dir).expect("labels dir");
    let f = std::fs::File::create(dir.join(format!("{stem}.png"))).expect("create scan");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .expect("png header")
        .write_image_data(&rgb)
        .expect("png data");
}

/// How many of the frame's pixels are the label's magenta, lit or shaded.
fn magenta_px(frame: &[u8]) -> usize {
    frame
        .chunks_exact(4)
        .filter(|p| p[0] > 110 && p[2] > 110 && p[1] < 40 && p[0].abs_diff(p[2]) < 40)
        .count()
}

/// The shelf drawn with the faces as they are uploaded now, without taking in the faces the
/// label workers have handed back since, as compose would.
fn drawn_as_uploaded(f: &Frontend, c: &mut Compositor) -> Vec<u8> {
    let mut draws = Vec::new();
    f.app().draw(&mut draws);
    c.begin_frame();
    c.draw_list(&draws);
    c.read_frame()
}

fn png_width(path: &Path) -> u32 {
    let file = std::fs::File::open(path).expect("open scan");
    png::Decoder::new(file)
        .read_info()
        .expect("png header")
        .info()
        .width
}

/// Composes frames until `done`, for up to ten seconds; false if it never was.
fn compose_until(
    f: &mut Frontend,
    c: &mut Compositor,
    mut done: impl FnMut(&mut Frontend, &[u8]) -> bool,
) -> bool {
    let began = Instant::now();
    while began.elapsed() < Duration::from_secs(10) {
        f.advance(&mut Idle);
        f.app_mut().finish_slide();
        f.compose(c);
        if done(f, &c.read_frame()) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

#[test]
fn a_boot_cuts_the_scans_it_finds_uncut_and_the_next_boot_draws_from_the_cuts() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    write_scan(d.path(), "Emerald");
    let cut = d.path().join("Labels/GBA/.cut/Emerald.png");

    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    assert!(
        compose_until(&mut f, &mut c, |_, px| cut.is_file()
            && magenta_px(px) > 500),
        "the scan was never cut and its cart never drawn in it"
    );
    drop(f);

    let made = std::fs::metadata(&cut).and_then(|m| m.modified()).unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    assert!(
        f.app().carts().all(|c| !label_uncut(c)),
        "the second boot found a scan to cut"
    );
    f.upload_faces(&mut c);
    assert!(
        compose_until(&mut f, &mut c, |_, px| magenta_px(px) > 500),
        "the second boot did not draw the cart from its cut"
    );
    assert_eq!(
        std::fs::metadata(&cut).and_then(|m| m.modified()).unwrap(),
        made,
        "the second boot cut the scan again"
    );
}

#[test]
fn a_label_a_sync_saves_is_cut_and_its_cart_drawn_in_it() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.app_mut().finish_slide();
    f.compose(&mut c);
    assert_eq!(
        magenta_px(&c.read_frame()),
        0,
        "a magenta cart before any label"
    );

    write_scan(d.path(), "Emerald");
    f.app_mut()
        .set_label_jobs(Box::new(Saved(VecDeque::from([LabelNews::Saved {
            platform: Platform::Gba,
            stem: "Emerald".into(),
        }]))));
    let cut = d.path().join("Labels/GBA/.cut/Emerald.png");
    assert!(
        compose_until(&mut f, &mut c, |_, px| cut.is_file()
            && magenta_px(px) > 500),
        "the saved label was never cut and its cart never drawn in it"
    );
}

#[test]
fn a_boot_shrinks_the_scans_cut_before_and_leaves_their_cuts_as_they_were() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    write_scan(d.path(), "Emerald");
    // cut as the release before scans were shrunk cut it, the scan left whole
    let carts = omega_store::scan(d.path()).expect("scan the card");
    let emerald = carts.iter().find(|c| c.stem == "Emerald").unwrap();
    assert!(omega_ui::cut_label(emerald));
    let scan = d.path().join("Labels/GBA/Emerald.png");
    let cut = d.path().join("Labels/GBA/.cut/Emerald.png");
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).unwrap();
    let (made, scanned) = (modified(&cut), modified(&scan));

    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    assert!(
        compose_until(&mut f, &mut c, |_, px| magenta_px(px) > 500
            && png_width(&scan) < 600),
        "the scan was never shrunk, or its cart never drawn from its cut"
    );
    assert_eq!(modified(&cut), made, "the boot cut the scan again");
    assert_eq!(modified(&scan), scanned, "the shrunk scan lost its time");
    assert!(f.app().carts().all(|c| !label_uncut(c)));
}

/// A card booted before, its scans cut and shrunk: the boot uploads every cart in its generated
/// face, the cut cart's included, and it is the label workers that draw that cart in its label.
#[test]
fn a_boot_decodes_no_label_and_the_workers_draw_the_cut_cart_in_its_label() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    write_scan(d.path(), "Emerald");
    let carts = omega_store::scan(d.path()).expect("scan the card");
    let emerald = carts.iter().find(|c| c.stem == "Emerald").unwrap();
    assert!(omega_ui::cut_label(emerald));
    omega_ui::shrink_label(emerald);

    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.app_mut().finish_slide();
    assert!(
        !f.settled(),
        "settled with the cart's label still to be drawn"
    );
    assert_eq!(
        magenta_px(&drawn_as_uploaded(&f, &mut c)),
        0,
        "the boot drew the cart from its cut itself"
    );
    assert!(
        compose_until(&mut f, &mut c, |f, _| f.settled()),
        "the label workers never handed the cart's face back"
    );
    assert!(
        magenta_px(&drawn_as_uploaded(&f, &mut c)) > 500,
        "settled without the cart in its label"
    );
}

/// A label a sync saves that has a cut already, which the frame it arrives in used to draw on
/// the main thread, is drawn by the label workers too.
#[test]
fn a_label_a_sync_saves_cut_already_is_drawn_by_the_workers() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    assert!(
        compose_until(&mut f, &mut c, |f, px| f.settled() && magenta_px(px) == 0),
        "the boot never settled"
    );

    write_scan(d.path(), "Emerald");
    let carts = omega_store::scan(d.path()).expect("scan the card");
    assert!(omega_ui::cut_label(
        carts.iter().find(|c| c.stem == "Emerald").unwrap()
    ));
    f.app_mut()
        .set_label_jobs(Box::new(Saved(VecDeque::from([LabelNews::Saved {
            platform: Platform::Gba,
            stem: "Emerald".into(),
        }]))));
    let mut asked = None;
    assert!(
        compose_until(&mut f, &mut c, |f, px| {
            asked = Some(magenta_px(px));
            f.drawing_labels()
        }),
        "the saved label was never handed to the label workers"
    );
    assert_eq!(
        asked,
        Some(0),
        "the frame the label arrived in drew it on the main thread"
    );
    assert!(!f.settled(), "settled with the label still being drawn");
    assert!(
        compose_until(&mut f, &mut c, |f, px| f.settled() && magenta_px(px) > 500),
        "the workers never drew the cart in its saved label"
    );
}

/// The faces the label workers hand back in a burst are uploaded a few a frame rather than all in
/// the frame they came back by, and the workers hold no more of them than can wait.
#[test]
fn a_burst_of_faces_is_uploaded_over_several_frames() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let stems: Vec<String> = (0..12).map(|i| format!("Cart {i:02}")).collect();
    let names: Vec<&str> = stems.iter().map(String::as_str).collect();
    let d = tmp_root_with_carts(&names);
    clocked(d.path());
    for stem in &stems {
        write_scan(d.path(), stem);
    }
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    // a scan is shrunk once its face is handed back; eight faces can wait to be taken, and the
    // workers with one more wait for room
    let scan = |stem: &str| d.path().join(format!("Labels/GBA/{stem}.png"));
    let shrunk = || stems.iter().filter(|s| png_width(&scan(s)) < 600).count();
    let began = Instant::now();
    while shrunk() < 8 {
        assert!(
            began.elapsed() < Duration::from_secs(10),
            "the workers never drew the faces that can wait"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        shrunk(),
        8,
        "the workers drew on with no room for their faces"
    );
    f.compose(&mut c);
    assert!(
        f.drawing_labels(),
        "every face waiting was uploaded in one frame"
    );
    assert!(compose_until(&mut f, &mut c, |f, _| !f.drawing_labels()));
}
