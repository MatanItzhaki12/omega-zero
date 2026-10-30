#![cfg(target_os = "macos")]

//! The Bluetooth modal as the frontend draws it, held pixel for pixel to the kit's faces put
//! where flow.html puts them, over a plain backdrop, and to flow.html's own screenshots of it
//! (omega-ui's tests/suite/bluetooth/ref). Set SCRATCH_PNG_DIR to a folder to have each frame
//! written there.

use crate::common;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use common::{clocked, tmp_root_with_carts};
use omega_gfx::{out_h, out_w, Compositor, HeadlessSurface};
use omega_input::{Btn, InputSource, Millis, RawEvent};
use omega_power::SimPlatform;
use omega_store::{colour_index, read_app_state, write_app_state, BackdropStyle};
use omega_ui::kit::layout::modal_rect;
use omega_ui::kit::list_row::{list_row_face, DeviceKind, ListRow};
use omega_ui::kit::palette::{palette, Palette};
use omega_ui::kit::panel::title_face;
use omega_ui::kit::row::{row_face, row_rect, Row, ROW_H};
use omega_ui::kit::text::text_face;
use omega_ui::network::{network_panels, NOTE_IN, NOTE_TOP};
use omega_ui::{SettingsRow, UndoFace};
use omega_zero::bluetooth::{BluetoothJob, BluetoothJobs, BluetoothNews, Device, Status, LOOK_MS};
use omega_zero::frontend::Frontend;
use omega_zero::screens::bluetooth::{
    bluetooth_chrome, bluetooth_title, BLUETOOTH_TITLE, DEVICES_TITLE,
};
use tempfile::TempDir;

const NIGHT: u8 = 0;

fn amber() -> u8 {
    colour_index("amber").expect("amber is a colour")
}

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

/// The ways flow.html shows the modal, each a screenshot of its own. The cursor is on the switch
/// in each but `Pairing`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shot {
    /// On, as the screen opens: the paired devices, and X to look for more.
    On,
    /// Off, the list saying so.
    Off,
    /// On with nothing paired, before any look: X to look, alone.
    Empty,
    /// A look under way, the keyboard found so far.
    Looking,
    /// After the look: flow.html's four devices in the list.
    Found,
    /// After the look, the cursor on the keyboard, which is pairing.
    Pairing,
}

const SHOTS: [Shot; 6] = [
    Shot::On,
    Shot::Off,
    Shot::Empty,
    Shot::Looking,
    Shot::Found,
    Shot::Pairing,
];

impl Shot {
    fn name(self) -> &'static str {
        match self {
            Shot::On => "on",
            Shot::Off => "off",
            Shot::Empty => "empty",
            Shot::Looking => "looking",
            Shot::Found => "found",
            Shot::Pairing => "pairing",
        }
    }
}

/// A radio that comes up when asked, knowing `devices`, and answers nothing else: not a look,
/// not a refresh, not a pairing.
#[derive(Clone)]
struct Radio {
    devices: Vec<Device>,
    news: Arc<Mutex<Vec<BluetoothNews>>>,
}

impl BluetoothJobs for Radio {
    fn present(&self) -> bool {
        true
    }

    fn ask(&mut self, job: BluetoothJob) {
        if job == BluetoothJob::Up {
            let mut news = self.news.lock().unwrap();
            news.push(BluetoothNews::Started(true));
            news.push(BluetoothNews::Status(Status {
                enabled: true,
                powered: true,
                discovering: true,
                devices: self.devices.clone(),
            }));
        }
    }

    fn news(&mut self) -> Vec<BluetoothNews> {
        std::mem::take(&mut self.news.lock().unwrap())
    }
}

/// flow.html's devices: the gamepad connected, the headphones paired, and the keyboard and the
/// band found.
fn flows() -> Vec<Device> {
    let device = |mac: &str, name: &str, kind, paired, connected| Device {
        mac: mac.into(),
        name: name.into(),
        kind,
        paired,
        trusted: paired,
        connected,
    };
    vec![
        device(
            "E4:17:D8:9A:01:22",
            "8BitDo SN30 Pro",
            DeviceKind::Gamepad,
            true,
            true,
        ),
        device(
            "38:18:4C:AB:CD:EF",
            "WH-1000XM4",
            DeviceKind::Audio,
            true,
            false,
        ),
        device(
            "DC:2C:26:01:02:03",
            "Keychron K3",
            DeviceKind::Keyboard,
            false,
            false,
        ),
        device(
            "C8:0F:10:4D:7A:25",
            "Mi Smart Band 4",
            DeviceKind::Other,
            false,
            false,
        ),
    ]
}

