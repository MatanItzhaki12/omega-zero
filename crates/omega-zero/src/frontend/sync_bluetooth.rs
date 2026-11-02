//! The Bluetooth modal's faces, each baked only when what it shows changes, as Network's are:
//! the modal and its panels once for the palette, a row when its words, its lighting or the
//! palette change. A device found that comes into the list re-bakes the rows it moved and leaves
//! the others; the cursor moving re-bakes the row it leaves and the one it lands on.

use omega_gfx::Compositor;
use omega_ui::kit::list_row::{list_row_face, ListRow};
use omega_ui::kit::row::{row_face, Row};
use omega_ui::kit::text::text_face;
use omega_ui::network::{network_row_widths, NetworkFaces};

use crate::app::App;
use crate::bluetooth_screen::{BluetoothView, DeviceLine, Lit};
use crate::frontend::sync_network::{room_for, Held};
use crate::screens::bluetooth::{bluetooth_chrome, bluetooth_title};
use crate::wifi_screen::SettingLine;

/// The Bluetooth modal's textures. Its panels are laid out the same way whatever it shows, so
/// the palette colour is all the chrome is baked for.
#[derive(Default)]
pub(crate) struct BluetoothTexes {
    chrome: Held<u8>,
    wells: [[u32; 4]; 2],
    title_at: [[u32; 4]; 2],
    titles: [Held<u8>; 2],
    switch: Held<(SettingLine, bool, u8)>,
    devices: Vec<Held<(DeviceLine, bool, u8)>>,
    notes: Vec<Held<(String, u8)>>,
    /// What the faces were last made for.
    shown: Option<(BluetoothView, u8)>,
}

pub(crate) fn sync_bluetooth(
    app: &mut App,
    compositor: &mut Compositor,
    state: &mut BluetoothTexes,
) {
    let Some(view) = app.bluetooth_view() else {
        return;
    };
    let colour = app.palette_colour();
    let now = (view, colour);
    if state.shown.as_ref() == Some(&now) {
        return;
    }
    let (view, colour) = now;
    let pal = app.palette();

    let (wells, title_at) = (&mut state.wells, &mut state.title_at);
    let chrome = state.chrome.face(compositor, colour, |_| {
        let chrome = bluetooth_chrome(pal);
        *wells = chrome.wells;
        *title_at = chrome.titles;
        chrome.face
    });
    let titles =
        [0, 1].map(|i| state.titles[i].face(compositor, colour, |_| bluetooth_title(i, pal)));

    let [switch_w, device_w] = network_row_widths(&state.wells);
    let key = (view.switch.clone(), view.lit == Lit::Switch, colour);
    let switch = state.switch.face(compositor, key, |(line, lit, _)| {
        let row = Row {
            label: line.label,
            value: line.value,
            lit: *lit,
            carets: line.carets,
            opens: line.opens,
            dim: false,
            caret_room: false,
        };
        row_face(switch_w, &row, pal)
    });
    room_for(&mut state.devices, view.devices.len());
    let devices = view
        .devices
        .iter()
        .zip(&mut state.devices)
        .enumerate()
        .map(|(i, (line, held))| {
            let key = (line.clone(), view.lit == Lit::Device(i), colour);
            held.face(compositor, key, |(line, lit, _)| {
                let row = ListRow {
                    label: &line.name,
                    value: line.value,
                    signal: None,
                    lock: false,
                    device: Some(line.kind),
                    lit: *lit,
                    dim: false,
                };
                list_row_face(device_w, &row, pal)
            })
        })
        .collect();
    room_for(&mut state.notes, view.notes.len());
    let notes = view
        .notes
        .iter()
        .zip(&mut state.notes)
        .map(|(note, held)| {
            held.face(compositor, (note.clone(), colour), |(note, _)| {
                text_face(note, pal.dim)
            })
        })
        .collect();

    app.set_bluetooth_faces(NetworkFaces {
        chrome,
        wells: state.wells,
        title_at: state.title_at,
        titles,
        active: matches!(view.lit, Lit::Device(_)) as usize,
        settings: vec![switch],
        networks: devices,
        notes,
    });
    state.shown = Some((view, colour));
}
