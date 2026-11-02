//! flow.html's Bluetooth screen (#screen=bluetooth), as bluetooth/make-refs.sh screenshots it:
//! the modal alone at one pixel to an art pixel, in Night, at each canvas. Here it is put
//! together from the kit's faces where Network's layout puts them, and held to the screenshots
//! pixel for pixel: BLUETOOTH as tall as its one row and DEVICES a pixel over it down to the
//! frame (`network_panels(1)`), the devices' rows from DEVICES' well's top, and a note `NOTE_IN`
//! in and `NOTE_TOP` under the rows. The frontend's Bluetooth screen is drawn the same way.

use omega_gfx::{out_h, out_w};
use omega_ui::kit::layout::modal_rect;
use omega_ui::kit::list_row::{list_row_face, DeviceKind, ListRow};
use omega_ui::kit::modal::modal_face;
use omega_ui::kit::palette::{palette, Palette};
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::row::{row_face, row_rect, Row, ROW_H};
use omega_ui::kit::text::text_face;
use omega_ui::network::{network_panels, NOTE_IN, NOTE_TOP};
use omega_ui::UndoFace;

const NIGHT: u8 = 0;

/// The panels' titles, top first.
const TITLES: [&str; 2] = ["BLUETOOTH", "DEVICES"];

/// flow.html's devices: a name, what it is to the handheld, and its kind. The first two are
/// paired; a look finds the other two, the keyboard first.
type Device<'a> = (&'a str, Option<&'a str>, DeviceKind);
const DEVICES: [Device; 4] = [
    ("8BitDo SN30 Pro", Some("CONNECTED"), DeviceKind::Gamepad),
    ("WH-1000XM4", Some("PAIRED"), DeviceKind::Audio),
    ("Keychron K3", None, DeviceKind::Keyboard),
    ("Mi Smart Band 4", None, DeviceKind::Other),
];

/// What the screen shows: the radio's value, the devices, the row under the cursor (0 the
/// radio's, then the devices' from 1) and the note under the devices.
struct Shown<'a> {
    radio: &'a str,
    devices: Vec<Device<'a>>,
    lit: usize,
    note: Option<&'a str>,
}

/// Each of flow.html's #bt= states, as its ref is named.
fn states() -> [(&'static str, Shown<'static>); 6] {
    let mut pairing = DEVICES.to_vec();
    pairing[2].1 = Some("PAIRING");
    let on = |devices: &[Device<'static>], note| Shown {
        radio: "ON",
        devices: devices.to_vec(),
        lit: 0,
        note,
    };
    [
        (
            "on",
            on(&DEVICES[..2], Some("Press X to look for devices.")),
        ),
        (
            "off",
            Shown {
                radio: "OFF",
                devices: Vec::new(),
                lit: 0,
                note: Some("Bluetooth is off."),
            },
        ),
        ("empty", on(&[], Some("Press X to look for devices."))),
        ("looking", on(&DEVICES[..3], Some("Looking for devices…"))),
        ("found", on(&DEVICES, None)),
        (
            "pairing",
            Shown {
                radio: "ON",
                devices: pairing,
                lit: 3,
                note: None,
            },
        ),
    ]
}

