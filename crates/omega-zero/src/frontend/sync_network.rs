//! The Network modal's faces, each baked only when what it shows changes: the modal and its
//! panels once for the palette (and again when Wi-Fi going on or off makes NETWORK taller or
//! shorter), a row when its words, its lighting or the palette change. A scan that brings one
//! network in re-bakes the rows it moved and leaves the others; the cursor moving re-bakes the
//! row it leaves and the one it lands on.

use omega_gfx::{Compositor, TexId};
use omega_ui::kit::list_row::{list_row_face, ListRow};
use omega_ui::kit::row::{row_face, Row};
use omega_ui::kit::text::text_face;
use omega_ui::network::{network_chrome, network_row_widths, network_title, Face, NetworkFaces};
use omega_ui::UndoFace;

use crate::app::App;
use crate::frontend::faces::upload_nearest;
use crate::wifi_screen::{Lit, NetworkLine, NetworkView, SettingLine};

/// A texture, and what it was last baked to show.
pub(crate) struct Held<K> {
    tex: Option<TexId>,
    size: (u32, u32),
    shows: Option<K>,
}

impl<K> Default for Held<K> {
    fn default() -> Self {
        Held {
            tex: None,
            size: (0, 0),
            shows: None,
        }
    }
}

impl<K: PartialEq> Held<K> {
    /// The face showing `key`: as it is when it already shows it, or baked by `bake` and
    /// uploaded into the same texture.
    pub(crate) fn face(
        &mut self,
        compositor: &mut Compositor,
        key: K,
        bake: impl FnOnce(&K) -> UndoFace,
    ) -> Face {
        if self.tex.is_none() || self.shows.as_ref() != Some(&key) {
            let face = bake(&key);
            upload_nearest(compositor, &mut self.tex, &face);
            self.size = (face.w, face.h);
            self.shows = Some(key);
        }
        let tex = self
            .tex
            .expect("a held face is uploaded before it is drawn");
        (tex, self.size.0, self.size.1)
    }
}

/// `held` grown to `n` faces, for as many rows as there are.
pub(crate) fn room_for<K>(held: &mut Vec<Held<K>>, n: usize) {
    if held.len() < n {
        held.resize_with(n, Held::default);
    }
}

/// The Network modal's textures. The canvas is fixed for the process, so the palette colour
/// and NETWORK's row count are all the chrome is baked for.
#[derive(Default)]
pub(crate) struct NetworkTexes {
    chrome: Held<(u8, usize)>,
    wells: [[u32; 4]; 2],
    title_at: [[u32; 4]; 2],
    titles: [Held<u8>; 2],
    settings: Vec<Held<(SettingLine, bool, u8)>>,
    networks: Vec<Held<(NetworkLine, bool, u8)>>,
    notes: Vec<Held<(String, u8)>>,
    /// What the faces were last made for.
    shown: Option<(NetworkView, u8)>,
}

pub(crate) fn sync_network(app: &mut App, compositor: &mut Compositor, state: &mut NetworkTexes) {
    let Some(view) = app.network_view() else {
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
    let chrome = state
        .chrome
        .face(compositor, (colour, view.settings.len()), |&(_, rows)| {
            let chrome = network_chrome(rows, pal);
            *wells = chrome.wells;
            *title_at = chrome.titles;
            chrome.face
        });
    let titles =
        [0, 1].map(|i| state.titles[i].face(compositor, colour, |_| network_title(i, pal)));

    let [setting_w, network_w] = network_row_widths(&state.wells);
    room_for(&mut state.settings, view.settings.len());
    let settings = view
        .settings
        .iter()
        .zip(&mut state.settings)
        .enumerate()
        .map(|(i, (line, held))| {
            let key = (line.clone(), view.lit == Lit::Setting(i), colour);
            held.face(compositor, key, |(line, lit, _)| {
                let row = Row {
                    label: line.label,
                    value: line.value,
                    lit: *lit,
                    carets: line.carets,
                    opens: line.opens,
                    dim: false,
                    caret_room: false,
                };
                row_face(setting_w, &row, pal)
            })
        })
        .collect();
    room_for(&mut state.networks, view.networks.len());
    let networks = view
        .networks
        .iter()
        .zip(&mut state.networks)
        .enumerate()
        .map(|(i, (line, held))| {
            let key = (line.clone(), view.lit == Lit::Network(i), colour);
            held.face(compositor, key, |(line, lit, _)| {
                let row = ListRow {
                    label: &line.name,
                    value: line.value,
                    signal: line.signal,
                    lock: line.lock,
                    device: None,
                    lit: *lit,
                    dim: false,
                };
                list_row_face(network_w, &row, pal)
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

    app.set_network_faces(NetworkFaces {
        chrome,
        wells: state.wells,
        title_at: state.title_at,
        titles,
        active: matches!(view.lit, Lit::Network(_)) as usize,
        settings,
        networks,
        notes,
    });
    state.shown = Some((view, colour));
}