struct Rig {
    _d: TempDir,
    f: Frontend,
    input: Script,
}

impl Rig {
    /// The Bluetooth screen in palette `colour`, as `shot` has it.
    fn at(c: &mut Compositor, colour: u8, shot: Shot) -> Rig {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        clocked(d.path());
        let mut s = read_app_state(d.path());
        s.backdrop = Some(BackdropStyle::Solid);
        s.backdrop_colour = colour;
        s.bluetooth = shot != Shot::Off;
        write_app_state(d.path(), &s).expect("write omega-zero.state");
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        // the radio knows the devices found before the screen is opened too: it lists them
        // only once X has looked
        let radio = Radio {
            devices: match shot {
                Shot::Empty => Vec::new(),
                Shot::Looking => flows()[..3].to_vec(),
                _ => flows(),
            },
            news: Arc::default(),
        };
        f.app_mut().set_bluetooth_jobs(Box::new(radio));
        f.upload_faces(c);
        let mut r = Rig {
            _d: d,
            f,
            input: Script(VecDeque::new()),
        };
        r.open_bluetooth();
        if matches!(shot, Shot::Looking | Shot::Found | Shot::Pairing) {
            r.tap(Btn::X);
        }
        if matches!(shot, Shot::Found | Shot::Pairing) {
            // the look over, which the clock would take LOOK_MS to come to
            let app = r.f.app_mut();
            app.tick_ms(app.now() + LOOK_MS);
            assert!(!app.bluetooth().looking(), "the look did not end");
        }
        if shot == Shot::Pairing {
            for _ in 0..3 {
                r.tap(Btn::Down);
            }
            r.tap(Btn::A);
        }
        r
    }

    fn tap(&mut self, btn: Btn) {
        self.input.0.push_back(vec![RawEvent::Down(btn)]);
        self.f.advance(&mut self.input);
        self.input.0.push_back(vec![RawEvent::Up(btn)]);
        self.f.advance(&mut self.input);
    }

    fn open_bluetooth(&mut self) {
        self.tap(Btn::Menu);
        for _ in 0..SettingsRow::ALL.len() {
            if self.f.app().settings_row() == Some(SettingsRow::Bluetooth) {
                break;
            }
            self.tap(Btn::Down);
        }
        assert_eq!(
            self.f.app().settings_row(),
            Some(SettingsRow::Bluetooth),
            "Settings has no Bluetooth row"
        );
        self.tap(Btn::A);
    }

    /// The frame once the screen has slid into place, written out as `name` when asked.
    fn frame(&mut self, c: &mut Compositor, name: &str) -> Vec<u8> {
        self.f.app_mut().finish_slide();
        self.f.compose(c);
        let px = c.read_frame();
        if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
            let path = format!("{dir}/{name}-{}x{}.png", out_w(), out_h());
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
}

fn at(px: &[u8], x: u32, y: u32) -> [u8; 3] {
    let o = ((y * out_w() + x) * 4) as usize;
    [px[o], px[o + 1], px[o + 2]]
}

/// `top` laid over `under` with its top left at (`x`, `y`), wherever it is not clear.
fn lay(under: &mut UndoFace, top: &UndoFace, x: u32, y: u32) {
    for ty in 0..top.h {
        for tx in 0..top.w {
            let from = ((ty * top.w + tx) * 4) as usize;
            if top.rgba[from + 3] == 0 {
                continue;
            }
            let to = (((y + ty) * under.w + x + tx) * 4) as usize;
            under.rgba[to..to + 4].copy_from_slice(&top.rgba[from..from + 4]);
        }
    }
}

/// Every screen pixel the modal `art` covers at 2x, from art pixel `origin`, where `want` says
/// what it should be: the screen pixels that are not, as (x, y, drawn, wanted).
fn off_from(
    px: &[u8],
    art: &UndoFace,
    [ox, oy]: [u32; 2],
    want: impl Fn(u32, u32) -> [u8; 3],
) -> Vec<(u32, u32, [u8; 3], [u8; 3])> {
    let mut off = Vec::new();
    for y in 0..art.h {
        for x in 0..art.w {
            if art.rgba[((y * art.w + x) * 4 + 3) as usize] == 0 {
                continue;
            }
            let want = want(x, y);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (sx, sy) = (2 * (ox + x) + dx, 2 * (oy + y) + dy);
                let got = at(px, sx, sy);
                if got != want {
                    off.push((sx, sy, got, want));
                }
            }
        }
    }
    off
}