/// `top` laid over `under` with its top left at (`x`, `y`), where it is not clear.
fn over(under: &mut UndoFace, top: &UndoFace, x: u32, y: u32) {
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

/// The modal at `modal_rect()`'s size, with nothing under it: its two panels, the one holding
/// the cursor with its title lit, the radio's row, the devices and the note.
fn composed(shown: &Shown, pal: &Palette) -> UndoFace {
    let [_, _, w, h] = modal_rect();
    let mut art = modal_face(w, h, pal);
    let active = usize::from(shown.lit > 0);
    let mut wells = [[0; 4]; 2];
    for (i, [x, y, pw, ph]) in network_panels(1).into_iter().enumerate() {
        let panel = panel_face(pw, ph, TITLES[i], i == active, pal);
        over(&mut art, &panel.face, x, y);
        let [wx, wy, ww, wh] = panel.well;
        wells[i] = [x + wx, y + wy, ww, wh];
    }
    let radio = Row {
        label: "BLUETOOTH",
        value: Some(shown.radio),
        lit: shown.lit == 0,
        carets: true,
        opens: false,
        dim: false,
        caret_room: false,
    };
    let [x, y, rw, _] = row_rect(wells[0], 0);
    over(&mut art, &row_face(rw, &radio, pal), x, y);
    for (i, &(label, value, kind)) in shown.devices.iter().enumerate() {
        let row = ListRow {
            label,
            value,
            signal: None,
            lock: false,
            device: Some(kind),
            lit: shown.lit == i + 1,
            dim: false,
        };
        let [x, y, rw, _] = row_rect(wells[1], i as u32);
        over(&mut art, &list_row_face(rw, &row, pal), x, y);
    }
    if let Some(note) = shown.note {
        let [x, y, _, _] = wells[1];
        let below = shown.devices.len() as u32 * ROW_H + NOTE_TOP;
        over(&mut art, &text_face(note, pal.dim), x + NOTE_IN, y + below);
    }
    art
}

fn read_png(path: &str) -> UndoFace {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().expect("a PNG");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("its pixels");
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type.samples() {
        4 => buf,
        _ => buf
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
    };
    UndoFace {
        rgba,
        w: info.width,
        h: info.height,
    }
}

fn write(path: &str, face: &UndoFace) {
    let f = std::fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), face.w, face.h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .unwrap()
        .write_image_data(&face.rgba)
        .unwrap();
}

/// `face`'s pixel at (`x`, `y`): its colour, or None where it is clear.
fn px(face: &UndoFace, x: u32, y: u32) -> Option<[u8; 3]> {
    let i = ((y * face.w + x) * 4) as usize;
    (face.rgba[i + 3] != 0).then(|| [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]])
}

/// Every state at this canvas is flow.html's to the art pixel, wherever the modal is drawn (its
/// cut corners show the backdrop, which the faces leave clear). Set BLUETOOTH_OUT to a folder to
/// have each composed modal written there.
#[test]
fn bluetooth_is_flows_pixel_for_pixel() {
    let canvas = format!("{}x{}", out_w(), out_h());
    let pal = palette(NIGHT);
    for (state, shown) in states() {
        let name = format!("flow-bluetooth-{state}-{canvas}");
        let ours = composed(&shown, pal);
        if let Ok(dir) = std::env::var("BLUETOOTH_OUT") {
            write(&format!("{dir}/{name}.png"), &ours);
        }
        let path = format!(
            "{}/tests/suite/bluetooth/ref/{name}.png",
            env!("CARGO_MANIFEST_DIR")
        );
        let shot = read_png(&path);
        assert_eq!((ours.w, ours.h), (shot.w, shot.h), "{name}'s size");
        let off: Vec<_> = (0..ours.h)
            .flat_map(|y| (0..ours.w).map(move |x| (x, y)))
            .filter_map(|(x, y)| {
                let c = px(&ours, x, y)?;
                let want = px(&shot, x, y).expect("an opaque screenshot");
                (c != want).then_some((x, y, c, want))
            })
            .collect();
        assert!(
            off.is_empty(),
            "{name}: {} art pixels differ from flow.html's, first (x, y, ours, Chrome's) {:?}",
            off.len(),
            &off[..off.len().min(8)]
        );
    }
}

/// The screen's two panels share Network's layout with one setting: BLUETOOTH hugs its row and
/// DEVICES takes the rest of the modal, with room for every device flow.html shows and a note,
/// as while a look finds them.
#[test]
fn devices_has_room_for_flows_devices_and_a_note() {
    let [top, list] = network_panels(1);
    assert_eq!(top[3], 23 + ROW_H, "BLUETOOTH hugs its one row");
    let well = list[3] - 23;
    assert!(
        DEVICES.len() as u32 * ROW_H + NOTE_TOP + 10 <= well,
        "{well} pixels of well"
    );
}