/// The frame shows `art` at 2x with its top left at art pixel `origin`: every screen pixel of
/// it, none off.
fn shows(px: &[u8], art: &UndoFace, origin: [u32; 2], name: &str) {
    let off = off_from(px, art, origin, |x, y| {
        let i = ((y * art.w + x) * 4) as usize;
        [art.rgba[i], art.rgba[i + 1], art.rgba[i + 2]]
    });
    assert!(
        off.is_empty(),
        "{name}: {} screen pixels off, first (x, y, drawn, kit) {:?}",
        off.len(),
        &off[..off.len().min(6)]
    );
}

/// A device's row as flow.html draws it: its name, what it is to the handheld, its kind's icon.
fn device<'a>(label: &'a str, value: Option<&'a str>, kind: DeviceKind) -> ListRow<'a> {
    ListRow {
        label,
        value,
        signal: None,
        lock: false,
        device: Some(kind),
        lit: false,
        dim: false,
    }
}

/// The Bluetooth modal as flow.html's `shot` draws it, in `pal`.
fn bluetooth_art(shot: Shot, pal: &Palette) -> UndoFace {
    let on = shot != Shot::Off;
    let switch = Row {
        label: "BLUETOOTH",
        value: Some(if on { "ON" } else { "OFF" }),
        lit: shot != Shot::Pairing,
        carets: true,
        opens: false,
        dim: false,
    };
    let all = vec![
        device("8BitDo SN30 Pro", Some("CONNECTED"), DeviceKind::Gamepad),
        device("WH-1000XM4", Some("PAIRED"), DeviceKind::Audio),
        device("Keychron K3", None, DeviceKind::Keyboard),
        device("Mi Smart Band 4", None, DeviceKind::Other),
    ];
    let mut devices = match shot {
        Shot::On => all[..2].to_vec(),
        Shot::Looking => all[..3].to_vec(),
        Shot::Found | Shot::Pairing => all,
        Shot::Off | Shot::Empty => Vec::new(),
    };
    if shot == Shot::Pairing {
        devices[2].value = Some("PAIRING");
        devices[2].lit = true;
    }
    let notes: &[&str] = match shot {
        Shot::Off => &["Bluetooth is off."],
        Shot::On | Shot::Empty => &["Press X to look for devices."],
        Shot::Looking => &["Looking for devices…"],
        Shot::Found | Shot::Pairing => &[],
    };
    // the panel holding the cursor has its title lit
    let active = (shot == Shot::Pairing) as usize;

    let chrome = bluetooth_chrome(pal);
    let mut art = chrome.face;
    let [tx, ty, _, _] = chrome.titles[active];
    lay(&mut art, &bluetooth_title(active, pal), tx, ty);
    let [x, y, w, _] = row_rect(chrome.wells[0], 0);
    lay(&mut art, &row_face(w, &switch, pal), x, y);
    for (i, row) in devices.iter().enumerate() {
        let [x, y, w, _] = row_rect(chrome.wells[1], i as u32);
        lay(&mut art, &list_row_face(w, row, pal), x, y);
    }
    let [x, y, _, _] = chrome.wells[1];
    let y = y + devices.len() as u32 * ROW_H + NOTE_TOP;
    for (i, note) in notes.iter().enumerate() {
        lay(
            &mut art,
            &text_face(note, pal.dim),
            x + NOTE_IN,
            y + 10 * i as u32,
        );
    }
    art
}

fn compositor() -> Option<(HeadlessSurface, Compositor)> {
    let surface = HeadlessSurface::new().ok()?;
    let c = Compositor::new(&surface).ok()?;
    Some((surface, c))
}

#[test]
fn the_chrome_is_networks_with_one_row_and_its_own_titles() {
    let pal = palette(NIGHT);
    let chrome = bluetooth_chrome(pal);
    let [_, _, w, h] = modal_rect();
    assert_eq!((chrome.face.w, chrome.face.h), (w, h));
    let panels = network_panels(1);
    for (i, well) in chrome.wells.iter().enumerate() {
        let [px, py, pw, ph] = panels[i];
        let [wx, wy, ww, wh] = *well;
        assert!(
            wx > px && wy > py && wx + ww < px + pw && wy + wh < py + ph,
            "well {i} is outside its panel"
        );
    }
    assert_eq!(
        chrome.wells[0][3], ROW_H,
        "BLUETOOTH is as tall as its one row"
    );
    // each title lit is the kit's own, drawn where the chrome left the dim one
    for (i, name) in [BLUETOOTH_TITLE, DEVICES_TITLE].into_iter().enumerate() {
        let lit = bluetooth_title(i, pal);
        let kit = title_face(name, true, pal);
        assert_eq!(
            (lit.w, lit.h, &lit.rgba),
            (kit.w, kit.h, &kit.rgba),
            "{name}"
        );
        assert_eq!([chrome.titles[i][2], chrome.titles[i][3]], [lit.w, lit.h]);
    }
}

#[test]
fn each_way_the_modal_shows_is_the_kits_faces_where_flow_puts_them() {
    let Some((_s, mut c)) = compositor() else {
        return;
    };
    for (colour, tint) in [(NIGHT, "night"), (amber(), "amber")] {
        for shot in SHOTS {
            let name = format!("bluetooth-{}-{tint}", shot.name());
            let mut r = Rig::at(&mut c, colour, shot);
            let px = r.frame(&mut c, &name);
            let art = bluetooth_art(shot, palette(colour));
            let [x, y, _, _] = modal_rect();
            shows(&px, &art, [x, y], &name);
        }
    }
}

/// flow.html's screenshot of `shot` on this canvas, in Night: the modal alone, cropped at
/// `modal_rect()`, one pixel to an art pixel, three bytes a pixel.
fn flow_shot(shot: Shot) -> UndoFace {
    let path = format!(
        "{}/../omega-ui/tests/suite/bluetooth/ref/flow-bluetooth-{}-{}x{}.png",
        env!("CARGO_MANIFEST_DIR"),
        shot.name(),
        out_w(),
        out_h()
    );
    let file = std::fs::File::open(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().expect("a PNG");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("its pixels");
    let channels = info.color_type.samples();
    let rgba = buf[..info.buffer_size()]
        .chunks(channels)
        .flat_map(|px| [px[0], px[1], px[2], 0xff])
        .collect();
    UndoFace {
        rgba,
        w: info.width,
        h: info.height,
    }
}

#[test]
fn the_modal_is_flows_screenshot_of_it() {
    let Some((_s, mut c)) = compositor() else {
        return;
    };
    for shot in SHOTS {
        let name = format!("flow-bluetooth-{}", shot.name());
        let flow = flow_shot(shot);
        let [mx, my, mw, mh] = modal_rect();
        assert_eq!((flow.w, flow.h), (mw, mh), "{name} is not the modal's size");
        let mut r = Rig::at(&mut c, NIGHT, shot);
        let px = r.frame(&mut c, &name);
        // where the modal's face is opaque: its cut corners show each one's own backdrop
        let face = bluetooth_art(shot, palette(NIGHT));
        let off = off_from(&px, &face, [mx, my], |x, y| {
            let i = ((y * flow.w + x) * 4) as usize;
            [flow.rgba[i], flow.rgba[i + 1], flow.rgba[i + 2]]
        });
        assert!(
            off.is_empty(),
            "{name}: {} screen pixels differ from flow.html's, first (x, y, ours, flow's) {:?}",
            off.len(),
            &off[..off.len().min(8)]
        );
    }
}
